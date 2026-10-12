use async_trait::async_trait;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::io::BufReader;
use tokio_util::sync::CancellationToken;

use crate::endpoint::authorize_endpoint;
use crate::error::{LlmError, classify_http};
use crate::sse::consume_sse;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCapabilities {
    pub streaming: bool,
    pub structured_output: bool,
    pub tool_call: bool,
    pub reasoning: bool,
    pub context_limit: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsage {
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
}

impl TokenUsage {
    pub fn unknown() -> Self {
        Self {
            prompt_tokens: None,
            completion_tokens: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LlmRequest {
    pub request_id: String,
    pub model: String,
    pub prompt: String,
    pub endpoint: String,
    pub api_key: String,
    pub allow_localhost_http: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmResponse {
    pub text: String,
    pub usage: TokenUsage,
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    fn capabilities(&self) -> ProviderCapabilities;
    async fn generate(
        &self,
        request: LlmRequest,
        tokens: tokio::sync::mpsc::UnboundedSender<String>,
        cancel: CancellationToken,
    ) -> Result<LlmResponse, LlmError>;
}

pub struct UnconfiguredProvider;

#[async_trait]
impl LlmProvider for UnconfiguredProvider {
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            streaming: false,
            structured_output: false,
            tool_call: false,
            reasoning: false,
            context_limit: 0,
        }
    }

    async fn generate(
        &self,
        _request: LlmRequest,
        _tokens: tokio::sync::mpsc::UnboundedSender<String>,
        _cancel: CancellationToken,
    ) -> Result<LlmResponse, LlmError> {
        Err(LlmError::Unavailable {
            billed: false,
            reason: "还没有配置可用的模型".into(),
        })
    }
}

#[derive(Clone)]
pub struct OpenAiCompatibleProvider {
    client: reqwest::Client,
}

impl OpenAiCompatibleProvider {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

impl Default for OpenAiCompatibleProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LlmProvider for OpenAiCompatibleProvider {
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            streaming: true,
            structured_output: false,
            tool_call: false,
            reasoning: false,
            context_limit: 128_000,
        }
    }

    async fn generate(
        &self,
        request: LlmRequest,
        tokens: tokio::sync::mpsc::UnboundedSender<String>,
        cancel: CancellationToken,
    ) -> Result<LlmResponse, LlmError> {
        authorize_endpoint(&request.endpoint, request.allow_localhost_http)?;
        let body = serde_json::json!({
            "model": request.model,
            "stream": true,
            "messages": [{"role": "user", "content": request.prompt}]
        });
        let pending = self
            .client
            .post(&request.endpoint)
            .bearer_auth(request.api_key)
            .header("x-request-id", &request.request_id)
            .json(&body)
            .send();
        let response = tokio::select! {
            _ = cancel.cancelled() => return Err(LlmError::Cancelled),
            result = pending => result.map_err(|_| LlmError::Unavailable { billed: false, reason: "模型服务没有连上".into() })?,
        };
        if !response.status().is_success() {
            return Err(classify_http(response.status().as_u16(), false));
        }
        let stream = response
            .bytes_stream()
            .map(|chunk| chunk.map_err(|err| std::io::Error::other(err.to_string())));
        let reader = BufReader::new(tokio_util::io::StreamReader::new(stream));
        let (text, usage) = consume_sse(reader, &tokens, &cancel).await?;
        Ok(LlmResponse { text, usage })
    }
}

pub fn redact_secrets(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let mut index = 0;
    while index < chars.len() {
        if starts_with(chars.as_slice(), index, "sk-")
            || starts_with(chars.as_slice(), index, "Bearer ")
        {
            let marker = if starts_with(chars.as_slice(), index, "Bearer ") {
                "Bearer "
            } else {
                "sk-"
            };
            out.push_str(marker);
            out.push_str("***");
            index += marker.chars().count();
            while index < chars.len() && !chars[index].is_whitespace() {
                index += 1;
            }
            continue;
        }
        out.push(chars[index]);
        index += 1;
    }
    out
}

fn starts_with(chars: &[char], index: usize, marker: &str) -> bool {
    let marker: Vec<char> = marker.chars().collect();
    chars[index..].starts_with(&marker)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_keys_without_turning_unknown_usage_into_zero() {
        assert_eq!(redact_secrets("key sk-abc123 tail"), "key sk-*** tail");
        assert_eq!(TokenUsage::unknown().prompt_tokens, None);
    }
}
