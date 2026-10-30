//! 按协议分发的 Provider：协议只来自调用方显式传入，绝不猜测。

use async_trait::async_trait;
use tokio::sync::mpsc::Sender;
use tokio_util::sync::CancellationToken;

use crate::error::LlmError;
use crate::ollama::OllamaProvider;
use crate::provider::{
    LlmProvider, LlmRequest, LlmResponse, OpenAiCompatibleProvider, ProviderCapabilities,
};

pub struct ProtocolRouterProvider {
    openai: OpenAiCompatibleProvider,
    ollama: OllamaProvider,
}

impl Default for ProtocolRouterProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl ProtocolRouterProvider {
    pub fn new() -> Self {
        Self {
            openai: OpenAiCompatibleProvider::new(),
            ollama: OllamaProvider::new(),
        }
    }
}

#[async_trait]
impl LlmProvider for ProtocolRouterProvider {
    fn capabilities(&self) -> ProviderCapabilities {
        self.openai.capabilities()
    }

    async fn generate(
        &self,
        request: LlmRequest,
        tokens: Sender<String>,
        cancel: CancellationToken,
    ) -> Result<LlmResponse, LlmError> {
        self.openai.generate(request, tokens, cancel).await
    }

    async fn generate_with_protocol(
        &self,
        request: LlmRequest,
        protocol: &str,
        tokens: Sender<String>,
        cancel: CancellationToken,
    ) -> Result<LlmResponse, LlmError> {
        if protocol == "ollama" {
            self.ollama.generate(request, tokens, cancel).await
        } else {
            self.openai.generate(request, tokens, cancel).await
        }
    }
}
