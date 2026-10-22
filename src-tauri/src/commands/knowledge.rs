use novel_application::{AppError, KnowledgeRepository, NewDocument, NewEmbedding, SearchHit};
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
            embedding,
        })
        .await
        .map_err(AppError::from)
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
