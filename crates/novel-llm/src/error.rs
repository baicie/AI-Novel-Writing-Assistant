use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LlmError {
    #[error("模型请求过于频繁")]
    RateLimit,
    #[error("模型请求超时")]
    Timeout,
    #[error("模型请求已取消")]
    Cancelled,
    #[error("模型密钥不可用")]
    Unauthorized,
    #[error("{reason}")]
    Unavailable { billed: bool, reason: String },
    /// HTTP 层返回了未被专门处理的状态码。请求已经到达服务端，可能已计费。
    #[error("{0}")]
    InvalidResponse(String),
    /// SSE 流已经开始，但某一行无法解析。服务端必然处理过这次请求。
    #[error("{0}")]
    StreamCorrupted(String),
    /// 服务端在流中下发了 error 事件。服务端已经受理并计费。
    #[error("{0}")]
    ProviderError(String),
    /// 请求在发出前就被拒绝（端点策略、参数校验）。
    #[error("{0}")]
    EndpointRejected(String),
    #[error("{0}")]
    StructuredOutput(String),
    #[error("{0}")]
    BudgetExceeded(String),
}

impl LlmError {
    pub fn retryable_without_new_charge(&self) -> bool {
        match self {
            Self::RateLimit => true,
            // A timeout may have reached the provider and may already be billed.
            // Retrying a non-idempotent generation would risk duplicate charges.
            Self::Timeout => false,
            Self::Unavailable { billed, .. } => !billed,
            Self::Cancelled
            | Self::Unauthorized
            | Self::InvalidResponse(_)
            | Self::StreamCorrupted(_)
            | Self::ProviderError(_)
            | Self::EndpointRejected(_)
            | Self::StructuredOutput(_)
            | Self::BudgetExceeded(_) => false,
        }
    }
}

pub fn classify_http(status: u16, stream_started: bool) -> LlmError {
    match status {
        401 | 403 => LlmError::Unauthorized,
        408 => LlmError::Timeout,
        429 => LlmError::RateLimit,
        500..=599 => LlmError::Unavailable {
            billed: stream_started,
            reason: "模型服务暂时不可用".into(),
        },
        _ => LlmError::InvalidResponse(format!("模型返回了无法处理的状态 {status}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn started_stream_is_not_retried_as_a_free_request() {
        let err = classify_http(502, true);
        assert!(!err.retryable_without_new_charge());
        assert!(classify_http(502, false).retryable_without_new_charge());
        assert!(classify_http(429, false).retryable_without_new_charge());
    }
}
