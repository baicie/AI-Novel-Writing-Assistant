use novel_application::{AppError, PortError};
use novel_domain::{FirstNovelOnboarding, Genre, QuickSetupStatus, StoryMode, World};
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
pub async fn get_quick_setup_status(
    state: State<'_, AppState>,
) -> Result<QuickSetupStatus, AppError> {
    let legacy = state.runtime.setup.get_quick_setup_status().await?;
    if legacy.ready_for_creation {
        return Ok(legacy);
    }
    // 模型来源注册表里已经有「有密钥 + 有模型」的来源时，同样算配置完成。
    let ready = crate::commands::provider::service(&state)
        .ready_provider()
        .await
        .map_err(crate::commands::provider::to_app_error)?;
    Ok(match ready {
        Some((endpoint, model, allow_localhost_http)) => QuickSetupStatus {
            ready_for_creation: true,
            model_configured: true,
            api_key_present: true,
            endpoint: Some(endpoint),
            model: Some(model),
            allow_localhost_http,
            next_step: "可以创建第一本书并开始创作。".to_string(),
        },
        None => legacy,
    })
}

#[tauri::command]
pub async fn get_first_novel_onboarding(
    state: State<'_, AppState>,
) -> Result<FirstNovelOnboarding, AppError> {
    state.runtime.setup.get_first_novel_onboarding().await
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
