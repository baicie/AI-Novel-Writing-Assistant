use axum::Json;
use axum::extract::State;
use serde_json::{Value, json};

use crate::error::HttpResult;
use crate::state::KernelState;

pub async fn health(State(state): State<KernelState>) -> HttpResult<Json<Value>> {
    Ok(Json(json!({
        "success": true,
        "data": {
            "status": "ok",
            "mode": "kernel",
            "supportedPaths": state.supported_paths.as_ref(),
        },
        "message": "服务运行正常。"
    })))
}
