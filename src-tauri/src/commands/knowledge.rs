use novel_application::{
    AppError, IndexJobRepository, IndexJobView, KnowledgeDocumentSummary, KnowledgeRepository,
    NewDocument, NewEmbedding, NewIndexJob, NovelRepository, SearchHit,
};
use novel_context::{Lexicon, fts_query, tokenize};
use serde::Deserialize;
use tauri::State;

use crate::state::AppState;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexKnowledgeRequest {
    pub id: String,
    pub novel_id: String,
    pub owner_type: String,
    pub title: String,
    pub body: String,
    pub embedding: Option<Vec<f32>>,
    pub embedding_model: Option<String>,
    pub source_revision: Option<String>,
    pub source_content_hash: Option<String>,
}

#[tauri::command]
pub async fn index_knowledge(
    state: State<'_, AppState>,
    request: IndexKnowledgeRequest,
) -> Result<(), AppError> {
    let body = request.body.trim();
    if body.is_empty() {
        return Err(AppError::validation("知识正文不能为空。"));
    }
    if !matches!(
        request.owner_type.as_str(),
        "world" | "character" | "chapter" | "style" | "document"
    ) {
        return Err(AppError::validation("未知的知识归属类型。"));
    }
    if request.owner_type == "chapter" || request.id.starts_with("chapter:") {
        return Err(AppError::validation(
            "正式章节索引只能由章节保存流程维护，不能通过通用知识接口覆盖。",
        ));
    }
    let terms = fts_query(&tokenize(body, &Lexicon::default()));
    let embedding = request.embedding.map(|vector| NewEmbedding {
        model: request
            .embedding_model
            .unwrap_or_else(|| "local".to_string()),
        normalized: true,
        vector,
    });
    state
        .kernel
        .index_document(NewDocument {
            id: request.id,
            novel_id: request.novel_id,
            owner_type: request.owner_type,
            title: request.title,
            body: body.to_string(),
            terms,
            created_at: novel_platform::now_ms(),
            source_revision: request.source_revision,
            source_content_hash: request.source_content_hash,
            embedding,
        })
        .await
        .map_err(AppError::from)
}

#[tauri::command]
pub async fn list_knowledge_documents(
    state: State<'_, AppState>,
) -> Result<Vec<KnowledgeDocumentSummary>, AppError> {
    let documents = state.kernel.list_documents().await?;
    tracing::info!(
        count = documents.len(),
        "knowledge documents served to webview"
    );
    Ok(documents)
}

#[tauri::command]
pub async fn search_knowledge(
    state: State<'_, AppState>,
    novel_id: String,
    query: String,
    limit: Option<i64>,
) -> Result<Vec<SearchHit>, AppError> {
    let tokens = tokenize(query.trim(), &Lexicon::default());
    let fts = fts_query(&tokens);
    if fts.is_empty() {
        return Ok(Vec::new());
    }
    state
        .kernel
        .search_keyword(&novel_id, &fts, limit.unwrap_or(8).clamp(1, 50))
        .await
        .map_err(AppError::from)
}

#[tauri::command]
pub async fn search_knowledge_vector(
    state: State<'_, AppState>,
    novel_id: String,
    model: String,
    vector: Vec<f32>,
    limit: Option<i64>,
) -> Result<Vec<SearchHit>, AppError> {
    if vector.is_empty() {
        return Err(AppError::validation("查询向量不能为空。"));
    }
    state
        .kernel
        .search_vector(
            &novel_id,
            &model,
            &vector,
            true,
            limit.unwrap_or(8).clamp(1, 50),
        )
        .await
        .map_err(AppError::from)
}

#[tauri::command]
pub async fn delete_knowledge_document(
    state: State<'_, AppState>,
    document_id: String,
) -> Result<bool, AppError> {
    state
        .kernel
        .delete_document(&document_id)
        .await
        .map_err(AppError::from)
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertCharacterStateRequest {
    pub id: String,
    pub character_id: String,
    pub chapter_order: i32,
    pub body: String,
    pub source_chapter_id: String,
    pub source_revision: String,
    pub status: String,
}

#[tauri::command]
pub async fn upsert_character_state(
    state: State<'_, AppState>,
    request: UpsertCharacterStateRequest,
) -> Result<(), AppError> {
    if !matches!(
        request.status.as_str(),
        "confirmed" | "proposed" | "disputed" | "superseded"
    ) {
        return Err(AppError::validation("人物状态类型不受支持。"));
    }
    state
        .kernel
        .upsert_character_state(novel_application::NewCharacterState {
            id: request.id,
            character_id: request.character_id,
            chapter_order: request.chapter_order,
            body: request.body,
            source_chapter_id: request.source_chapter_id,
            source_revision: request.source_revision,
            status: request.status,
            now_ms: novel_platform::now_ms(),
        })
        .await
        .map_err(AppError::from)
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertTimelineEventRequest {
    pub id: String,
    pub novel_id: String,
    pub chapter_order: i32,
    pub body: String,
    pub source_chapter_id: String,
    pub source_revision: String,
    pub status: String,
}

#[tauri::command]
pub async fn upsert_timeline_event(
    state: State<'_, AppState>,
    request: UpsertTimelineEventRequest,
) -> Result<(), AppError> {
    state
        .kernel
        .upsert_timeline_event(novel_application::NewTimelineEvent {
            id: request.id,
            novel_id: request.novel_id,
            chapter_order: request.chapter_order,
            body: request.body,
            source_chapter_id: request.source_chapter_id,
            source_revision: request.source_revision,
            status: request.status,
            now_ms: novel_platform::now_ms(),
        })
        .await
        .map_err(AppError::from)
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertPlotThreadRequest {
    pub id: String,
    pub novel_id: String,
    pub title: String,
    pub body: String,
    pub opened_chapter_order: i32,
    pub resolved_chapter_order: Option<i32>,
    pub status: String,
    pub source_chapter_id: String,
    pub source_revision: String,
}

#[tauri::command]
pub async fn upsert_plot_thread(
    state: State<'_, AppState>,
    request: UpsertPlotThreadRequest,
) -> Result<(), AppError> {
    state
        .kernel
        .upsert_plot_thread(novel_application::NewPlotThread {
            id: request.id,
            novel_id: request.novel_id,
            title: request.title,
            body: request.body,
            opened_chapter_order: request.opened_chapter_order,
            resolved_chapter_order: request.resolved_chapter_order,
            status: request.status,
            source_chapter_id: request.source_chapter_id,
            source_revision: request.source_revision,
            now_ms: novel_platform::now_ms(),
        })
        .await
        .map_err(AppError::from)
}

#[tauri::command]
pub async fn list_story_facts(
    state: State<'_, AppState>,
    novel_id: String,
    before_chapter_order: i32,
) -> Result<Vec<novel_application::StoryFactView>, AppError> {
    state
        .kernel
        .list_story_facts(&novel_id, before_chapter_order)
        .await
        .map_err(AppError::from)
}

#[tauri::command]
pub async fn retry_knowledge_document(
    state: State<'_, AppState>,
    document_id: String,
) -> Result<bool, AppError> {
    state
        .kernel
        .retry_index_document(&document_id)
        .await
        .map_err(AppError::from)
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractMemoryRequest {
    pub command_id: String,
    pub novel_id: String,
    pub chapter_id: String,
    pub provider: String,
    pub model: String,
    pub endpoint: String,
    pub allow_localhost_http: bool,
    pub token_budget: Option<u64>,
}

fn memory_service(state: &AppState) -> novel_application::MemoryService {
    novel_application::MemoryService::new(
        state.kernel.clone(),
        state.kernel.clone(),
        state.runtime.worker.llm(),
        state.runtime.worker.clock(),
        state.runtime.worker.ids(),
        state.runtime.worker.secrets(),
    )
}

#[tauri::command]
pub async fn extract_memory(
    state: State<'_, AppState>,
    request: ExtractMemoryRequest,
) -> Result<novel_application::MemoryExtractionJobView, AppError> {
    memory_service(&state)
        .extract(novel_application::ExtractMemoryInput {
            command_id: request.command_id,
            novel_id: request.novel_id,
            chapter_id: request.chapter_id,
            provider: request.provider,
            model: request.model,
            endpoint: request.endpoint,
            allow_localhost_http: request.allow_localhost_http,
            token_budget: request.token_budget,
        })
        .await
}

#[tauri::command]
pub async fn list_memory_candidates(
    state: State<'_, AppState>,
    novel_id: String,
    chapter_id: String,
) -> Result<Vec<novel_application::MemoryCandidateView>, AppError> {
    memory_service(&state)
        .list_candidates(&novel_id, &chapter_id)
        .await
}

#[tauri::command]
pub async fn confirm_memory_candidate(
    state: State<'_, AppState>,
    candidate_id: String,
    expected_version: i64,
) -> Result<novel_application::MemoryCandidateView, AppError> {
    memory_service(&state)
        .confirm(&candidate_id, expected_version)
        .await
}

#[tauri::command]
pub async fn reject_memory_candidate(
    state: State<'_, AppState>,
    candidate_id: String,
    expected_version: i64,
    reason: String,
) -> Result<novel_application::MemoryCandidateView, AppError> {
    memory_service(&state)
        .reject(&candidate_id, expected_version, &reason)
        .await
}

#[tauri::command]
pub async fn defer_memory_candidate(
    state: State<'_, AppState>,
    candidate_id: String,
    expected_version: i64,
) -> Result<novel_application::MemoryCandidateView, AppError> {
    memory_service(&state)
        .defer(&candidate_id, expected_version)
        .await
}

#[tauri::command]
pub async fn update_memory_candidate_value(
    state: State<'_, AppState>,
    candidate_id: String,
    expected_version: i64,
    value: String,
) -> Result<novel_application::MemoryCandidateView, AppError> {
    memory_service(&state)
        .update_value(&candidate_id, expected_version, &value)
        .await
}

#[tauri::command]
pub async fn get_knowledge_document(
    state: State<'_, AppState>,
    document_id: String,
) -> Result<Option<novel_application::KnowledgeDocumentBody>, AppError> {
    state
        .kernel
        .get_document_body(&document_id)
        .await
        .map_err(AppError::from)
}

#[tauri::command]
pub async fn sync_memory_conflicts(
    state: State<'_, AppState>,
    novel_id: String,
) -> Result<Vec<novel_application::MemoryConflictView>, AppError> {
    memory_service(&state).sync_conflicts(&novel_id).await
}

#[tauri::command]
pub async fn list_memory_conflicts(
    state: State<'_, AppState>,
    novel_id: String,
) -> Result<Vec<novel_application::MemoryConflictView>, AppError> {
    memory_service(&state).list_conflicts(&novel_id).await
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveMemoryConflictRequest {
    pub conflict_id: String,
    pub expected_version: i64,
    pub command_id: String,
    pub decision: String,
    pub chosen_candidate_id: Option<String>,
    pub reason: String,
    pub operator: String,
}

#[tauri::command]
pub async fn resolve_memory_conflict(
    state: State<'_, AppState>,
    request: ResolveMemoryConflictRequest,
) -> Result<novel_application::MemoryConflictView, AppError> {
    memory_service(&state)
        .resolve_conflict(
            &request.conflict_id,
            request.expected_version,
            &request.command_id,
            &request.decision,
            request.chosen_candidate_id.as_deref(),
            &request.reason,
            &request.operator,
        )
        .await
}

#[tauri::command]
pub async fn list_index_jobs(
    state: State<'_, AppState>,
    novel_id: String,
) -> Result<Vec<IndexJobView>, AppError> {
    state
        .kernel
        .list_index_jobs(&novel_id)
        .await
        .map_err(AppError::from)
}

#[tauri::command]
pub async fn recover_index_jobs(state: State<'_, AppState>) -> Result<Vec<IndexJobView>, AppError> {
    state
        .kernel
        .recover_expired_index_jobs(novel_platform::now_ms())
        .await
        .map_err(AppError::from)
}

#[tauri::command]
pub async fn retry_index_job(
    state: State<'_, AppState>,
    job_id: String,
) -> Result<IndexJobView, AppError> {
    let jobs = state
        .kernel
        .list_index_jobs("")
        .await
        .map_err(AppError::from)?;
    let _ = jobs;
    let now = novel_platform::now_ms();
    let _ = state.kernel.reset_index_job_for_retry(&job_id, now).await;
    let owner = format!("tauri-manual-{now}");
    let job = state
        .kernel
        .claim_index_job(&job_id, &owner, now + 60_000, now)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::conflict("索引任务不可执行或已被其他执行者领取。"))?;
    execute_index_job(&state, job, &owner).await
}

#[tauri::command]
pub async fn retry_knowledge_document_with_job(
    state: State<'_, AppState>,
    novel_id: String,
    document_id: String,
    source_revision: String,
    index_version: i64,
) -> Result<IndexJobView, AppError> {
    let now = novel_platform::now_ms();
    let job = state
        .kernel
        .enqueue_index_job(NewIndexJob {
            id: format!("index-job-{now}-{document_id}"),
            command_id: format!("retry-{document_id}-{now}"),
            novel_id,
            source_type: "knowledge_document".into(),
            source_id: document_id,
            source_revision,
            index_version,
            max_attempts: 5,
            now_ms: now,
        })
        .await
        .map_err(AppError::from)?;
    let owner = format!("tauri-manual-{now}");
    let claimed = state
        .kernel
        .claim_index_job(&job.id, &owner, now + 60_000, now)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::conflict("索引任务不可执行。"))?;
    execute_index_job(&state, claimed, &owner).await
}

pub(crate) async fn execute_index_job(
    state: &AppState,
    job: IndexJobView,
    owner: &str,
) -> Result<IndexJobView, AppError> {
    let now = novel_platform::now_ms();
    let current = state
        .kernel
        .get_document_body(&job.source_id)
        .await
        .map_err(AppError::from)?;
    let Some(current) = current else {
        return state
            .kernel
            .mark_index_job_obsolete(&job.id, owner, job.claim_epoch, "来源已删除", now)
            .await
            .map_err(AppError::from);
    };
    if !job.source_revision.is_empty()
        && current.source_revision.as_deref() != Some(job.source_revision.as_str())
    {
        return state
            .kernel
            .mark_index_job_obsolete(&job.id, owner, job.claim_epoch, "来源 revision 已更新", now)
            .await
            .map_err(AppError::from);
    }
    match state.kernel.retry_index_document(&job.source_id).await {
        Ok(true) => {
            state
                .kernel
                .complete_index_job(&job.id, owner, job.claim_epoch, now)
                .await
                .map_err(AppError::from)?;
        }
        Ok(false) => {
            return state
                .kernel
                .mark_index_job_obsolete(
                    &job.id,
                    owner,
                    job.claim_epoch,
                    "索引已是就绪状态但请求未执行",
                    now,
                )
                .await
                .map_err(AppError::from);
        }
        Err(error) => {
            return state
                .kernel
                .fail_index_job(
                    &job.id,
                    owner,
                    job.claim_epoch,
                    "index_error",
                    &error.to_string(),
                    now + job.attempt_count.max(1) * 1_000,
                    now,
                )
                .await
                .map_err(AppError::from);
        }
    }
    let jobs = state
        .kernel
        .list_index_jobs(&job.novel_id)
        .await
        .map_err(AppError::from)?;
    jobs.into_iter()
        .find(|current| current.id == job.id)
        .ok_or_else(|| AppError::not_found("索引任务不存在。"))
}
