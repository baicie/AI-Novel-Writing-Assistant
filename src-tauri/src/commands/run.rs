use std::collections::BTreeMap;

use novel_application::{AppError, CommandOutcome, GenerationEvent, OpenRunInput, RunSummary};
use serde::Deserialize;
use tauri::State;
use tauri::ipc::Channel;

use crate::state::AppState;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenRunRequest {
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

#[tauri::command]
pub async fn list_all_runs(state: State<'_, AppState>) -> Result<Vec<RunSummary>, AppError> {
    let runs = state.runtime.director.list_all_runs().await?;
    tracing::info!(count = runs.len(), "all runs served to webview");
    Ok(runs)
}

#[tauri::command]
pub async fn list_runs(
    state: State<'_, AppState>,
    novel_id: String,
) -> Result<Vec<RunSummary>, AppError> {
    state.runtime.director.list_runs(&novel_id).await
}

#[tauri::command]
pub async fn cancel_generation(state: State<'_, AppState>, run_id: String) -> Result<(), AppError> {
    state.runtime.worker.cancel_generation(&run_id);
    Ok(())
}

#[tauri::command]
pub async fn open_run(
    state: State<'_, AppState>,
    request: OpenRunRequest,
) -> Result<CommandOutcome, AppError> {
    state
        .runtime
        .director
        .open_run(OpenRunInput {
            command_id: request.command_id,
            novel_id: request.novel_id,
            target_chapter_id: request.target_chapter_id,
            provider: request.provider,
            model: request.model,
            prompt_versions: request.prompt_versions,
            token_budget: request.token_budget,
            max_automatic_retries: request.max_automatic_retries,
            endpoint: request.endpoint,
            allow_localhost_http: request.allow_localhost_http,
        })
        .await
}

#[tauri::command]
pub async fn approve_run(
    state: State<'_, AppState>,
    command_id: String,
    run_id: String,
) -> Result<CommandOutcome, AppError> {
    state.runtime.director.approve(&command_id, &run_id).await
}

#[tauri::command]
pub async fn cancel_run(
    state: State<'_, AppState>,
    command_id: String,
    run_id: String,
) -> Result<CommandOutcome, AppError> {
    state.runtime.director.cancel(&command_id, &run_id).await
}

#[tauri::command]
pub async fn resume_run(
    state: State<'_, AppState>,
    command_id: String,
    run_id: String,
    accept_candidate: bool,
) -> Result<CommandOutcome, AppError> {
    state
        .runtime
        .director
        .resume(&command_id, &run_id, accept_candidate)
        .await
}

#[tauri::command]
pub async fn replay_run(
    state: State<'_, AppState>,
    run_id: String,
    after_seq: i64,
) -> Result<Vec<GenerationEvent>, AppError> {
    state.runtime.director.replay(&run_id, after_seq).await
}

#[tauri::command]
pub async fn subscribe_run(
    state: State<'_, AppState>,
    run_id: String,
    channel: Channel<GenerationEvent>,
) -> Result<(), AppError> {
    let mut receiver = state.runtime.hub.subscribe();
    tauri::async_runtime::spawn(async move {
        loop {
            match receiver.recv().await {
                Ok(event) if event.run_id() == run_id => {
                    if channel.send(event).is_err() {
                        break;
                    }
                }
                Ok(_) => {}
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
    Ok(())
}
