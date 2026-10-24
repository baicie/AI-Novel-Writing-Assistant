use novel_application::{
    AppError, KnowledgeDocumentSummary, KnowledgeRepository, NewDocument, NewEmbedding,
    NovelRepository, SearchHit,
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
