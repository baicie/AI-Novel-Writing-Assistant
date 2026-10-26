use novel_application::{AppError, NewStructuredFact, StructuredFactView};
use tauri::State;

use crate::state::AppState;

#[tauri::command]
pub async fn list_structured_facts(
    state: State<'_, AppState>,
    novel_id: String,
    entity_id: Option<String>,
    as_of_chapter_order: Option<i64>,
) -> Result<Vec<StructuredFactView>, AppError> {
    state
        .structured_memory
        .list(&novel_id, entity_id, as_of_chapter_order)
        .await
}

#[tauri::command]
pub async fn confirm_structured_fact(
    state: State<'_, AppState>,
    fact: NewStructuredFact,
) -> Result<StructuredFactView, AppError> {
    state.structured_memory.confirm(fact).await
}
