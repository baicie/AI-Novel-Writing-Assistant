mod endpoint;
mod error;
mod provider;
mod sse;
mod structured;

pub use endpoint::authorize_endpoint;
pub use error::LlmError;
pub use provider::{
    LlmProvider, LlmRequest, LlmResponse, OpenAiCompatibleProvider, ProviderCapabilities,
    TokenUsage, UnconfiguredProvider, redact_secrets,
};
pub use sse::{SsePiece, consume_sse, parse_sse_line};
pub use structured::{may_repair, parse_structured_json};
