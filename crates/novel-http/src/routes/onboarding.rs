use axum::Json;
use axum::extract::State;
use serde_json::{Value, json};

use crate::error::{HttpError, HttpResult};
use crate::state::KernelState;

pub async fn first_novel(State(state): State<KernelState>) -> HttpResult<Json<Value>> {
    let projection = state
        .runtime
        .setup
        .get_first_novel_onboarding()
        .await
        .map_err(HttpError::from)?;
    Ok(Json(json!({
        "success": true,
        "data": projection,
        "message": "第一本书创作进度已更新。",
    })))
}

pub async fn quick_setup_status(State(state): State<KernelState>) -> HttpResult<Json<Value>> {
    let status = state
        .runtime
        .setup
        .get_quick_setup_status()
        .await
        .map_err(HttpError::from)?;
    Ok(Json(json!({
        "success": true,
        "data": status,
        "message": if status.ready_for_creation {
            "创作环境可用。"
        } else {
            "还需要完成创作环境配置。"
        },
    })))
}

pub async fn list_genres(State(state): State<KernelState>) -> HttpResult<Json<Value>> {
    let genres = state
        .runtime
        .setup
        .list_genres()
        .await
        .map_err(HttpError::from)?;
    Ok(Json(json!({
        "success": true,
        "data": genres,
    })))
}

pub async fn list_story_modes(State(state): State<KernelState>) -> HttpResult<Json<Value>> {
    let modes = state
        .runtime
        .setup
        .list_story_modes()
        .await
        .map_err(HttpError::from)?;
    Ok(Json(json!({
        "success": true,
        "data": modes,
    })))
}
