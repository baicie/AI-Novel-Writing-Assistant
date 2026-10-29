use axum::Json;
use axum::extract::State;
use serde::Serialize;
use serde_json::{Value, json};

use crate::error::{HttpError, HttpResult};
use crate::state::KernelState;

pub async fn get(State(state): State<KernelState>) -> HttpResult<Json<Value>> {
    let view = read_view(&state)
        .await
        .map_err(|error| HttpError::internal(error))?;
    Ok(Json(json!({
        "success": true,
        "data": view,
    })))
}

pub async fn save(
    State(state): State<KernelState>,
    Json(payload): Json<SaveModelSetupBody>,
) -> HttpResult<Json<Value>> {
    let endpoint = payload.endpoint.trim();
    let model = payload.model.trim();
    let api_key = payload.api_key.trim();
    if endpoint.is_empty() || model.is_empty() {
        return Err(HttpError::bad_request("请填写完整的接口地址和模型名称。"));
    }
    let now = novel_platform::now_ms();
    let secret_ref = novel_storage_sqlite::MODEL_SECRET_REF.to_string();
    state
        .secrets
        .put(&secret_ref, api_key)
        .await
        .map_err(|error| HttpError::internal(format!("无法写入系统凭据：{error}")))?;
    state
        .kernel
        .save_model_setup(endpoint, model, payload.allow_localhost_http, now)
        .await
        .map_err(|error| HttpError::internal(format!("无法保存模型配置：{error}")))?;
    let view = read_view(&state)
        .await
        .map_err(|error| HttpError::internal(error))?;
    Ok(Json(json!({
        "success": true,
        "data": view,
        "message": "已保存到本地创作服务。",
    })))
}

async fn read_view(state: &KernelState) -> Result<Option<ModelSetupView>, String> {
    let stored = state
        .kernel
        .load_model_setup()
        .await
        .map_err(|error| error.to_string())?;
    let Some(stored) = stored else {
        return Ok(None);
    };
    let has_api_key = state
        .secrets
        .get(&stored.secret_ref)
        .await
        .map(|value| value.is_some())
        .unwrap_or(false);
    Ok(Some(ModelSetupView {
        endpoint: stored.endpoint,
        model: stored.model,
        allow_localhost_http: stored.allow_localhost_http,
        has_api_key,
    }))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelSetupView {
    pub endpoint: String,
    pub model: String,
    pub allow_localhost_http: bool,
    pub has_api_key: bool,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveModelSetupBody {
    pub endpoint: String,
    pub model: String,
    pub allow_localhost_http: bool,
    pub api_key: String,
}
