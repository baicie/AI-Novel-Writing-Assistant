use novel_application::{AppError, RecoverableTaskListResponse, TaskOverviewSummary};
use tauri::State;

use crate::state::AppState;

#[tauri::command]
pub async fn get_task_overview(
    state: State<'_, AppState>,
) -> Result<TaskOverviewSummary, AppError> {
    state.runtime.tasks.get_overview().await
}

#[tauri::command]
pub async fn list_recovery_candidates(
    state: State<'_, AppState>,
) -> Result<RecoverableTaskListResponse, AppError> {
    state.runtime.tasks.list_recovery_candidates().await
}
