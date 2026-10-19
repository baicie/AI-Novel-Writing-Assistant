use axum::Json;
use axum::extract::{Path, State};
use serde_json::{Value, json};

use novel_application::ChapterSnapshot;

use crate::error::{HttpError, HttpResult};
use crate::state::KernelState;

pub async fn list(
    State(state): State<KernelState>,
    Path(novel_id): Path<String>,
) -> HttpResult<Json<Value>> {
    let chapters = state
        .runtime
        .director
        .list_chapters(&novel_id)
        .await
        .map_err(HttpError::from)?;
    Ok(Json(json!({
        "success": true,
        "data": to_dto(chapters),
    })))
}

pub async fn full_detail(
    State(state): State<KernelState>,
    Path((novel_id, chapter_id)): Path<(String, String)>,
) -> HttpResult<Json<Value>> {
    let chapter = state
        .runtime
        .director
        .get_chapter_full(&novel_id, &chapter_id)
        .await
        .map_err(HttpError::from)?;
    Ok(Json(json!({
        "success": true,
        "data": chapter,
    })))
}

pub async fn create(
    State(state): State<KernelState>,
    Path(novel_id): Path<String>,
    Json(payload): Json<CreateChapterBody>,
) -> HttpResult<Json<Value>> {
    let chapter = state
        .runtime
        .director
        .create_chapter(&novel_id, &payload.title, payload.sort_order)
        .await
        .map_err(HttpError::from)?;
    Ok(Json(json!({
        "success": true,
        "data": to_dto(vec![chapter]),
    })))
}

pub async fn detail(
    State(state): State<KernelState>,
    Path((novel_id, chapter_id)): Path<(String, String)>,
) -> HttpResult<Json<Value>> {
    let chapter = state
        .runtime
        .director
        .get_chapter(&novel_id, &chapter_id)
        .await
        .map_err(HttpError::from)?;
    Ok(Json(json!({
        "success": true,
        "data": to_dto(vec![chapter]),
    })))
}

pub async fn save(
    State(state): State<KernelState>,
    Path((novel_id, chapter_id)): Path<(String, String)>,
    Json(payload): Json<SaveChapterBody>,
) -> HttpResult<Json<Value>> {
    let chapter = state
        .runtime
        .director
        .save_chapter(
            &novel_id,
            &chapter_id,
            payload.expected_version,
            &payload.title,
            &payload.content,
        )
        .await
        .map_err(HttpError::from)?;
    Ok(Json(json!({
        "success": true,
        "data": to_dto(vec![chapter]),
    })))
}

fn to_dto(chapters: Vec<ChapterSnapshot>) -> Value {
    serde_json::to_value(chapters).unwrap_or(Value::Null)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateChapterBody {
    pub title: String,
    pub sort_order: i32,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveChapterBody {
    pub expected_version: i64,
    pub title: String,
    pub content: String,
}
