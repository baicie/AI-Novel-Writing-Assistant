use std::collections::BTreeMap;

use novel_application::{
    AppError, CommandOutcome, GenerationEvent, OpenRunInput, RunDetailView, RunSummary,
};
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
    pub context_token_budget: Option<u64>,
    pub max_automatic_retries: u8,
    pub repair_budget: Option<u8>,
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
pub async fn get_active_run(
    state: State<'_, AppState>,
    novel_id: String,
) -> Result<Option<RunSummary>, AppError> {
    state.runtime.director.active_run(&novel_id).await
}

#[tauri::command]
pub async fn get_run_detail(
    state: State<'_, AppState>,
    novel_id: String,
    run_id: String,
) -> Result<RunDetailView, AppError> {
    state
        .runtime
        .director
        .get_run_detail(&novel_id, &run_id)
        .await
}

#[tauri::command]
pub async fn smoke_channel_message(channel: Channel<GenerationEvent>) -> Result<(), AppError> {
    let event = GenerationEvent::RunStarted {
        run_id: "smoke-run".into(),
        seq: 1,
        occurred_at: novel_platform::now_ms(),
    };
    channel
        .send(event)
        .map_err(|_| AppError::integrity("测试 Channel 消息发送失败。"))
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
            context_token_budget: request.context_token_budget,
            max_automatic_retries: request.max_automatic_retries,
            repair_budget: request.repair_budget,
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
    expected_version: Option<i64>,
) -> Result<CommandOutcome, AppError> {
    match expected_version {
        Some(version) => {
            state
                .runtime
                .director
                .approve_expected(&command_id, &run_id, version)
                .await
        }
        None => state.runtime.director.approve(&command_id, &run_id).await,
    }
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
    let runtime = state.runtime.clone();
    let mut receiver = state.runtime.hub.subscribe();
    tauri::async_runtime::spawn(async move {
        let mut last_seq = 0_i64;
        loop {
            match receiver.recv().await {
                Ok(event) if event.run_id() == run_id => {
                    let terminal = matches!(
                        event,
                        GenerationEvent::RunCompleted { .. }
                            | GenerationEvent::RunFailed { .. }
                            | GenerationEvent::RunCancelled { .. }
                    );
                    if let Some(seq) = event.seq() {
                        last_seq = last_seq.max(seq);
                    }
                    if channel.send(event).is_err() || terminal {
                        break;
                    }
                }
                Ok(_) => {}
                Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                    // The in-memory broadcast is allowed to drop messages. Recover the
                    // authoritative suffix from SQLite instead of silently losing a
                    // checkpoint/terminal event in the WebView.
                    tracing::warn!(run_id = %run_id, skipped, from_seq = last_seq, "run event channel lagged; replaying persisted events");
                    match runtime.director.replay(&run_id, last_seq).await {
                        Ok(events) => {
                            for event in events {
                                if let Some(seq) = event.seq() {
                                    last_seq = last_seq.max(seq);
                                }
                                if channel.send(event).is_err() {
                                    return;
                                }
                            }
                        }
                        Err(error) => {
                            tracing::error!(run_id = %run_id, %error, "failed to recover lagged run events");
                            break;
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
    Ok(())
}

#[tauri::command]
pub async fn reject_run(
    state: State<'_, AppState>,
    command_id: String,
    run_id: String,
    expected_version: Option<i64>,
) -> Result<CommandOutcome, AppError> {
    state
        .runtime
        .director
        .reject(&command_id, &run_id, expected_version)
        .await
}

#[tauri::command]
pub async fn regenerate_run(
    state: State<'_, AppState>,
    command_id: String,
    run_id: String,
    expected_version: Option<i64>,
) -> Result<CommandOutcome, AppError> {
    state
        .runtime
        .director
        .regenerate(&command_id, &run_id, expected_version)
        .await
}

#[tauri::command]
pub async fn confirm_after_edit_run(
    state: State<'_, AppState>,
    command_id: String,
    run_id: String,
    expected_version: Option<i64>,
    payload_json: String,
) -> Result<CommandOutcome, AppError> {
    state
        .runtime
        .director
        .confirm_after_edit(&command_id, &run_id, expected_version, &payload_json)
        .await
}

#[tauri::command]
pub async fn confirm_candidate_after_edit_run(
    state: State<'_, AppState>,
    command_id: String,
    run_id: String,
    expected_version: Option<i64>,
    base_candidate_id: String,
    base_content_sha256: String,
    content: String,
) -> Result<CommandOutcome, AppError> {
    state
        .runtime
        .director
        .confirm_candidate_after_edit(
            &command_id,
            &run_id,
            expected_version,
            &base_candidate_id,
            &base_content_sha256,
            &content,
        )
        .await
}

#[tauri::command]
pub async fn get_candidate_edit_draft(
    state: State<'_, AppState>,
    run_id: String,
    gate_id: String,
    client_id: String,
) -> Result<Option<novel_application::CandidateEditDraftView>, AppError> {
    state
        .runtime
        .director
        .get_candidate_edit_draft(&run_id, &gate_id, &client_id)
        .await
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveCandidateEditDraftRequest {
    pub novel_id: String,
    pub chapter_id: String,
    pub run_id: String,
    pub gate_id: String,
    pub client_id: String,
    pub base_candidate_id: String,
    pub base_content_sha256: String,
    pub content: String,
    pub control_version: i64,
    pub expected_revision: i64,
}

#[tauri::command]
pub async fn save_candidate_edit_draft(
    state: State<'_, AppState>,
    request: SaveCandidateEditDraftRequest,
) -> Result<novel_application::CandidateEditDraftView, AppError> {
    state
        .runtime
        .director
        .save_candidate_edit_draft(
            &request.novel_id,
            &request.chapter_id,
            &request.run_id,
            &request.gate_id,
            &request.client_id,
            &request.base_candidate_id,
            &request.base_content_sha256,
            &request.content,
            request.control_version,
            request.expected_revision,
        )
        .await
}

#[tauri::command]
pub async fn clear_candidate_edit_draft(
    state: State<'_, AppState>,
    run_id: String,
    gate_id: String,
    client_id: String,
    content: String,
    local_revision: i64,
) -> Result<bool, AppError> {
    state
        .runtime
        .director
        .clear_candidate_edit_draft(&run_id, &gate_id, &client_id, &content, local_revision)
        .await
}
