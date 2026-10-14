use axum::Json;
use axum::extract::{Path, State};
use serde::Deserialize;
use serde_json::{Value, json};

use novel_application::UpdateWorldFields;

use crate::error::{HttpError, HttpResult};
use crate::state::KernelState;

pub async fn list(State(state): State<KernelState>) -> HttpResult<Json<Value>> {
    let worlds = state.runtime.setup.list_worlds().await.map_err(HttpError::from)?;
    Ok(Json(json!({
        "success": true,
        "data": worlds,
        "message": "World list loaded.",
    })))
}

pub async fn create(
    State(state): State<KernelState>,
    Json(payload): Json<CreateWorldBody>,
) -> HttpResult<Json<Value>> {
    let world = state
        .runtime
        .setup
        .create_world(
            &payload.name,
            payload.description.as_deref().unwrap_or(""),
            payload.overview_summary.as_deref().unwrap_or(""),
            payload.source_template_id,
            payload.owner_novel_id,
        )
        .await
        .map_err(HttpError::from)?;
    Ok(Json(json!({
        "success": true,
        "data": world,
        "message": "World created.",
    })))
}

pub async fn detail(
    State(state): State<KernelState>,
    Path(id): Path<String>,
) -> HttpResult<Json<Value>> {
    let world = state.runtime.setup.get_world(&id).await.map_err(HttpError::from)?;
    Ok(Json(json!({
        "success": true,
        "data": world,
        "message": "World loaded.",
    })))
}

pub async fn update(
    State(state): State<KernelState>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateWorldBody>,
) -> HttpResult<Json<Value>> {
    let fields = UpdateWorldFields {
        name: payload.name,
        description: payload.description.map(Some),
        overview_summary: payload.overview_summary.map(Some),
        source_template_id: payload.source_template_id.map(Some),
        owner_novel_id: payload.owner_novel_id.map(Some),
        status: payload.status,
    };
    let world = state
        .runtime
        .setup
        .update_world(&id, &fields)
        .await
        .map_err(HttpError::from)?;
    Ok(Json(json!({
        "success": true,
        "data": world,
        "message": "World updated.",
    })))
}

pub async fn delete(
    State(state): State<KernelState>,
    Path(id): Path<String>,
) -> HttpResult<Json<Value>> {
    state
        .runtime
        .setup
        .delete_world(&id)
        .await
        .map_err(HttpError::from)?;
    Ok(Json(json!({
        "success": true,
        "data": null,
        "message": "World deleted.",
    })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorldBody {
    pub name: String,
    pub description: Option<String>,
    pub overview_summary: Option<String>,
    pub source_template_id: Option<String>,
    pub owner_novel_id: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateWorldBody {
    pub name: Option<String>,
    pub description: Option<String>,
    pub overview_summary: Option<String>,
    pub source_template_id: Option<String>,
    pub owner_novel_id: Option<String>,
    pub status: Option<String>,
}
