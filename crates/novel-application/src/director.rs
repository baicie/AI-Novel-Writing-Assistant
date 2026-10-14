use std::collections::BTreeMap;
use std::sync::Arc;

use serde::Serialize;

use novel_domain::{CreationExperience, NarrativeForm, ProjectMode, WritingMode};
use novel_llm::authorize_endpoint;
use novel_prompt::resolve;
use novel_workflow::{CHAPTER_PLAN_VERSION, PRODUCTION_PLAN_VERSION};

use crate::dto::{
    ChapterSnapshot, CreateNovelInput, GenerationEvent, ListNovelsQuery, ListNovelsResult,
    NovelDetail, NovelSummary, RunContract, UpdateNovelFields, event_from_stored,
};
use crate::error::AppError;
use crate::hash::{json_hash, sha256_hex};
use crate::ports::{
    Clock, CommandOutcome, Ids, NewChapter, NewNovel, NovelRepository,
    OpenRunRecord, SaveChapterRecord, WorkflowRepository,
};

#[derive(Debug, Clone)]
pub struct OpenRunInput {
    pub command_id: String,
    pub novel_id: String,
    pub target_chapter_id: String,
    pub provider: String,
    pub model: String,
    pub prompt_versions: BTreeMap<String, String>,
    pub token_budget: Option<u64>,
    pub max_automatic_retries: u8,
    pub endpoint: String,
    pub allow_localhost_http: bool,
}

pub struct DirectorService {
    novels: Arc<dyn NovelRepository>,
    runs: Arc<dyn WorkflowRepository>,
    clock: Arc<dyn Clock>,
    ids: Arc<dyn Ids>,
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
        }
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

    pub async fn open_run(&self, input: OpenRunInput) -> Result<CommandOutcome, AppError> {
        if input.command_id.trim().is_empty() {
            return Err(AppError::validation("这次操作缺少编号，请重新发起。"));
        }
        if input.max_automatic_retries > 1 {
            return Err(AppError::validation("自动重试次数只能是 0 或 1。"));
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
            max_automatic_retries: input.max_automatic_retries,
            endpoint: input.endpoint.clone(),
            allow_localhost_http: input.allow_localhost_http,
        };
        let request_hash = json_hash(&OpenRunHash {
            novel_id: &input.novel_id,
            target_chapter_id: &input.target_chapter_id,
            provider: &input.provider,
            model: &input.model,
            prompt_versions: &input.prompt_versions,
            token_budget: input.token_budget,
            max_automatic_retries: input.max_automatic_retries,
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

    pub async fn replay(
        &self,
        run_id: &str,
        after_seq: i64,
    ) -> Result<Vec<GenerationEvent>, AppError> {
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
                    .approve(command_id, &request_hash, run_id, now)
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
    max_automatic_retries: u8,
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
