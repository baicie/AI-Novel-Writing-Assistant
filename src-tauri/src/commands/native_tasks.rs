use novel_application::{AppError, NativeTaskQuery, NativeTaskSummary};
use tauri::State;

use crate::state::AppState;

#[tauri::command]
pub async fn list_native_tasks(
    state: State<'_, AppState>,
    novel_id: Option<String>,
    task_type: Option<String>,
    status: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<NativeTaskSummary>, AppError> {
    state
        .native_tasks
        .list(NativeTaskQuery {
            novel_id,
            task_type,
            status,
            limit: limit.unwrap_or(100),
            offset: offset.unwrap_or(0),
        })
        .await
}
