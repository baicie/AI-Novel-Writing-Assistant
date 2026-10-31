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
        reasoning_effort: Option<&str>,
        tokens: Sender<String>,
        cancel: CancellationToken,
    ) -> Result<LlmResponse, LlmError> {
        match protocol {
            "ollama" => {
                // Ollama 没有推理强度字段，忽略该选项。
                let _ = reasoning_effort;
                self.ollama.generate(request, tokens, cancel).await
            }
            "openai-compatible" => {
                self.openai
                    .generate_with_protocol(request, protocol, reasoning_effort, tokens, cancel)
                    .await
            }
            // 只读协议的适配器（例如 Anthropic）不能自动当成 OpenAI 兼容协议发送。
            other => Err(LlmError::Unavailable {
                billed: false,
                reason: format!("协议 {other} 目前只能读取模型列表，还不能用于创作生成"),
            }),
        }
    }
}
