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
    /// 请求可能已经到达模型服务，但没有拿到可确认的结果。
    /// 与普通失败区分开：界面不能把它描述成「一定没扣费」，也不能自动重发。
    ModelResultUnknown,
    /// 模型已经返回，但输出无法使用（非法 JSON、空候选）。
    /// 这次调用已经发出并可能计费；界面不能按「发送前校验失败」处理。
    ModelResultUnusable,
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

    /// 结果未知：可能已经计费，调用方不能自动重试，只能请作者显式确认。
    pub fn model_result_unknown(message: impl Into<String>) -> Self {
        Self::new(AppErrorCode::ModelResultUnknown, message, false)
    }

    /// 已发送且模型返回，但结果不可用：同样可能已经计费。
    pub fn model_result_unusable(message: impl Into<String>) -> Self {
        Self::new(AppErrorCode::ModelResultUnusable, message, false)
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
                let mut error = Self::new(
                    AppErrorCode::StorageCorrupt,
                    "本地资料暂时没有写完，请稍后重试。",
                    true,
                );
                // 保留底层原因：否则排查时只剩一句通用文案，无法区分损坏、锁等待与 IO 失败。
                error.details = Some(serde_json::json!({ "cause": message }));
                error
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
