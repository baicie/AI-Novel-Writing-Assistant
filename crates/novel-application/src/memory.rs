use std::sync::Arc;

use serde::Deserialize;
use tokio_util::sync::CancellationToken;

use crate::dto::ChapterSnapshot;
use crate::error::AppError;
use crate::hash::sha256_hex;
use crate::ports::{
    Clock, Ids, MemoryCandidateView, MemoryConflictView, MemoryExtractionJobView, MemoryRepository,
    ModelSecrets, NewMemoryCandidate, NewMemoryJob, NovelRepository,
};
use novel_llm::LlmProvider;

pub struct ExtractMemoryInput {
    pub command_id: String,
    pub novel_id: String,
    pub chapter_id: String,
    pub provider: String,
    pub model: String,
    pub endpoint: String,
    pub allow_localhost_http: bool,
    pub token_budget: Option<u64>,
}

pub struct MemoryService {
    novels: Arc<dyn NovelRepository>,
    memory: Arc<dyn MemoryRepository>,
    llm: Arc<dyn LlmProvider>,
    clock: Arc<dyn Clock>,
    ids: Arc<dyn Ids>,
    secrets: Arc<dyn ModelSecrets>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExtractionOutput {
    facts: Vec<ExtractedFact>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExtractedFact {
    fact_type: String,
    truthfulness: String,
    entity_name: Option<String>,
    value: String,
    evidence_quote: String,
    confidence: Option<f64>,
}

impl MemoryService {
    pub fn new(
        novels: Arc<dyn NovelRepository>,
        memory: Arc<dyn MemoryRepository>,
        llm: Arc<dyn LlmProvider>,
        clock: Arc<dyn Clock>,
        ids: Arc<dyn Ids>,
        secrets: Arc<dyn ModelSecrets>,
    ) -> Self {
        Self {
            novels,
            memory,
            llm,
            clock,
            ids,
            secrets,
        }
    }

    pub async fn list_candidates(
        &self,
        novel_id: &str,
        chapter_id: &str,
    ) -> Result<Vec<MemoryCandidateView>, AppError> {
        Ok(self
            .memory
            .list_memory_candidates(novel_id, chapter_id)
            .await?)
    }

    pub async fn extract(
        &self,
        input: ExtractMemoryInput,
    ) -> Result<MemoryExtractionJobView, AppError> {
        if let Some(existing) = self.memory.memory_job_by_command(&input.command_id).await? {
            return Ok(existing);
        }
        let chapter = self
            .novels
            .get_chapter(&input.novel_id, &input.chapter_id)
            .await?;
        if chapter.official_version <= 0 || chapter.content.trim().is_empty() {
            return Err(AppError::validation("只有存在正式正文的章节才能提取记忆。"));
        }
        let characters = self.novels.list_characters(&input.novel_id).await?;
        let characters_text = characters
            .iter()
            .map(|character| format!("{}(id={})", character.name, character.id))
            .collect::<Vec<_>>()
            .join("、");
        let asset = novel_prompt::resolve("memory_extraction", "memory-extraction-v1")
            .map_err(|_| AppError::validation("记忆抽取提示词版本不存在"))?;
        let mut vars = std::collections::BTreeMap::new();
        vars.insert("characters".into(), characters_text);
        vars.insert("title".into(), chapter.title.clone());
        vars.insert("content".into(), chapter.content.clone());
        let prompt = novel_prompt::render(asset.template, &vars)
            .map_err(|_| AppError::validation("记忆抽取提示词渲染失败"))?;
        let job_id = self.ids.new_id();
        let now = self.clock.now_ms();
        let api_key = self
            .secrets
            .api_key(&input.provider)
            .await?
            .unwrap_or_default();
        let request_id = self.ids.new_id();
        let (sender, mut receiver) = tokio::sync::mpsc::channel::<String>(64);
        let collect = tokio::spawn(async move {
            let mut text = String::new();
            while let Some(delta) = receiver.recv().await {
                text.push_str(&delta);
            }
            text
        });
        let result = self
            .llm
            .generate(
                novel_llm::LlmRequest {
                    request_id: request_id.clone(),
                    model: input.model.clone(),
                    prompt,
                    endpoint: input.endpoint.clone(),
                    api_key,
                    allow_localhost_http: input.allow_localhost_http,
                    max_output_tokens: Some(input.token_budget.unwrap_or(1_500)),
                },
                sender,
                CancellationToken::new(),
            )
            .await;
        let raw = collect.await.unwrap_or_default();
        let response = match result {
            Ok(response) => response,
            Err(error) => {
                let job = NewMemoryJob {
                    id: job_id.clone(),
                    command_id: input.command_id,
                    novel_id: input.novel_id,
                    chapter_id: input.chapter_id,
                    chapter_version: chapter.official_version,
                    source_content_hash: sha256_hex(chapter.content.as_bytes()),
                    extraction_version: "memory-extraction-v1".into(),
                    provider: input.provider,
                    model: input.model,
                    endpoint: input.endpoint,
                    now_ms: now,
                };
                let _ = self.memory.create_memory_job(job, &[]).await?;
                self.memory
                    .fail_memory_job(&job_id, &error.to_string(), now)
                    .await?;
                return Err(AppError::new(
                    crate::error::AppErrorCode::IntegrityError,
                    format!("记忆抽取模型调用失败：{error}"),
                    false,
                ));
            }
        };
        let text = if raw.trim().is_empty() {
            response.text
        } else {
            raw
        };
        let parsed: ExtractionOutput = match serde_json::from_str(text.trim()) {
            Ok(parsed) => parsed,
            Err(error) => {
                let job = NewMemoryJob {
                    id: job_id.clone(),
                    command_id: input.command_id,
                    novel_id: input.novel_id,
                    chapter_id: input.chapter_id,
                    chapter_version: chapter.official_version,
                    source_content_hash: sha256_hex(chapter.content.as_bytes()),
                    extraction_version: "memory-extraction-v1".into(),
                    provider: input.provider,
                    model: input.model,
                    endpoint: input.endpoint,
                    now_ms: now,
                };
                let _ = self.memory.create_memory_job(job, &[]).await?;
                self.memory
                    .fail_memory_job(&job_id, &format!("模型输出不是合法 JSON：{error}"), now)
                    .await?;
                return Err(AppError::integrity(
                    "模型输出不是合法记忆 JSON，未保存任何候选。",
                ));
            }
        };
        let mut candidates = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for fact in parsed.facts {
            let fact_type = fact.fact_type.as_str();
            if !matches!(
                fact_type,
                "character_state" | "timeline_event" | "plot_thread"
            ) {
                return Err(AppError::integrity("抽取结果包含未知事实类型。"));
            }
            if !matches!(
                fact.truthfulness.as_str(),
                "narrator_fact"
                    | "character_claim"
                    | "rumor"
                    | "hypothesis"
                    | "plan"
                    | "author_asserted"
            ) {
                return Err(AppError::integrity("抽取结果包含未知真实性类型。"));
            }
            if fact.value.trim().is_empty() || fact.evidence_quote.trim().is_empty() {
                return Err(AppError::integrity("抽取结果缺少事实正文或原文证据。"));
            }
            let Some(evidence_start) = chapter.content.find(&fact.evidence_quote) else {
                return Err(AppError::integrity("抽取结果的证据不是章节正文原文。"));
            };
            let (entity_id, ambiguous) = if fact_type == "character_state" {
                let name = fact.entity_name.as_deref().unwrap_or("").trim();
                let matches = characters
                    .iter()
                    .filter(|character| character.name == name)
                    .collect::<Vec<_>>();
                if matches.len() == 1 {
                    (Some(matches[0].id.clone()), false)
                } else {
                    (None, true)
                }
            } else {
                (None, false)
            };
            let key = format!(
                "{}\u{001f}{}\u{001f}{}",
                fact.fact_type,
                fact.value.trim(),
                fact.evidence_quote.trim()
            );
            if !seen.insert(key) {
                continue;
            }
            candidates.push(NewMemoryCandidate {
                id: self.ids.new_id(),
                job_id: job_id.clone(),
                novel_id: input.novel_id.clone(),
                chapter_id: input.chapter_id.clone(),
                chapter_version: chapter.official_version,
                source_content_hash: sha256_hex(chapter.content.as_bytes()),
                fact_type: fact.fact_type,
                truthfulness: fact.truthfulness,
                entity_name: fact.entity_name,
                entity_id,
                value: fact.value.trim().to_string(),
                evidence_quote: fact.evidence_quote,
                evidence_start: Some(evidence_start as i64),
                confidence: fact.confidence.map(|value| value.clamp(0.0, 1.0)),
                ambiguous,
                now_ms: now,
            });
        }
        let job = NewMemoryJob {
            id: job_id,
            command_id: input.command_id,
            novel_id: input.novel_id,
            chapter_id: input.chapter_id,
            chapter_version: chapter.official_version,
            source_content_hash: sha256_hex(chapter.content.as_bytes()),
            extraction_version: "memory-extraction-v1".into(),
            provider: input.provider,
            model: input.model,
            endpoint: input.endpoint,
            now_ms: now,
        };
        Ok(self.memory.create_memory_job(job, &candidates).await?)
    }

    pub async fn sync_conflicts(
        &self,
        novel_id: &str,
    ) -> Result<Vec<MemoryConflictView>, AppError> {
        Ok(self
            .memory
            .sync_memory_conflicts(novel_id, self.clock.now_ms())
            .await?)
    }

    pub async fn list_conflicts(
        &self,
        novel_id: &str,
    ) -> Result<Vec<MemoryConflictView>, AppError> {
        Ok(self.memory.list_memory_conflicts(novel_id).await?)
    }

    pub async fn resolve_conflict(
        &self,
        conflict_id: &str,
        expected_version: i64,
        command_id: &str,
        decision: &str,
        chosen_candidate_id: Option<&str>,
        reason: &str,
        operator: &str,
    ) -> Result<MemoryConflictView, AppError> {
        if command_id.trim().is_empty() || operator.trim().is_empty() {
            return Err(AppError::validation("冲突裁决需要 commandId 和操作者。"));
        }
        if !matches!(
            decision,
            "confirm_new"
                | "keep_existing"
                | "mark_disputed"
                | "supersede_with_evidence"
                | "reject_extraction"
        ) {
            return Err(AppError::validation("未知的冲突裁决动作。"));
        }
        let request_hash = sha256_hex(
            format!("{conflict_id}\u{1f}{expected_version}\u{1f}{decision}\u{1f}{}\u{1f}{}\u{1f}{reason}\u{1f}{operator}", chosen_candidate_id.unwrap_or(""), command_id).as_bytes(),
        );
        Ok(self
            .memory
            .resolve_memory_conflict(
                conflict_id,
                expected_version,
                command_id,
                &request_hash,
                decision,
                chosen_candidate_id,
                reason,
                operator,
                self.clock.now_ms(),
            )
            .await?)
    }

    pub async fn confirm(
        &self,
        candidate_id: &str,
        expected_version: i64,
    ) -> Result<MemoryCandidateView, AppError> {
        Ok(self
            .memory
            .confirm_memory_candidate(candidate_id, expected_version, self.clock.now_ms())
            .await?)
    }

    pub async fn reject(
        &self,
        candidate_id: &str,
        expected_version: i64,
        reason: &str,
    ) -> Result<MemoryCandidateView, AppError> {
        if reason.trim().is_empty() {
            return Err(AppError::validation("拒绝记忆候选需要填写原因。"));
        }
        Ok(self
            .memory
            .reject_memory_candidate(candidate_id, expected_version, reason, self.clock.now_ms())
            .await?)
    }

    pub async fn defer(
        &self,
        candidate_id: &str,
        expected_version: i64,
    ) -> Result<MemoryCandidateView, AppError> {
        Ok(self
            .memory
            .defer_memory_candidate(candidate_id, expected_version, self.clock.now_ms())
            .await?)
    }

    pub async fn update_value(
        &self,
        candidate_id: &str,
        expected_version: i64,
        value: &str,
    ) -> Result<MemoryCandidateView, AppError> {
        if value.trim().is_empty() {
            return Err(AppError::validation("候选事实正文不能为空。"));
        }
        Ok(self
            .memory
            .update_memory_candidate_value(
                candidate_id,
                expected_version,
                value.trim(),
                self.clock.now_ms(),
            )
            .await?)
    }
}

pub fn chapter_source_identity(chapter: &ChapterSnapshot) -> (i64, String) {
    (
        chapter.official_version,
        sha256_hex(chapter.content.as_bytes()),
    )
}
