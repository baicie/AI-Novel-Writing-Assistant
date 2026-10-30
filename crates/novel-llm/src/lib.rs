mod catalog;
mod discovery;
mod endpoint;
mod error;
mod ollama;
mod provider;
mod router;
mod sse;
mod structured;

pub use catalog::{
    BUILTIN_PROVIDERS, CATALOG_VERSION, CatalogModel, CatalogProvider, catalog_models,
    catalog_provider,
};
pub use discovery::{
    AnthropicDiscovery, DEFAULT_TIMEOUT, DiscoveredModel, DiscoveryError, DiscoveryOutcome,
    DiscoveryRequest, MAX_RESPONSE_BYTES, OllamaDiscovery, OpenAiCompatibleDiscovery,
    ProviderModelDiscovery, discovery_for,
};
pub use endpoint::{authorize_endpoint, authorize_resolved_endpoint};
pub use error::LlmError;
pub use ollama::OllamaProvider;
pub use provider::{
    LlmProvider, LlmRequest, LlmResponse, OpenAiCompatibleProvider, ProviderCapabilities,
    ProviderPolicy, TokenUsage, UnconfiguredProvider, redact_secrets,
};
pub use router::ProtocolRouterProvider;
pub use sse::{SsePiece, consume_sse, parse_sse_line};
pub use structured::{may_repair, parse_structured_json};
