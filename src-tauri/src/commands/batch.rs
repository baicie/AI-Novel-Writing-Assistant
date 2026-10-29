use std::collections::BTreeMap;

use novel_application::{AppError, BatchRunRequest, BatchRunView};
use tauri::State;

use crate::state::AppState;

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBatchRequest {
    pub command_id: String,
    pub novel_id: String,
    pub chapter_ids: Vec<String>,
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

#[tauri::command]
pub async fn create_batch(
    state: State<'_, AppState>,
    request: CreateBatchRequest,
) -> Result<BatchRunView, AppError> {
    let chapters = state
        .runtime
        .director
        .list_chapters(&request.novel_id)
        .await?;
    if chapters.is_empty() {
        return Err(AppError::validation("这本小说还没有章节。"));
    }
    for chapter_id in &request.chapter_ids {
        if !chapters.iter().any(|chapter| &chapter.id == chapter_id) {
            return Err(AppError::validation("批量章节不属于这本小说。"));
        }
    }
    let batch_request = BatchRunRequest {
        provider: request.provider,
        model: request.model,
        prompt_versions: request.prompt_versions,
        token_budget: request.token_budget,
        context_token_budget: request.context_token_budget,
        max_automatic_retries: request.max_automatic_retries,
        repair_budget: request.repair_budget,
        endpoint: request.endpoint,
        allow_localhost_http: request.allow_localhost_http,
    };
    state
        .batch
        .create(
            &request.novel_id,
            &request.chapter_ids,
            batch_request,
            &request.command_id,
            novel_platform::now_ms(),
        )
        .await
}

#[tauri::command]
pub async fn list_batches(
    state: State<'_, AppState>,
    novel_id: String,
) -> Result<Vec<BatchRunView>, AppError> {
    state.batch.list(&novel_id).await
}

#[tauri::command]
pub async fn get_batch(
    state: State<'_, AppState>,
    batch_id: String,
) -> Result<BatchRunView, AppError> {
    state.batch.get(&batch_id).await
}

#[tauri::command]
pub async fn pause_batch(
    state: State<'_, AppState>,
    batch_id: String,
    expected_version: i64,
) -> Result<BatchRunView, AppError> {
    state
        .batch
        .pause(&batch_id, expected_version, novel_platform::now_ms())
        .await
}

#[tauri::command]
pub async fn resume_batch(
    state: State<'_, AppState>,
    batch_id: String,
    expected_version: i64,
) -> Result<BatchRunView, AppError> {
    state
        .batch
        .resume(&batch_id, expected_version, novel_platform::now_ms())
        .await
}

#[tauri::command]
pub async fn cancel_batch(
    state: State<'_, AppState>,
    batch_id: String,
    expected_version: i64,
) -> Result<BatchRunView, AppError> {
    state
        .batch
        .cancel(
            &state.runtime.director,
            &batch_id,
            expected_version,
            novel_platform::now_ms(),
        )
        .await
}

#[tauri::command]
pub async fn tick_batches(state: State<'_, AppState>) -> Result<Vec<BatchRunView>, AppError> {
    state
        .batch
        .tick(
            &state.runtime.director,
            &format!("manual-batch-{}", std::process::id()),
            novel_platform::now_ms(),
        )
        .await
}
