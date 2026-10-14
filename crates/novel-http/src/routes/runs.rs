use std::collections::BTreeMap;

use axum::Json;
use axum::extract::{Path, Query, State};
use serde::Deserialize;
use serde_json::{Value, json};

use novel_application::{GenerationEvent, OpenRunInput};

use crate::error::{HttpError, HttpResult};
use crate::state::KernelState;

pub async fn open(
    State(state): State<KernelState>,
    Path(novel_id): Path<String>,
    Json(payload): Json<OpenRunBody>,
) -> HttpResult<Json<Value>> {
    let outcome = state
        .runtime
        .director
        .open_run(OpenRunInput {
            command_id: payload.command_id,
            novel_id,
            target_chapter_id: payload.target_chapter_id,
            provider: payload.provider,
            model: payload.model,
            prompt_versions: payload.prompt_versions,
            token_budget: payload.token_budget,
            max_automatic_retries: payload.max_automatic_retries,
            endpoint: payload.endpoint,
            allow_localhost_http: payload.allow_localhost_http,
        })
        .await
        .map_err(HttpError::from)?;
    Ok(Json(json!({
        "success": true,
        "data": outcome,
    })))
}

pub async fn approve(
    State(state): State<KernelState>,
    Path(run_id): Path<String>,
    Json(payload): Json<CommandBody>,
) -> HttpResult<Json<Value>> {
    let outcome = state
        .runtime
        .director
        .approve(&payload.command_id, &run_id)
        .await
        .map_err(HttpError::from)?;
    Ok(Json(json!({ "success": true, "data": outcome })))
}

pub async fn cancel(
    State(state): State<KernelState>,
    Path(run_id): Path<String>,
    Json(payload): Json<CommandBody>,
) -> HttpResult<Json<Value>> {
    let outcome = state
        .runtime
        .director
        .cancel(&payload.command_id, &run_id)
        .await
        .map_err(HttpError::from)?;
    Ok(Json(json!({ "success": true, "data": outcome })))
}

pub async fn resume(
    State(state): State<KernelState>,
    Path(run_id): Path<String>,
    Json(payload): Json<ResumeBody>,
) -> HttpResult<Json<Value>> {
    let outcome = state
        .runtime
        .director
        .resume(&payload.command_id, &run_id, payload.accept_candidate)
        .await
        .map_err(HttpError::from)?;
    Ok(Json(json!({ "success": true, "data": outcome })))
}

pub async fn replay(
    State(state): State<KernelState>,
    Path(run_id): Path<String>,
    Query(query): Query<ReplayQuery>,
) -> HttpResult<Json<Value>> {
    let events: Vec<GenerationEvent> = state
        .runtime
        .director
        .replay(&run_id, query.after_seq)
        .await
        .map_err(HttpError::from)?;
    Ok(Json(json!({
        "success": true,
        "data": events,
    })))
}

pub async fn cancel_generation(
    State(state): State<KernelState>,
    Path(run_id): Path<String>,
) -> HttpResult<Json<Value>> {
    state.runtime.worker.cancel_generation(&run_id);
    Ok(Json(json!({
        "success": true,
        "data": null,
        "message": "已请求取消当前生成。",
    })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenRunBody {
    pub command_id: String,
    pub target_chapter_id: String,
    pub provider: String,
    pub model: String,
    pub prompt_versions: BTreeMap<String, String>,
    pub token_budget: Option<u64>,
    pub max_automatic_retries: u8,
    pub endpoint: String,
    pub allow_localhost_http: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandBody {
    pub command_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeBody {
    pub command_id: String,
    pub accept_candidate: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayQuery {
    #[serde(default)]
    pub after_seq: i64,
}
