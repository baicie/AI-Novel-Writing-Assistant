use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AppErrorCode {
    Validation,
    Conflict,
    NotFound,
    Cancelled,
    ModelRateLimit,
    StorageCorrupt,
    IntegrityError,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: AppErrorCode,
    pub message: String,
    pub retryable: bool,
    pub details: Option<Value>,
}

impl AppError {
    pub fn new(code: AppErrorCode, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            code,
            message: message.into(),
            retryable,
            details: None,
        }
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::new(AppErrorCode::Validation, message, false)
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(AppErrorCode::Conflict, message, true)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(AppErrorCode::NotFound, message, false)
    }

    pub fn integrity(message: impl Into<String>) -> Self {
        Self::new(AppErrorCode::IntegrityError, message, false)
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.message)
    }
}

impl std::error::Error for AppError {}

impl From<PortError> for AppError {
    fn from(error: PortError) -> Self {
        match error {
            PortError::NotFound(message) => Self::not_found(message),
            PortError::Conflict(message) => Self::conflict(message),
            PortError::Integrity(message) => {
                tracing::error!(error = %message, "fact integrity");
                Self::integrity("这本书的运行记录不完整，需要人工查看后再继续。")
            }
            PortError::Storage(message) => {
                tracing::error!(error = %message, "storage");
                Self::new(
                    AppErrorCode::StorageCorrupt,
                    "本地资料暂时没有写完，请稍后重试。",
                    true,
                )
            }
        }
    }
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum PortError {
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Integrity(String),
    #[error("{0}")]
    Storage(String),
}
