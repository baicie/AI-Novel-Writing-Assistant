use std::collections::BTreeMap;
use std::sync::Arc;

use dashmap::DashMap;
use serde::Serialize;

use novel_domain::{CreationExperience, NarrativeForm, PauseKind, ProjectMode, WritingMode};
use novel_llm::authorize_endpoint;
use novel_prompt::resolve;
use novel_workflow::{CHAPTER_PLAN_VERSION, PRODUCTION_PLAN_VERSION};

use crate::dto::{
    CandidateEditDraftView, ChapterDraftView, ChapterRevisionSnapshot, ChapterSnapshot,
    CharacterSnapshot, CreateNovelInput, GenerationEvent, ListNovelsQuery, ListNovelsResult,
    NovelDetail, NovelSummary, RunCandidateView, RunContract, RunDetailView, RunGateView,
    RunUsageView, UpdateNovelFields, VolumeSnapshot, event_from_stored,
};
use crate::error::AppError;
use crate::hash::{json_hash, sha256_hex};
use crate::ports::{
    Clock, CommandOutcome, DeleteChapterRecord, Ids, NewChapter, NewNovel, NewVolume,
    NovelRepository, OpenRunRecord, RestoreChapterRecord, SaveCandidateEditDraftRecord,
    SaveChapterDraftRecord, SaveChapterRecord, UpdateChapterMetaRecord, UpdateVolumeRecord,
    WorkflowRepository,
};
use crate::provider_service::FrozenRoute;

#[derive(Debug, Clone)]
pub struct OpenRunInput {
    pub command_id: String,
    pub novel_id: String,
    pub target_chapter_id: String,
    pub provider: String,
    pub model: String,
    pub prompt_versions: BTreeMap<String, String>,
    pub token_budget: Option<u64>,
    pub context_token_budget: Option<u64>,
    pub max_automatic_retries: u8,
    pub repair_budget: Option<u8>,
    pub endpoint: String,
    pub allow_localhost_http: bool,
}

/// 事件流内存缓存，用于减少数据库查询
pub struct EventCache {
    // run_id -> Vec<(seq, GenerationEvent)>
    active_runs: DashMap<String, Vec<(i64, GenerationEvent)>>,
}

impl EventCache {
    pub fn new() -> Self {
        Self {
            active_runs: DashMap::new(),
        }
    }

    /// 记录一条事件到缓存
    pub fn record_event(&self, run_id: String, seq: i64, event: GenerationEvent) {
        let mut entry = self.active_runs.entry(run_id).or_default();
        entry.push((seq, event));

        // 只保留最近 1000 条事件，防止内存无限增长
        if entry.len() > 1000 {
            entry.drain(0..100);
        }
    }

    /// 获取指定 seq 之后的所有缓存事件
    pub fn get_events(&self, run_id: &str, after_seq: i64) -> Vec<GenerationEvent> {
        self.active_runs
            .get(run_id)
            .map(|entry| {
                entry
                    .iter()
                    .filter(|(seq, _)| *seq > after_seq)
                    .map(|(_, event)| event.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 清理已完成运行的缓存
    pub fn clear_run(&self, run_id: &str) {
        self.active_runs.remove(run_id);
    }

    /// 获取缓存命中情况（用于性能监控）
    #[allow(dead_code)]
    pub fn cache_size(&self) -> usize {
        self.active_runs.len()
    }
}

impl Default for EventCache {
    fn default() -> Self {
        Self::new()
    }
}

pub struct DirectorService {
    novels: Arc<dyn NovelRepository>,
    runs: Arc<dyn WorkflowRepository>,
    clock: Arc<dyn Clock>,
    ids: Arc<dyn Ids>,
    cache: Arc<EventCache>,
}

impl DirectorService {
    pub fn new(
        novels: Arc<dyn NovelRepository>,
        runs: Arc<dyn WorkflowRepository>,
        clock: Arc<dyn Clock>,
        ids: Arc<dyn Ids>,
    ) -> Self {
        Self {
            novels,
            runs,
            clock,
            ids,
            cache: Arc::new(EventCache::new()),
        }
    }

    /// 获取内部的事件缓存，用于后台 Worker 记录事件
    pub fn event_cache(&self) -> Arc<EventCache> {
        self.cache.clone()
    }

    pub async fn list_novels(&self) -> Result<Vec<NovelSummary>, AppError> {
        Ok(self.novels.list_novels().await?)
    }

    pub async fn list_novels_paginated(
        &self,
        query: &ListNovelsQuery,
    ) -> Result<ListNovelsResult, AppError> {
        Ok(self.novels.list_novels_paginated(query).await?)
    }

    pub async fn get_novel(&self, id: &str) -> Result<NovelDetail, AppError> {
        Ok(self.novels.get_novel(id).await?)
    }

    pub async fn update_novel(
        &self,
        id: &str,
        fields: &UpdateNovelFields,
    ) -> Result<NovelDetail, AppError> {
        Ok(self
            .novels
            .update_novel(id, fields, self.clock.now_ms())
            .await?)
    }

    pub async fn delete_novel(&self, id: &str) -> Result<(), AppError> {
        Ok(self.novels.delete_novel(id).await?)
    }

    pub async fn set_creation_experience(
        &self,
        id: &str,
        experience: CreationExperience,
    ) -> Result<NovelDetail, AppError> {
        Ok(self
            .novels
            .set_creation_experience(id, experience, self.clock.now_ms())
            .await?)
    }

    pub async fn create_novel(
        &self,
        title: &str,
        description: &str,
    ) -> Result<NovelSummary, AppError> {
        let title = title.trim();
        if title.is_empty() {
            return Err(AppError::validation("先写一个书名。"));
        }
        Ok(self
            .novels
            .create_novel(NewNovel {
                id: self.ids.new_id(),
                title: title.to_string(),
                description: description.trim().to_string(),
                now_ms: self.clock.now_ms(),
            })
            .await?)
    }

    /// Phase 2 入口：带着 setup 字段一起建书。
    pub async fn create_novel_with_setup(
        &self,
        title: &str,
        description: &str,
        narrative_form: NarrativeForm,
        writing_mode: WritingMode,
        project_mode: ProjectMode,
        creation_experience: CreationExperience,
        target_audience: &str,
        book_selling_point: &str,
        competing_feel: &str,
        first_30_chapter_promise: &str,
        commercial_tags: Vec<String>,
        genre_id: Option<String>,
        primary_story_mode_id: Option<String>,
        secondary_story_mode_id: Option<String>,
        world_id: Option<String>,
        style_tone: &str,
        default_chapter_length: Option<i32>,
        estimated_chapter_count: Option<i32>,
        post_generation_style_review_enabled: bool,
    ) -> Result<NovelDetail, AppError> {
        let title = title.trim();
        if title.is_empty() {
            return Err(AppError::validation("先写一个书名。"));
        }
        let input = CreateNovelInput {
            id: self.ids.new_id(),
            title: title.to_string(),
            description: description.trim().to_string(),
            narrative_form,
            writing_mode,
            project_mode,
            creation_experience,
            target_audience: target_audience.trim().to_string(),
            book_selling_point: book_selling_point.trim().to_string(),
            competing_feel: competing_feel.trim().to_string(),
            first_30_chapter_promise: first_30_chapter_promise.trim().to_string(),
            commercial_tags,
            genre_id,
            primary_story_mode_id,
            secondary_story_mode_id,
            world_id,
            style_tone: style_tone.trim().to_string(),
            default_chapter_length,
            estimated_chapter_count,
            post_generation_style_review_enabled,
            now_ms: self.clock.now_ms(),
        };
        Ok(self.novels.create_novel_with_setup(input).await?)
    }

    pub async fn create_chapter(
        &self,
        novel_id: &str,
        title: &str,
        sort_order: i32,
    ) -> Result<ChapterSnapshot, AppError> {
        self.create_chapter_in_volume(novel_id, title, sort_order, None)
            .await
    }

    pub async fn create_chapter_in_volume(
        &self,
        novel_id: &str,
        title: &str,
        sort_order: i32,
        volume_id: Option<String>,
    ) -> Result<ChapterSnapshot, AppError> {
        let title = title.trim();
        if title.is_empty() || sort_order < 1 {
            return Err(AppError::validation("章节需要标题和从 1 开始的顺序。"));
        }
        Ok(self
            .novels
            .create_chapter(NewChapter {
                id: self.ids.new_id(),
                novel_id: novel_id.to_string(),
                title: title.to_string(),
                sort_order,
                volume_id,
                now_ms: self.clock.now_ms(),
            })
            .await?)
    }

    pub async fn get_chapter(
        &self,
        novel_id: &str,
        chapter_id: &str,
    ) -> Result<ChapterSnapshot, AppError> {
        Ok(self.novels.get_chapter(novel_id, chapter_id).await?)
    }

    pub async fn list_chapters(&self, novel_id: &str) -> Result<Vec<ChapterSnapshot>, AppError> {
        Ok(self.novels.list_chapters(novel_id).await?)
    }

    pub async fn list_characters(
        &self,
        novel_id: &str,
    ) -> Result<Vec<CharacterSnapshot>, AppError> {
        Ok(self.novels.list_characters(novel_id).await?)
    }

    pub async fn save_chapter(
        &self,
        novel_id: &str,
        chapter_id: &str,
        expected_version: i64,
        title: &str,
        content: &str,
    ) -> Result<ChapterSnapshot, AppError> {
        let title = title.trim();
        if title.is_empty() {
            return Err(AppError::validation("章节标题不能是空的。"));
        }
        Ok(self
            .novels
            .save_user_chapter(SaveChapterRecord {
                novel_id: novel_id.to_string(),
                chapter_id: chapter_id.to_string(),
                expected_version,
                title: title.to_string(),
                content: content.to_string(),
                revision_id: self.ids.new_id(),
                artifact_id: self.ids.new_id(),
                content_sha256: sha256_hex(content.as_bytes()),
                now_ms: self.clock.now_ms(),
            })
            .await?)
    }

    pub async fn delete_chapter(
        &self,
        novel_id: &str,
        chapter_id: &str,
        expected_version: i64,
    ) -> Result<(), AppError> {
        Ok(self
            .novels
            .delete_chapter(DeleteChapterRecord {
                novel_id: novel_id.to_string(),
                chapter_id: chapter_id.to_string(),
                expected_version,
            })
            .await?)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn update_chapter_meta(
        &self,
        novel_id: &str,
        chapter_id: &str,
        expected_version: i64,
        title: &str,
        sort_order: i32,
        volume_id: Option<String>,
    ) -> Result<ChapterSnapshot, AppError> {
        let title = title.trim();
        if title.is_empty() || sort_order < 1 {
            return Err(AppError::validation("章节需要标题和从 1 开始的顺序。"));
        }
        Ok(self
            .novels
            .update_chapter_meta(UpdateChapterMetaRecord {
                novel_id: novel_id.to_string(),
                chapter_id: chapter_id.to_string(),
                expected_version,
                title: title.to_string(),
                sort_order,
                volume_id,
            })
            .await?)
    }

    pub async fn list_chapter_revisions(
        &self,
        novel_id: &str,
        chapter_id: &str,
    ) -> Result<Vec<ChapterRevisionSnapshot>, AppError> {
        Ok(self
            .novels
            .list_chapter_revisions(novel_id, chapter_id)
            .await?)
    }

    pub async fn restore_chapter_revision(
        &self,
        novel_id: &str,
        chapter_id: &str,
        source_revision_id: &str,
        expected_version: i64,
        command_id: &str,
        draft_decision: &str,
        draft_client_id: Option<&str>,
        expected_draft_revision: Option<i64>,
    ) -> Result<ChapterSnapshot, AppError> {
        Ok(self
            .novels
            .restore_chapter_revision(RestoreChapterRecord {
                novel_id: novel_id.to_string(),
                chapter_id: chapter_id.to_string(),
                source_revision_id: source_revision_id.to_string(),
                expected_version,
                revision_id: self.ids.new_id(),
                artifact_id: self.ids.new_id(),
                command_id: command_id.to_string(),
                operator: "desktop-author".into(),
                reason: String::new(),
                draft_decision: draft_decision.to_string(),
                draft_client_id: draft_client_id.map(str::to_string),
                expected_draft_revision,
                now_ms: self.clock.now_ms(),
            })
            .await?)
    }

    pub async fn create_volume(
        &self,
        novel_id: &str,
        title: &str,
        sort_order: i32,
        summary: &str,
    ) -> Result<VolumeSnapshot, AppError> {
        let title = title.trim();
        if title.is_empty() || sort_order < 1 {
            return Err(AppError::validation("卷需要标题和从 1 开始的顺序。"));
        }
        Ok(self
            .novels
            .create_volume(NewVolume {
                id: self.ids.new_id(),
                novel_id: novel_id.to_string(),
                title: title.to_string(),
                sort_order,
                summary: summary.trim().to_string(),
            })
            .await?)
    }

    pub async fn list_volumes(&self, novel_id: &str) -> Result<Vec<VolumeSnapshot>, AppError> {
        Ok(self.novels.list_volumes(novel_id).await?)
    }

    pub async fn update_volume(
        &self,
        novel_id: &str,
        volume_id: &str,
        title: &str,
        sort_order: i32,
        summary: &str,
    ) -> Result<VolumeSnapshot, AppError> {
        let title = title.trim();
        if title.is_empty() || sort_order < 1 {
            return Err(AppError::validation("卷需要标题和从 1 开始的顺序。"));
        }
        Ok(self
            .novels
            .update_volume(UpdateVolumeRecord {
                novel_id: novel_id.to_string(),
                volume_id: volume_id.to_string(),
                title: title.to_string(),
                sort_order,
                summary: summary.trim().to_string(),
            })
            .await?)
    }

    pub async fn delete_volume(&self, novel_id: &str, volume_id: &str) -> Result<(), AppError> {
        Ok(self.novels.delete_volume(novel_id, volume_id).await?)
    }

    pub async fn open_run(&self, input: OpenRunInput) -> Result<CommandOutcome, AppError> {
        self.open_run_with_route(input, FrozenRoute::default())
            .await
    }

    /// 与 open_run 相同，但把任务路由冻结进 Run 记录（旧调用方不传即保持原行为）。
    pub async fn open_run_with_route(
        &self,
        input: OpenRunInput,
        route: FrozenRoute,
    ) -> Result<CommandOutcome, AppError> {
        if input.command_id.trim().is_empty() {
            return Err(AppError::validation("这次操作缺少编号，请重新发起。"));
        }
        if input
            .context_token_budget
            .is_some_and(|budget| budget < 512)
        {
            return Err(AppError::validation("上下文预算不能小于 512。"));
        }
        if input.max_automatic_retries > 1 {
            return Err(AppError::validation("自动重试次数只能是 0 或 1。"));
        }
        if input.repair_budget.is_some_and(|budget| budget > 1) {
            return Err(AppError::validation("质量修复次数只能是 0 或 1。"));
        }
        let prompt_version = input
            .prompt_versions
            .get("chapter_draft")
            .map(String::as_str)
            .unwrap_or("");
        if resolve("chapter_draft", prompt_version).is_err() {
            return Err(AppError::validation("正文提示词版本不存在，不能开始创作。"));
        }
        authorize_endpoint(&input.endpoint, input.allow_localhost_http)
            .map_err(|error| AppError::validation(error.to_string()))?;
        let chapter = self
            .get_chapter(&input.novel_id, &input.target_chapter_id)
            .await?;
        let run_id = self.ids.new_id();
        let contract = RunContract {
            run_id: run_id.clone(),
            novel_id: input.novel_id.clone(),
            plan_version: PRODUCTION_PLAN_VERSION.to_string(),
            chapter_plan_version: CHAPTER_PLAN_VERSION.to_string(),
            target_chapter_id: input.target_chapter_id.clone(),
            provider: input.provider.clone(),
            model: input.model.clone(),
            prompt_versions: input.prompt_versions.clone(),
            token_budget: input.token_budget,
            context_token_budget: input
                .context_token_budget
                .unwrap_or(crate::context::DEFAULT_CONTEXT_TOKEN_BUDGET),
            max_automatic_retries: input.max_automatic_retries,
            repair_budget: input.repair_budget.unwrap_or(0),
            endpoint: input.endpoint.clone(),
            protocol: route.protocol.clone(),
            credential_ref: route.credential_ref.clone(),
            allow_localhost_http: input.allow_localhost_http,
            reasoning_effort: route.reasoning_effort.clone(),
        };
        let request_hash = json_hash(&OpenRunHash {
            novel_id: &input.novel_id,
            target_chapter_id: &input.target_chapter_id,
            provider: &input.provider,
            model: &input.model,
            prompt_versions: &input.prompt_versions,
            token_budget: input.token_budget,
            context_token_budget: input
                .context_token_budget
                .unwrap_or(crate::context::DEFAULT_CONTEXT_TOKEN_BUDGET),
            max_automatic_retries: input.max_automatic_retries,
            repair_budget: input.repair_budget,
            endpoint: &input.endpoint,
            allow_localhost_http: input.allow_localhost_http,
            plan_version: PRODUCTION_PLAN_VERSION,
        })?;
        let seed_payload = serde_json::json!({ "title": chapter.title }).to_string();
        Ok(self
            .runs
            .open_run(OpenRunRecord {
                command_id: input.command_id,
                request_hash,
                run_id,
                novel_id: input.novel_id,
                contract_json: serde_json::to_string(&contract)
                    .map_err(|_| AppError::validation("创作合同无法保存"))?,
                plan_version: PRODUCTION_PLAN_VERSION.to_string(),
                seed_artifact_id: self.ids.new_id(),
                seed_hash: sha256_hex(seed_payload.as_bytes()),
                seed_payload,
                now_ms: self.clock.now_ms(),
            })
            .await?)
    }

    pub async fn approve(
        &self,
        command_id: &str,
        run_id: &str,
    ) -> Result<CommandOutcome, AppError> {
        self.command(command_id, run_id, "approve", None).await
    }

    /// Confirm a gate with an explicit expected control version. Used by the
    /// native Director UI so a stale Gate click becomes a visible conflict.
    pub async fn approve_expected(
        &self,
        command_id: &str,
        run_id: &str,
        expected_version: i64,
    ) -> Result<CommandOutcome, AppError> {
        if command_id.trim().is_empty() {
            return Err(AppError::validation("这次操作缺少编号，请重新发起。"));
        }
        let request_hash = json_hash(&RunCommandHash {
            run_id,
            kind: "approve_expected",
            intent: None,
        })?;
        Ok(self
            .runs
            .approve(
                command_id,
                &request_hash,
                run_id,
                Some(expected_version),
                self.clock.now_ms(),
            )
            .await?)
    }

    pub async fn reject(
        &self,
        command_id: &str,
        run_id: &str,
        expected_version: Option<i64>,
    ) -> Result<CommandOutcome, AppError> {
        self.decide_gate(command_id, run_id, expected_version, false)
            .await
    }

    pub async fn regenerate(
        &self,
        command_id: &str,
        run_id: &str,
        expected_version: Option<i64>,
    ) -> Result<CommandOutcome, AppError> {
        self.decide_gate(command_id, run_id, expected_version, true)
            .await
    }

    async fn decide_gate(
        &self,
        command_id: &str,
        run_id: &str,
        expected_version: Option<i64>,
        regenerate: bool,
    ) -> Result<CommandOutcome, AppError> {
        if command_id.trim().is_empty() {
            return Err(AppError::validation("这次操作缺少编号，请重新发起。"));
        }
        let kind = if regenerate { "regenerate" } else { "reject" };
        let request_hash = json_hash(&RunCommandHash {
            run_id,
            kind,
            intent: expected_version.map(|_| "expected"),
        })?;
        Ok(self
            .runs
            .reject_gate(
                command_id,
                &request_hash,
                run_id,
                expected_version,
                regenerate,
                self.clock.now_ms(),
            )
            .await?)
    }

    pub async fn confirm_after_edit(
        &self,
        command_id: &str,
        run_id: &str,
        expected_version: Option<i64>,
        payload_json: &str,
    ) -> Result<CommandOutcome, AppError> {
        if command_id.trim().is_empty() {
            return Err(AppError::validation("这次操作缺少编号，请重新发起。"));
        }
        let request_hash = json_hash(&RunCommandHash {
            run_id,
            kind: "confirm_after_edit",
            intent: expected_version.map(|_| "expected"),
        })?;
        Ok(self
            .runs
            .confirm_after_edit(
                command_id,
                &request_hash,
                run_id,
                expected_version,
                &self.ids.new_id(),
                payload_json,
                self.clock.now_ms(),
            )
            .await?)
    }

    pub async fn confirm_candidate_after_edit(
        &self,
        command_id: &str,
        run_id: &str,
        expected_version: Option<i64>,
        base_candidate_id: &str,
        base_content_sha256: &str,
        content: &str,
    ) -> Result<CommandOutcome, AppError> {
        if command_id.trim().is_empty() {
            return Err(AppError::validation("这次操作缺少编号，请重新发起。"));
        }
        if content.trim().is_empty() {
            return Err(AppError::validation("修改后的候选正文不能为空。"));
        }
        let intent = format!(
            "{base_candidate_id}:{base_content_sha256}:{}",
            sha256_hex(content.as_bytes())
        );
        let request_hash = json_hash(&RunCommandHash {
            run_id,
            kind: "confirm_candidate_after_edit",
            intent: Some(&intent),
        })?;
        Ok(self
            .runs
            .confirm_candidate_after_edit(
                command_id,
                &request_hash,
                run_id,
                expected_version,
                base_candidate_id,
                base_content_sha256,
                content,
                &self.ids.new_id(),
                &self.ids.new_id(),
                self.clock.now_ms(),
            )
            .await?)
    }

    pub async fn cancel(&self, command_id: &str, run_id: &str) -> Result<CommandOutcome, AppError> {
        self.command(command_id, run_id, "cancel", None).await
    }

    pub async fn resume(
        &self,
        command_id: &str,
        run_id: &str,
        accept_candidate: bool,
    ) -> Result<CommandOutcome, AppError> {
        let intent = if accept_candidate { "accept" } else { "retry" };
        self.command(command_id, run_id, "resume", Some(intent))
            .await
    }

    /// 作者逐条处理完正文质量提醒后，从质量修复上限停点继续这次创作。
    /// 与普通恢复分开：只有确有审校问题留档、且质量提醒都已被作者处理时才放行。
    pub async fn resume_accepting_quality_debt(
        &self,
        command_id: &str,
        run_id: &str,
        reason: &str,
    ) -> Result<CommandOutcome, AppError> {
        if command_id.trim().is_empty() {
            return Err(AppError::validation("这次操作缺少编号，请重新发起。"));
        }
        let reason = reason.trim();
        if reason.is_empty() {
            return Err(AppError::validation(
                "请说明为什么可以带着这些质量提醒继续。",
            ));
        }
        let request_hash = json_hash(&RunCommandHash {
            run_id,
            kind: "resume_quality_debt",
            intent: Some(reason),
        })?;
        Ok(self
            .runs
            .resume_accepting_quality_debt(
                command_id,
                &request_hash,
                run_id,
                reason,
                self.clock.now_ms(),
            )
            .await?)
    }

    pub async fn replay(
        &self,
        run_id: &str,
        after_seq: i64,
    ) -> Result<Vec<GenerationEvent>, AppError> {
        // 1. 优先查询缓存
        let cached = self.cache.get_events(run_id, after_seq);
        if !cached.is_empty() {
            return Ok(cached); // 缓存命中
        }

        // 2. 缓存未命中，查询数据库
        let stored = self.runs.events_after(run_id, after_seq).await?;
        let mut events = Vec::with_capacity(stored.len());
        for event in stored {
            let mapped = event_from_stored(
                &event.run_id,
                event.seq,
                &event.kind,
                event.step_id,
                &event.payload_json,
                event.occurred_at,
            )
            .ok_or_else(|| AppError::integrity("运行记录里有无法识别的事件。"))?;
            events.push(mapped);
        }

        // 3. 记录到缓存供下次使用
        for event in &events {
            if let Some(seq) = event.seq() {
                self.cache
                    .record_event(run_id.to_string(), seq, event.clone());
            }
        }

        Ok(events)
    }

    pub async fn record_debt(
        &self,
        run_id: &str,
        novel_id: &str,
        chapter_order: i32,
        code: &str,
        detail: &str,
    ) -> Result<(), AppError> {
        Ok(self
            .runs
            .append_debt(
                &self.ids.new_id(),
                novel_id,
                run_id,
                chapter_order,
                code,
                detail,
                self.clock.now_ms(),
            )
            .await?)
    }

    /// 返回某本小说当前仍可继续的 Run；数据来自 workflow_control 的持久化状态。
    pub async fn active_run(
        &self,
        novel_id: &str,
    ) -> Result<Option<crate::dto::RunSummary>, AppError> {
        Ok(self.list_runs(novel_id).await?.into_iter().find(|run| {
            matches!(
                run.state.as_str(),
                "queued" | "running" | "waiting_approval" | "paused"
            )
        }))
    }

    /// Tauri Director 的稳定读模型。状态、Gate、当前 Step、候选和用量均来自持久化事实。
    pub async fn get_run_detail(
        &self,
        novel_id: &str,
        run_id: &str,
    ) -> Result<RunDetailView, AppError> {
        let summary = self
            .list_runs(novel_id)
            .await?
            .into_iter()
            .find(|run| run.id == run_id)
            .ok_or_else(|| AppError::not_found("没有找到这次创作。"))?;
        let control = self.runs.run_control(run_id).await?;
        let artifacts = self.runs.run_artifacts(run_id).await?;
        let candidate = self.runs.latest_candidate(run_id).await?;
        let usage = self.runs.run_usage(run_id).await?;
        let quality_debt = self
            .runs
            .list_quality_debt(&summary.novel_id, &summary.chapter_id)
            .await?;
        let pause_kind = control
            .pause
            .as_ref()
            .map(|pause| pause_kind_name(pause.kind));
        let pause_signal = control
            .pause
            .as_ref()
            .map(|pause| stop_signal_name(pause.signal));
        let pause_reason = control.pause.as_ref().map(|pause| pause.reason.clone());
        let gate = control.gate.as_ref().map(|gate| RunGateView {
            id: gate.id.clone(),
            artifact_types: gate.artifact_types.clone(),
        });
        let safety_pause = control.status == novel_domain::RunStatus::Paused
            && control
                .pause
                .as_ref()
                .is_some_and(|pause| pause.kind == novel_domain::PauseKind::Safety);
        let quality_stop = safety_pause
            && control
                .pause
                .as_ref()
                .is_some_and(|pause| pause.signal == novel_domain::StopSignalKind::Safety);
        // 质量修复上限停点：审校确实留下了问题，且每条问题都已被作者处理过。
        let review_issues = artifacts
            .iter()
            .filter(|artifact| artifact.artifact_type == "consistency_review")
            .max_by_key(|artifact| artifact.version)
            .and_then(|artifact| {
                serde_json::from_str::<serde_json::Value>(&artifact.payload_json).ok()
            })
            .and_then(|payload| payload.get("issues").and_then(serde_json::Value::as_i64))
            .unwrap_or(0);
        let open_debt = quality_debt
            .iter()
            .filter(|item| item.run_id == run_id)
            .filter(|item| !matches!(item.status.as_str(), "accepted" | "resolved"))
            .count();
        let quality_stop_ready = quality_stop && review_issues > 0 && open_debt == 0;
        let mut legal_actions = Vec::new();
        match control.status {
            novel_domain::RunStatus::WaitingApproval if control.gate.is_some() => {
                legal_actions.extend(
                    ["confirm", "reject", "regenerate", "confirm_after_edit"]
                        .into_iter()
                        .map(str::to_string),
                );
            }
            novel_domain::RunStatus::Paused if safety_pause => {
                legal_actions.push("cancel".to_string());
                if quality_stop_ready {
                    legal_actions.push("resume_quality_debt".to_string());
                }
            }
            novel_domain::RunStatus::Paused => {
                legal_actions.extend(["resume", "cancel"].into_iter().map(str::to_string));
            }
            novel_domain::RunStatus::Running | novel_domain::RunStatus::Queued => {
                legal_actions.push("cancel".to_string());
            }
            _ => {}
        }
        Ok(RunDetailView {
            id: summary.id,
            novel_id: summary.novel_id,
            chapter_id: summary.chapter_id,
            chapter_title: summary.chapter_title,
            status: control.status.as_str().to_string(),
            control_version: control.version,
            cursor_step_id: control.cursor_step_id,
            pause_kind: pause_kind.map(str::to_string),
            pause_signal: pause_signal.map(str::to_string),
            pause_reason,
            failure_reason: control.failure_reason,
            gate,
            latest_candidate: candidate.map(|candidate| RunCandidateView {
                id: candidate.id,
                content: candidate.content,
                content_sha256: candidate.content_sha256,
            }),
            quality_debt,
            legal_actions,
            artifacts,
            usage: RunUsageView {
                calls: usage.calls,
                prompt_tokens: usage.prompt_tokens,
                completion_tokens: usage.completion_tokens,
            },
            provider: summary.provider,
            model: summary.model,
            created_at: summary.created_at,
            updated_at: summary.updated_at,
        })
    }

    /// Phase 2.1：获取小说的所有运行列表（按时间倒序）。
    /// Persist an author-local draft without touching chapter_revision.
    pub async fn save_chapter_draft(
        &self,
        novel_id: &str,
        chapter_id: &str,
        client_id: &str,
        content: &str,
        base_official_version: i64,
        base_updated_at: &str,
        expected_revision: i64,
    ) -> Result<ChapterDraftView, AppError> {
        self.novels.get_chapter(novel_id, chapter_id).await?;
        if client_id.trim().is_empty() {
            return Err(AppError::validation("本地草稿缺少窗口标识。"));
        }
        Ok(self
            .novels
            .save_chapter_draft(SaveChapterDraftRecord {
                novel_id: novel_id.to_string(),
                chapter_id: chapter_id.to_string(),
                client_id: client_id.to_string(),
                content: content.to_string(),
                base_official_version,
                base_updated_at: base_updated_at.to_string(),
                expected_revision,
                now_ms: self.clock.now_ms(),
            })
            .await?)
    }

    pub async fn get_chapter_draft(
        &self,
        novel_id: &str,
        chapter_id: &str,
        client_id: &str,
    ) -> Result<Option<ChapterDraftView>, AppError> {
        Ok(self
            .novels
            .get_chapter_draft(novel_id, chapter_id, client_id)
            .await?)
    }

    pub async fn list_chapter_drafts(
        &self,
        novel_id: &str,
        chapter_id: &str,
    ) -> Result<Vec<ChapterDraftView>, AppError> {
        Ok(self
            .novels
            .list_chapter_drafts(novel_id, chapter_id)
            .await?)
    }

    pub async fn save_candidate_edit_draft(
        &self,
        novel_id: &str,
        chapter_id: &str,
        run_id: &str,
        gate_id: &str,
        client_id: &str,
        base_candidate_id: &str,
        base_content_sha256: &str,
        content: &str,
        control_version: i64,
        expected_revision: i64,
    ) -> Result<CandidateEditDraftView, AppError> {
        if client_id.trim().is_empty() || gate_id.trim().is_empty() {
            return Err(AppError::validation("候选编辑草稿缺少窗口或 Gate 标识。"));
        }
        self.novels.get_chapter(novel_id, chapter_id).await?;
        Ok(self
            .novels
            .save_candidate_edit_draft(SaveCandidateEditDraftRecord {
                novel_id: novel_id.into(),
                chapter_id: chapter_id.into(),
                run_id: run_id.into(),
                gate_id: gate_id.into(),
                client_id: client_id.into(),
                base_candidate_id: base_candidate_id.into(),
                base_content_sha256: base_content_sha256.into(),
                content: content.into(),
                control_version,
                expected_revision,
                now_ms: self.clock.now_ms(),
            })
            .await?)
    }

    pub async fn get_candidate_edit_draft(
        &self,
        run_id: &str,
        gate_id: &str,
        client_id: &str,
    ) -> Result<Option<CandidateEditDraftView>, AppError> {
        Ok(self
            .novels
            .get_candidate_edit_draft(run_id, gate_id, client_id)
            .await?)
    }

    pub async fn list_review_occurrences(
        &self,
        run_id: &str,
        stable_issue_key: &str,
    ) -> Result<Vec<crate::dto::ReviewOccurrenceView>, AppError> {
        Ok(self
            .runs
            .list_review_occurrences(run_id, stable_issue_key)
            .await?)
    }

    pub async fn accept_quality_debt(
        &self,
        run_id: &str,
        debt_id: &str,
        expected_version: i64,
        reason: &str,
    ) -> Result<crate::dto::QualityDebtView, AppError> {
        if reason.trim().is_empty() {
            return Err(AppError::validation("接受质量问题需要填写原因。"));
        }
        Ok(self
            .runs
            .accept_quality_debt(
                run_id,
                debt_id,
                expected_version,
                reason,
                self.clock.now_ms(),
            )
            .await?)
    }

    pub async fn list_candidate_edit_drafts(
        &self,
        novel_id: &str,
        run_id: &str,
    ) -> Result<Vec<CandidateEditDraftView>, AppError> {
        self.novels.get_novel(novel_id).await?;
        Ok(self
            .novels
            .list_candidate_edit_drafts(novel_id, run_id)
            .await?)
    }

    pub async fn clear_candidate_edit_draft(
        &self,
        run_id: &str,
        gate_id: &str,
        client_id: &str,
        content: &str,
        local_revision: i64,
    ) -> Result<bool, AppError> {
        Ok(self
            .novels
            .clear_candidate_edit_draft(run_id, gate_id, client_id, content, local_revision)
            .await?)
    }

    pub async fn clear_chapter_draft(
        &self,
        novel_id: &str,
        chapter_id: &str,
        client_id: &str,
        submitted_content: &str,
        local_revision: i64,
    ) -> Result<bool, AppError> {
        Ok(self
            .novels
            .clear_chapter_draft(
                novel_id,
                chapter_id,
                client_id,
                submitted_content,
                local_revision,
            )
            .await?)
    }

    pub async fn run_id_for_command(&self, command_id: &str) -> Result<Option<String>, AppError> {
        Ok(self.runs.run_id_for_command(command_id).await?)
    }

    pub async fn list_runs(&self, novel_id: &str) -> Result<Vec<crate::dto::RunSummary>, AppError> {
        Ok(self.runs.list_runs(novel_id).await?)
    }

    pub async fn list_all_runs(&self) -> Result<Vec<crate::dto::RunSummary>, AppError> {
        Ok(self.runs.list_all_runs().await?)
    }

    /// Phase 2.1：获取章节的完整详情。
    pub async fn get_chapter_full(
        &self,
        novel_id: &str,
        chapter_id: &str,
    ) -> Result<crate::dto::ChapterFullDetail, AppError> {
        Ok(self.runs.get_chapter_full(novel_id, chapter_id).await?)
    }

    async fn command(
        &self,
        command_id: &str,
        run_id: &str,
        kind: &str,
        intent: Option<&str>,
    ) -> Result<CommandOutcome, AppError> {
        if command_id.trim().is_empty() {
            return Err(AppError::validation("这次操作缺少编号，请重新发起。"));
        }
        let request_hash = json_hash(&RunCommandHash {
            run_id,
            kind,
            intent,
        })?;
        let now = self.clock.now_ms();
        let outcome = match kind {
            "approve" => {
                self.runs
                    .approve(command_id, &request_hash, run_id, None, now)
                    .await?
            }
            "cancel" => {
                self.runs
                    .cancel(command_id, &request_hash, run_id, now)
                    .await?
            }
            "resume" => {
                self.runs
                    .resume(
                        command_id,
                        &request_hash,
                        run_id,
                        intent.unwrap_or("accept"),
                        now,
                    )
                    .await?
            }
            _ => return Err(AppError::validation("不支持这个操作。")),
        };
        Ok(outcome)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OpenRunHash<'a> {
    novel_id: &'a str,
    target_chapter_id: &'a str,
    provider: &'a str,
    model: &'a str,
    prompt_versions: &'a BTreeMap<String, String>,
    token_budget: Option<u64>,
    context_token_budget: u64,
    max_automatic_retries: u8,
    repair_budget: Option<u8>,
    endpoint: &'a str,
    allow_localhost_http: bool,
    plan_version: &'a str,
}

#[derive(Serialize)]
struct RunCommandHash<'a> {
    run_id: &'a str,
    kind: &'a str,
    intent: Option<&'a str>,
}

fn stop_signal_name(kind: novel_domain::StopSignalKind) -> &'static str {
    use novel_domain::StopSignalKind;
    match kind {
        StopSignalKind::Budget => "budget",
        StopSignalKind::DataIntegrity => "data_integrity",
        StopSignalKind::ProviderUnavailable => "provider_unavailable",
        StopSignalKind::ApprovalRequired => "approval_required",
        StopSignalKind::MigrationReviewRequired => "migration_review_required",
        StopSignalKind::Replan => "replan",
        StopSignalKind::Safety => "safety",
        StopSignalKind::NoUsableContent => "no_usable_content",
        StopSignalKind::ManualRecovery => "manual_recovery",
    }
}

fn pause_kind_name(kind: PauseKind) -> &'static str {
    match kind {
        PauseKind::ManualRecovery => "manual_recovery",
        PauseKind::Replan => "replan",
        PauseKind::Safety => "safety",
        PauseKind::MigrationReview => "migration_review",
    }
}
