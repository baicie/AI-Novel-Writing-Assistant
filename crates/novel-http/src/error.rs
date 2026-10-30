use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use novel_application::AppError;

/// 统一的 HTTP 错误：`AppError` 转成 4xx/5xx，并把 AppError 的内容放到 `ApiResponse` 的 `error` 字段。
#[derive(Debug, thiserror::Error)]
pub enum HttpError {
    #[error(transparent)]
    Application(#[from] AppError),
    #[error("请求体不是合法 JSON。")]
    BadRequest(String),
    #[error("接口尚未在内核里实现。")]
    NotImplemented {
        message: String,
        supported: Vec<String>,
    },
    #[error("内核处理请求时遇到内部错误：{0}")]
    Internal(String),
}

impl HttpError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::BadRequest(message.into())
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }

    pub fn not_implemented(supported: Vec<String>) -> Self {
        Self::NotImplemented {
            message: "这个接口还在 Node 旧服务里，没有进入 Rust 内核。".into(),
            supported,
        }
    }
}

impl IntoResponse for HttpError {
    fn into_response(self) -> Response {
        let (status, body) = match &self {
            HttpError::Application(error) => {
                let status = match error.code {
                    novel_application::AppErrorCode::Validation => StatusCode::BAD_REQUEST,
                    novel_application::AppErrorCode::Conflict => StatusCode::CONFLICT,
                    novel_application::AppErrorCode::NotFound => StatusCode::NOT_FOUND,
                    novel_application::AppErrorCode::Cancelled => StatusCode::ACCEPTED,
                    novel_application::AppErrorCode::ModelRateLimit => {
                        StatusCode::TOO_MANY_REQUESTS
                    }
                    novel_application::AppErrorCode::StorageCorrupt => {
                        StatusCode::SERVICE_UNAVAILABLE
                    }
                    novel_application::AppErrorCode::IntegrityError => {
                        StatusCode::INTERNAL_SERVER_ERROR
                    }
                };
                (
                    status,
                    json!({
                        "success": false,
                        "error": error.message,
                        "code": error.code,
                        "retryable": error.retryable,
                    }),
                )
            }
            HttpError::BadRequest(message) => (
                StatusCode::BAD_REQUEST,
                json!({
                    "success": false,
                    "error": message,
                }),
            ),
            HttpError::NotImplemented { message, supported } => (
                StatusCode::NOT_IMPLEMENTED,
                json!({
                    "success": false,
                    "error": message,
                    "migration": "pending",
                    "supportedPaths": supported,
                }),
            ),
            HttpError::Internal(message) => {
                tracing::error!(error = %message, "kernel http internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    json!({
                        "success": false,
                        "error": "本地创作服务暂时没有响应，请稍后重试。",
                    }),
                )
            }
        };
        (status, Json(body)).into_response()
    }
}

pub type HttpResult<T> = Result<T, HttpError>;
