//! Ollama 原生 `/api/chat` 适配器。
//!
//! 只做一件事：把同一条 `LlmRequest` 按 Ollama 的协议发出去，并把结果映射回 `LlmResponse`。
//! 协议判定来自调用方传入的 protocol 字段，绝不靠模型名或地址猜测。

use async_trait::async_trait;
use serde_json::json;
use tokio::sync::mpsc::Sender;
use tokio_util::sync::CancellationToken;

use crate::endpoint::authorize_endpoint;
use crate::error::LlmError;
use crate::provider::{LlmProvider, LlmRequest, LlmResponse, ProviderCapabilities, TokenUsage};

pub struct OllamaProvider {
    client: reqwest::Client,
}

impl Default for OllamaProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl OllamaProvider {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                // 与既有 Provider 一致：不跟随重定向，避免把密钥带去别处。
                .redirect(reqwest::redirect::Policy::none())
                .no_proxy()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .unwrap_or_default(),
        }
    }

    fn chat_endpoint(base: &str) -> String {
        format!("{}/api/chat", base.trim_end_matches('/'))
    }
}

#[async_trait]
impl LlmProvider for OllamaProvider {
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
        request: LlmRequest,
        tokens: Sender<String>,
        cancel: CancellationToken,
    ) -> Result<LlmResponse, LlmError> {
        authorize_endpoint(&request.endpoint, request.allow_localhost_http)?;
        if request.api_key.trim().is_empty() && !request.allow_localhost_http {
            return Err(LlmError::Unauthorized);
        }
        if cancel.is_cancelled() {
            return Err(LlmError::Cancelled);
        }
        let body = json!({
            "model": request.model,
            "messages": [{ "role": "user", "content": request.prompt }],
            "stream": false,
            "options": {
                "num_predict": request.max_output_tokens.unwrap_or(4_000),
            }
        });
        let response = self
            .client
            .post(Self::chat_endpoint(&request.endpoint))
            .json(&body)
            .send()
            .await
            .map_err(|error| {
                if error.is_timeout() {
                    LlmError::Timeout
                } else {
                    LlmError::Unavailable {
                        billed: false,
                        reason: format!("Ollama 请求失败：{error}"),
                    }
                }
            })?;
        match response.status().as_u16() {
            200 => {}
            401 | 403 => return Err(LlmError::Unauthorized),
            429 => return Err(LlmError::RateLimit),
            status => {
                return Err(LlmError::Unavailable {
                    billed: false,
                    reason: format!("Ollama 返回状态码 {status}"),
                });
            }
        }
        let payload: serde_json::Value = response.json().await.map_err(|error| {
            LlmError::InvalidResponse(format!("Ollama 返回内容无法解析：{error}"))
        })?;
        let text = payload
            .get("message")
            .and_then(|message| message.get("content"))
            .and_then(|content| content.as_str())
            .ok_or_else(|| LlmError::InvalidResponse("Ollama 响应缺少 message.content".into()))?
            .to_string();
        let prompt_tokens = payload
            .get("prompt_eval_count")
            .and_then(|value| value.as_u64());
        let completion_tokens = payload.get("eval_count").and_then(|value| value.as_u64());
        let _ = tokens.send(text.clone()).await;
        Ok(LlmResponse {
            text,
            usage: TokenUsage {
                prompt_tokens,
                completion_tokens,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_endpoint_normalises_trailing_slash() {
        assert_eq!(
            OllamaProvider::chat_endpoint("http://127.0.0.1:11434"),
            "http://127.0.0.1:11434/api/chat"
        );
        assert_eq!(
            OllamaProvider::chat_endpoint("http://127.0.0.1:11434/"),
            "http://127.0.0.1:11434/api/chat"
        );
    }
}
