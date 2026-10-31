use std::time::Duration;

use async_trait::async_trait;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::io::BufReader;
use tokio_util::sync::CancellationToken;

use crate::endpoint::{authorize_endpoint, authorize_resolved_endpoint};
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
    /// Hard per-request output cap. `None` uses the provider's conservative default.
    pub max_output_tokens: Option<u64>,
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
        tokens: tokio::sync::mpsc::Sender<String>,
        cancel: CancellationToken,
    ) -> Result<LlmResponse, LlmError>;

    /// 带协议上下文的生成入口：默认忽略协议，等价于 \`generate\`；
    /// 只有真正要按协议分发的实现才覆盖它。
    async fn generate_with_protocol(
        &self,
        request: LlmRequest,
        protocol: &str,
        reasoning_effort: Option<&str>,
        tokens: tokio::sync::mpsc::Sender<String>,
        cancel: CancellationToken,
    ) -> Result<LlmResponse, LlmError> {
        let _ = (protocol, reasoning_effort);
        self.generate(request, tokens, cancel).await
    }
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
        _tokens: tokio::sync::mpsc::Sender<String>,
        _cancel: CancellationToken,
    ) -> Result<LlmResponse, LlmError> {
        Err(LlmError::Unavailable {
            billed: false,
            reason: "还没有配置可用的模型".into(),
        })
    }
}

/// Bounded retry/timeout policy shared by every remote model request.
#[derive(Debug, Clone)]
pub struct ProviderPolicy {
    pub timeout: Duration,
    pub max_attempts: u8,
    pub retry_backoff: Duration,
    pub default_max_output_tokens: u64,
    pub absolute_max_output_tokens: u64,
}

impl Default for ProviderPolicy {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(120),
            max_attempts: 2,
            retry_backoff: Duration::from_millis(300),
            default_max_output_tokens: 4_000,
            absolute_max_output_tokens: 32_000,
        }
    }
}

#[derive(Clone)]
pub struct OpenAiCompatibleProvider {
    client: reqwest::Client,
    policy: ProviderPolicy,
}

impl OpenAiCompatibleProvider {
    pub fn new() -> Self {
        Self::with_policy(ProviderPolicy::default())
    }

    pub fn with_policy(policy: ProviderPolicy) -> Self {
        let client = reqwest::Client::builder()
            .timeout(policy.timeout)
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .build()
            .unwrap_or_default();
        Self { client, policy }
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
        tokens: tokio::sync::mpsc::Sender<String>,
        cancel: CancellationToken,
    ) -> Result<LlmResponse, LlmError> {
        self.generate_inner(request, None, tokens, cancel).await
    }

    async fn generate_with_protocol(
        &self,
        request: LlmRequest,
        _protocol: &str,
        reasoning_effort: Option<&str>,
        tokens: tokio::sync::mpsc::Sender<String>,
        cancel: CancellationToken,
    ) -> Result<LlmResponse, LlmError> {
        self.generate_inner(request, reasoning_effort, tokens, cancel)
            .await
    }
}

impl OpenAiCompatibleProvider {
    async fn generate_inner(
        &self,
        request: LlmRequest,
        reasoning_effort: Option<&str>,
        tokens: tokio::sync::mpsc::Sender<String>,
        cancel: CancellationToken,
    ) -> Result<LlmResponse, LlmError> {
        if request.api_key.trim().is_empty() {
            return Err(LlmError::Unavailable {
                billed: false,
                reason: "还没有填写模型密钥".into(),
            });
        }
        let requested_tokens = request
            .max_output_tokens
            .unwrap_or(self.policy.default_max_output_tokens);
        if requested_tokens == 0 || requested_tokens > self.policy.absolute_max_output_tokens {
            return Err(LlmError::BudgetExceeded(
                "本次生成的 token 预算不在允许范围内".into(),
            ));
        }
        // baseURL 与操作路径分开：内置 Provider 的 baseURL 是 /v1，真正生成要补 /chat/completions；
        // 已经写了完整路径的历史配置原样使用。
        let request_url =
            crate::request_url::generation_url(&request.endpoint, "openai-compatible")?;
        authorize_endpoint(&request_url, request.allow_localhost_http)?;
        authorize_resolved_endpoint(&request_url, request.allow_localhost_http).await?;
        let mut body = serde_json::json!({
            "model": request.model,
            "stream": true,
            "max_tokens": requested_tokens,
            "messages": [{"role": "user", "content": request.prompt}]
        });
        // 只有用户显式选了推理强度且模型支持时才带上，避免不支持的服务端报错。
        if let Some(effort) = reasoning_effort
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            body["reasoning_effort"] = serde_json::Value::String(effort.to_string());
        }
        let mut last_error = None;
        for attempt in 0..self.policy.max_attempts.max(1) {
            if cancel.is_cancelled() {
                return Err(LlmError::Cancelled);
            }
            let pending = self
                .client
                .post(&request_url)
                .bearer_auth(&request.api_key)
                .header("x-request-id", &request.request_id)
                .json(&body)
                .send();
            let response = tokio::select! {
                _ = cancel.cancelled() => return Err(LlmError::Cancelled),
                result = tokio::time::timeout(self.policy.timeout, pending) => match result {
                    Ok(Ok(response)) => response,
                    Ok(Err(_)) => {
                        // The request may have reached the provider even though no
                        // response arrived; do not retry a non-idempotent generation.
                        last_error = Some(LlmError::Unavailable {
                            billed: true,
                            reason: "模型服务连接中断，无法确认是否已计费".into(),
                        });
                        break;
                    }
                    Err(_) => {
                        // Timeout is deliberately non-retryable for the same billing
                        // uncertainty reason.
                        last_error = Some(LlmError::Timeout);
                        break;
                    }
                },
            };
            if !response.status().is_success() {
                let classified = classify_http(response.status().as_u16(), false);
                let retry = classified.retryable_without_new_charge()
                    && attempt + 1 < self.policy.max_attempts;
                last_error = Some(classified);
                if retry {
                    sleep_or_cancel(self.policy.retry_backoff, &cancel).await?;
                    continue;
                }
                break;
            }
            let stream = response
                .bytes_stream()
                .map(|chunk| chunk.map_err(|err| std::io::Error::other(err.to_string())));
            let reader = BufReader::new(tokio_util::io::StreamReader::new(stream));
            let (text, usage) = consume_sse(reader, &tokens, &cancel).await?;
            return Ok(LlmResponse { text, usage });
        }
        Err(last_error.unwrap_or(LlmError::Unavailable {
            billed: false,
            reason: "模型请求没有发出".into(),
        }))
    }
}

async fn sleep_or_cancel(duration: Duration, cancel: &CancellationToken) -> Result<(), LlmError> {
    tokio::select! {
        _ = cancel.cancelled() => Err(LlmError::Cancelled),
        _ = tokio::time::sleep(duration) => Ok(()),
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

    #[tokio::test]
    async fn openai_compatible_rejects_empty_api_key_before_hitting_the_endpoint() {
        let provider = OpenAiCompatibleProvider::new();
        let (sender, _receiver) = tokio::sync::mpsc::channel(64);
        let result = provider
            .generate(
                LlmRequest {
                    request_id: "req".into(),
                    model: "gpt-test".into(),
                    prompt: "hello".into(),
                    endpoint: "https://example.com/v1/chat/completions".into(),
                    api_key: String::new(),
                    allow_localhost_http: false,
                    max_output_tokens: Some(64),
                },
                sender,
                CancellationToken::new(),
            )
            .await;
        match result {
            Err(LlmError::Unavailable { billed, reason }) => {
                assert!(!billed);
                assert!(reason.contains("模型密钥"), "unexpected reason: {reason}");
            }
            other => panic!("expected Unavailable error, got {other:?}"),
        }
    }
}
