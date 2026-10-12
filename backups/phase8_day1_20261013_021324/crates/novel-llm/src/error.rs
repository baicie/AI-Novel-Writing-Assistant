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
    #[error("{0}")]
    InvalidResponse(String),
    #[error("{0}")]
    EndpointRejected(String),
    #[error("{0}")]
    StructuredOutput(String),
}

impl LlmError {
    pub fn retryable_without_new_charge(&self) -> bool {
        match self {
            Self::RateLimit | Self::Timeout => true,
            Self::Unavailable { billed, .. } => !billed,
            Self::Cancelled
            | Self::Unauthorized
            | Self::InvalidResponse(_)
            | Self::EndpointRejected(_)
            | Self::StructuredOutput(_) => false,
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
