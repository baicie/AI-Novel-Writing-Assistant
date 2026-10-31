use std::collections::BTreeMap;
use std::time::Duration;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Sse, sse::Event};
use futures::Stream;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::time::sleep;

use novel_application::{GenerationEvent, OpenRunInput};

use crate::error::{HttpError, HttpResult};
use crate::state::KernelState;

pub async fn list_runs(
    State(state): State<KernelState>,
    Path(novel_id): Path<String>,
) -> HttpResult<Json<Value>> {
    let runs = state
        .runtime
        .director
        .list_runs(&novel_id)
        .await
        .map_err(HttpError::from)?;
    Ok(Json(json!({
        "success": true,
        "data": {
            "items": runs,
            "total": runs.len() as i64,
        },
    })))
}

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
            context_token_budget: payload.context_token_budget,
            max_automatic_retries: payload.max_automatic_retries,
            repair_budget: payload.repair_budget,
            endpoint: payload.endpoint,
            allow_localhost_http: payload.allow_localhost_http,
            // Web 端没有模型注册表上下文，按单一路由执行。
            step_routes: Default::default(),
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
) -> HttpResult<impl IntoResponse> {
    if query.stream {
        // SSE 流式响应
        sse_stream(state, run_id, query.after_seq)
            .await
            .map(|stream| stream.into_response())
    } else {
        // JSON 数组响应（兼容旧模式）
        json_replay(state, run_id, query.after_seq)
            .await
            .map(|json| json.into_response())
    }
}

pub async fn confirm_event(
    State(_state): State<KernelState>,
    Path(_run_id): Path<String>,
    Json(payload): Json<ConfirmEventBody>,
) -> HttpResult<Json<Value>> {
    // 当前阶段简单记录和返回成功
    // 后续可扩展为：
    // 1. 将 last_confirmed_seq 记录到数据库用于崩溃恢复
    // 2. 更新内存状态追踪客户端进度

    // 基础验证
    if payload.last_confirmed_seq < 0 {
        return Err(HttpError::bad_request("序号不能为负数"));
    }

    Ok(Json(json!({
        "success": true,
        "data": {
            "confirmed": true,
            "last_confirmed_seq": payload.last_confirmed_seq,
        },
    })))
}

async fn json_replay(
    state: KernelState,
    run_id: String,
    after_seq: i64,
) -> HttpResult<Json<Value>> {
    let events: Vec<GenerationEvent> = state
        .runtime
        .director
        .replay(&run_id, after_seq)
        .await
        .map_err(HttpError::from)?;
    Ok(Json(json!({
        "success": true,
        "data": events,
    })))
}

async fn sse_stream(
    state: KernelState,
    run_id: String,
    after_seq: i64,
) -> HttpResult<Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>>> {
    // 查询初始事件
    let initial_events = state
        .runtime
        .director
        .replay(&run_id, after_seq)
        .await
        .map_err(HttpError::from)?;

    let run_id_for_polling = run_id.clone();
    let runtime = std::sync::Arc::new(state.runtime);

    // 使用 async_stream 宏创建流
    let stream = async_stream::stream! {
        // 1. 发送历史事件
        for event in initial_events.iter() {
            if let Ok(json) = serde_json::to_string(&event) {
                yield Ok(Event::default().data(json));
            }
        }

        // 2. 计算最后一条历史事件的序号
        let mut last_seq = initial_events
            .last()
            .and_then(|e| e.seq())
            .unwrap_or(after_seq);

        // 3. 进入长轮询模式
        loop {
            sleep(Duration::from_millis(500)).await;

            match runtime.director.replay(&run_id_for_polling, last_seq).await {
                Ok(new_events) => {
                    if !new_events.is_empty() {
                        for event in &new_events {
                            if let Ok(json) = serde_json::to_string(&event) {
                                yield Ok(Event::default().data(json));
                            }
                            // 更新最后序号
                            if let Some(seq) = event.seq() {
                                last_seq = seq;
                            }
                        }
                    }
                }
                Err(_) => {
                    // 错误时停止轮询
                    break;
                }
            }
        }
    };

    Ok(Sse::new(stream))
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
    pub context_token_budget: Option<u64>,
    pub max_automatic_retries: u8,
    pub repair_budget: Option<u8>,
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
    #[serde(default)]
    pub stream: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmEventBody {
    /// 前端确认的最后一条事件序号
    pub last_confirmed_seq: i64,
}
