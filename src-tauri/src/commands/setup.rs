use novel_application::{AppError, PortError};
use novel_domain::{Genre, StoryMode, World};
use tauri::State;

use crate::state::AppState;

#[tauri::command]
pub async fn list_genres(state: State<'_, AppState>) -> Result<Vec<Genre>, AppError> {
    state.runtime.setup.list_genres().await
}

#[tauri::command]
pub async fn list_story_modes(state: State<'_, AppState>) -> Result<Vec<StoryMode>, AppError> {
    state.runtime.setup.list_story_modes().await
}

#[tauri::command]
pub async fn list_worlds(state: State<'_, AppState>) -> Result<Vec<World>, AppError> {
    state.runtime.setup.list_worlds().await
}

#[tauri::command]
pub async fn create_world(
    state: State<'_, AppState>,
    name: String,
    description: String,
    overview_summary: String,
) -> Result<World, AppError> {
    state
        .runtime
        .setup
        .create_world(&name, &description, &overview_summary, None, None)
        .await
}

#[tauri::command]
pub async fn delete_world(state: State<'_, AppState>, world_id: String) -> Result<(), AppError> {
    let worlds = state.runtime.setup.list_worlds().await?;
    if !worlds.iter().any(|world| world.id == world_id) {
        return Err(AppError::from(PortError::NotFound(
            "没有找到这个世界观。".into(),
        )));
    }
    state.runtime.setup.delete_world(&world_id).await
}
