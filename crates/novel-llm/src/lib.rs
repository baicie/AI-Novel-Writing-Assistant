mod discovery;
mod endpoint;
mod error;
mod provider;
mod sse;
mod structured;

pub use discovery::{
    AnthropicDiscovery, DEFAULT_TIMEOUT, DiscoveredModel, DiscoveryError, DiscoveryOutcome,
    DiscoveryRequest, MAX_RESPONSE_BYTES, OpenAiCompatibleDiscovery, ProviderModelDiscovery,
    discovery_for,
};
pub use endpoint::{authorize_endpoint, authorize_resolved_endpoint};
pub use error::LlmError;
pub use provider::{
    LlmProvider, LlmRequest, LlmResponse, OpenAiCompatibleProvider, ProviderCapabilities,
    ProviderPolicy, TokenUsage, UnconfiguredProvider, redact_secrets,
};
pub use sse::{SsePiece, consume_sse, parse_sse_line};
pub use structured::{may_repair, parse_structured_json};
