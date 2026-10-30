//! P16.2 模型发现：协议适配器负责「怎么问」，这里只负责错误语义、边界与安全策略。
//!
//! 硬性约束：
//! - 请求全部在 Rust 侧完成，前端永远拿不到 API Key；
//! - 不跟随重定向（避免被引导到别的地址把密钥带走）；
//! - 响应体有大小上限，超时与取消都可控；
//! - 失败必须落进显式错误码，调用方据此决定「保留已有配置」还是提示用户。

use std::time::Duration;

use serde::Serialize;
use thiserror::Error;
use tokio_util::sync::CancellationToken;

use crate::endpoint::authorize_endpoint;

/// 单次发现的响应体上限：正常模型列表远小于这个尺寸。
pub const MAX_RESPONSE_BYTES: usize = 512 * 1024;
/// 默认超时。
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);
/// Anthropic 分页最多翻几页，避免服务端异常导致无限循环。
pub const MAX_PAGES: usize = 20;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredModel {
    pub id: String,
    pub display_name: Option<String>,
    pub context_window: Option<u32>,
    pub max_output_tokens: Option<u32>,
    pub input_modalities: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct DiscoveryRequest {
    pub base_url: String,
    pub api_key: String,
    pub allow_localhost_http: bool,
    pub timeout: Duration,
}

impl DiscoveryRequest {
    pub fn new(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        allow_localhost_http: bool,
    ) -> Self {
        Self {
            base_url: base_url.into(),
            api_key: api_key.into(),
            allow_localhost_http,
            timeout: DEFAULT_TIMEOUT,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryOutcome {
    pub models: Vec<DiscoveredModel>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DiscoveryError {
    #[error("鉴权失败，请检查 API Key")]
    AuthFailed,
    #[error("没有访问权限")]
    Forbidden,
    #[error("请求过于频繁，请稍后再试")]
    RateLimited,
    #[error("该接口不支持模型列表")]
    NotSupported,
    #[error("返回内容无法解析：{0}")]
    InvalidResponse(String),
    #[error("网络错误：{0}")]
    Network(String),
    #[error("请求超时")]
    Timeout,
    #[error("请求已取消")]
    Cancelled,
}

impl DiscoveryError {
    /// 与前端约定的稳定错误码。
    pub fn code(&self) -> &'static str {
        match self {
            Self::AuthFailed => "AUTH_FAILED",
            Self::Forbidden => "FORBIDDEN",
            Self::RateLimited => "RATE_LIMITED",
            Self::NotSupported => "NOT_SUPPORTED",
            Self::InvalidResponse(_) => "INVALID_RESPONSE",
            Self::Network(_) => "NETWORK_ERROR",
            Self::Timeout => "TIMEOUT",
            Self::Cancelled => "CANCELLED",
        }
    }
}

#[async_trait::async_trait]
pub trait ProviderModelDiscovery: Send + Sync {
    async fn discover(
        &self,
        request: &DiscoveryRequest,
        cancel: CancellationToken,
    ) -> Result<DiscoveryOutcome, DiscoveryError>;
}

fn client(timeout: Duration) -> Result<reqwest::Client, DiscoveryError> {
    reqwest::Client::builder()
        .timeout(timeout)
        // 不跟随重定向：跟随会把 Authorization 头带到新的地址上。
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .build()
        .map_err(|error| DiscoveryError::Network(error.to_string()))
}

fn join(base: &str, path: &str) -> String {
    let trimmed = base.trim_end_matches('/');
    format!("{trimmed}/{path}")
}

fn validate(request: &DiscoveryRequest) -> Result<(), DiscoveryError> {
    authorize_endpoint(&request.base_url, request.allow_localhost_http)
        .map_err(|error| DiscoveryError::InvalidResponse(error.to_string()))
}

fn map_status(status: u16) -> DiscoveryError {
    match status {
        401 => DiscoveryError::AuthFailed,
        403 => DiscoveryError::Forbidden,
        404 | 405 | 501 => DiscoveryError::NotSupported,
        429 => DiscoveryError::RateLimited,
        300..=399 => DiscoveryError::InvalidResponse("接口返回重定向，已拒绝跟随".into()),
        500..=599 => DiscoveryError::Network(format!("服务端返回 {status}")),
        _ => DiscoveryError::InvalidResponse(format!("未预期的状态码 {status}")),
    }
}

async fn read_body(response: reqwest::Response) -> Result<Vec<u8>, DiscoveryError> {
    if response
        .content_length()
        .is_some_and(|length| length as usize > MAX_RESPONSE_BYTES)
    {
        return Err(DiscoveryError::InvalidResponse("返回内容过大".into()));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|error| DiscoveryError::Network(error.to_string()))?;
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err(DiscoveryError::InvalidResponse("返回内容过大".into()));
    }
    Ok(bytes.to_vec())
}

/// 通用去重：保留第一次出现的顺序与原始 id。
fn dedupe(models: Vec<DiscoveredModel>) -> Vec<DiscoveredModel> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(models.len());
    for model in models {
        if model.id.trim().is_empty() {
            continue;
        }
        if seen.insert(model.id.clone()) {
            out.push(model);
        }
    }
    out
}

fn parse_openai_payload(bytes: &[u8]) -> Result<Vec<DiscoveredModel>, DiscoveryError> {
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|error| DiscoveryError::InvalidResponse(format!("不是合法 JSON：{error}")))?;
    let items = match value.get("data") {
        Some(serde_json::Value::Array(items)) => items.clone(),
        _ => match value.get("models") {
            // 兼容把模型放在对象映射里的写法：{ models: { "id": {...} } }
            Some(serde_json::Value::Object(map)) => map
                .iter()
                .map(|(id, detail)| {
                    let mut entry = detail.clone();
                    if let serde_json::Value::Object(object) = &mut entry {
                        object
                            .entry("id")
                            .or_insert_with(|| serde_json::Value::String(id.clone()));
                    }
                    entry
                })
                .collect(),
            Some(serde_json::Value::Array(items)) => items.clone(),
            _ => Vec::new(),
        },
    };
    let mut models = Vec::with_capacity(items.len());
    for item in items {
        let Some(id) = item.get("id").and_then(|value| value.as_str()) else {
            continue;
        };
        models.push(DiscoveredModel {
            id: id.to_string(),
            display_name: item
                .get("name")
                .or_else(|| item.get("display_name"))
                .and_then(|value| value.as_str())
                .map(str::to_string),
            context_window: item
                .get("context_window")
                .or_else(|| item.get("context_length"))
                .and_then(|value| value.as_u64())
                .map(|value| value as u32),
            max_output_tokens: item
                .get("max_output_tokens")
                .or_else(|| item.get("max_tokens"))
                .and_then(|value| value.as_u64())
                .map(|value| value as u32),
            input_modalities: item
                .get("modalities")
                .or_else(|| item.get("input_modalities"))
                .and_then(|value| value.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|value| value.as_str().map(str::to_string))
                        .collect()
                }),
        });
    }
    Ok(dedupe(models))
}

/// OpenAI 兼容：`GET {baseURL}/models`，Bearer 鉴权。
pub struct OpenAiCompatibleDiscovery;

#[async_trait::async_trait]
impl ProviderModelDiscovery for OpenAiCompatibleDiscovery {
    async fn discover(
        &self,
        request: &DiscoveryRequest,
        cancel: CancellationToken,
    ) -> Result<DiscoveryOutcome, DiscoveryError> {
        validate(request)?;
        let url = join(&request.base_url, "models");
        let client = client(request.timeout)?;
        let mut builder = client.get(&url).header("accept", "application/json");
        if !request.api_key.trim().is_empty() {
            builder = builder.header(
                "authorization",
                format!("Bearer {}", request.api_key.trim()),
            );
        }
        let send = async {
            let response = builder.send().await.map_err(|error| {
                if error.is_timeout() {
                    DiscoveryError::Timeout
                } else {
                    DiscoveryError::Network(error.to_string())
                }
            })?;
            if !response.status().is_success() {
                return Err(map_status(response.status().as_u16()));
            }
            read_body(response).await
        };
        let bytes = tokio::select! {
            _ = cancel.cancelled() => return Err(DiscoveryError::Cancelled),
            result = tokio::time::timeout(request.timeout, send) => match result {
                Err(_) => return Err(DiscoveryError::Timeout),
                Ok(Ok(bytes)) => bytes,
                Ok(Err(error)) => return Err(error),
            },
        };
        Ok(DiscoveryOutcome {
            models: parse_openai_payload(&bytes)?,
            warnings: Vec::new(),
        })
    }
}

/// Anthropic：`GET {baseURL}/v1/models`，x-api-key + anthropic-version，按游标分页。
pub struct AnthropicDiscovery;

#[async_trait::async_trait]
impl ProviderModelDiscovery for AnthropicDiscovery {
    async fn discover(
        &self,
        request: &DiscoveryRequest,
        cancel: CancellationToken,
    ) -> Result<DiscoveryOutcome, DiscoveryError> {
        validate(request)?;
        let client = client(request.timeout)?;
        let mut models: Vec<DiscoveredModel> = Vec::new();
        let mut after_id: Option<String> = None;
        let mut pages = 0usize;
        loop {
            pages += 1;
            if pages > MAX_PAGES {
                return Ok(DiscoveryOutcome {
                    models: dedupe(models),
                    warnings: vec!["分页超过上限，已截断".into()],
                });
            }
            let mut url = join(&request.base_url, "v1/models?limit=100");
            if let Some(cursor) = &after_id {
                url.push_str("&after_id=");
                url.push_str(cursor);
            }
            let mut builder = client
                .get(&url)
                .header("accept", "application/json")
                .header("anthropic-version", "2023-06-01");
            if !request.api_key.trim().is_empty() {
                builder = builder.header("x-api-key", request.api_key.trim());
            }
            let send = async {
                let response = builder.send().await.map_err(|error| {
                    if error.is_timeout() {
                        DiscoveryError::Timeout
                    } else {
                        DiscoveryError::Network(error.to_string())
                    }
                })?;
                if !response.status().is_success() {
                    return Err(map_status(response.status().as_u16()));
                }
                read_body(response).await
            };
            let bytes = tokio::select! {
                _ = cancel.cancelled() => return Err(DiscoveryError::Cancelled),
                result = tokio::time::timeout(request.timeout, send) => match result {
                    Err(_) => return Err(DiscoveryError::Timeout),
                    Ok(Ok(bytes)) => bytes,
                    Ok(Err(error)) => return Err(error),
                },
            };
            let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|error| {
                DiscoveryError::InvalidResponse(format!("不是合法 JSON：{error}"))
            })?;
            let items = value
                .get("data")
                .and_then(|value| value.as_array())
                .cloned()
                .unwrap_or_default();
            for item in items {
                let Some(id) = item.get("id").and_then(|value| value.as_str()) else {
                    continue;
                };
                models.push(DiscoveredModel {
                    id: id.to_string(),
                    display_name: item
                        .get("display_name")
                        .and_then(|value| value.as_str())
                        .map(str::to_string),
                    context_window: item
                        .get("context_window")
                        .and_then(|value| value.as_u64())
                        .map(|value| value as u32),
                    max_output_tokens: item
                        .get("max_output_tokens")
                        .and_then(|value| value.as_u64())
                        .map(|value| value as u32),
                    input_modalities: None,
                });
            }
            let has_more = value
                .get("has_more")
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            if !has_more {
                break;
            }
            after_id = value
                .get("last_id")
                .and_then(|value| value.as_str())
                .map(str::to_string);
            if after_id.is_none() {
                return Ok(DiscoveryOutcome {
                    models: dedupe(models),
                    warnings: vec!["服务端声明还有更多数据但没有给游标，已截断".into()],
                });
            }
        }
        Ok(DiscoveryOutcome {
            models: dedupe(models),
            warnings: Vec::new(),
        })
    }
}

/// Ollama：`GET {base}/api/tags`，按 Ollama 自己的响应结构解析（不是 OpenAI 的 /models）。
///
/// 请求只允许本地地址（Ollama 默认 `http://127.0.0.1:11434`），
/// 回环地址同样需要调用方显式允许，避免把请求发到意外的内网地址。
pub struct OllamaDiscovery;

#[async_trait::async_trait]
impl ProviderModelDiscovery for OllamaDiscovery {
    async fn discover(
        &self,
        request: &DiscoveryRequest,
        cancel: CancellationToken,
    ) -> Result<DiscoveryOutcome, DiscoveryError> {
        validate(request)?;
        let url = join(&request.base_url, "api/tags");
        let client = client(request.timeout)?;
        let mut builder = client.get(&url).header("accept", "application/json");
        if !request.api_key.trim().is_empty() {
            // Ollama 默认不需要密钥；如果部署在网关后面，允许 Bearer。
            builder = builder.header(
                "authorization",
                format!("Bearer {}", request.api_key.trim()),
            );
        }
        let send = async {
            let response = builder.send().await.map_err(|error| {
                if error.is_timeout() {
                    DiscoveryError::Timeout
                } else {
                    DiscoveryError::Network(error.to_string())
                }
            })?;
            if !response.status().is_success() {
                return Err(map_status(response.status().as_u16()));
            }
            read_body(response).await
        };
        let bytes = tokio::select! {
            _ = cancel.cancelled() => return Err(DiscoveryError::Cancelled),
            result = tokio::time::timeout(request.timeout, send) => match result {
                Err(_) => return Err(DiscoveryError::Timeout),
                Ok(Ok(bytes)) => bytes,
                Ok(Err(error)) => return Err(error),
            },
        };
        let value: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|error| DiscoveryError::InvalidResponse(format!("不是合法 JSON：{error}")))?;
        let items = value
            .get("models")
            .and_then(|value| value.as_array())
            .cloned()
            .unwrap_or_default();
        let mut models = Vec::with_capacity(items.len());
        for item in items {
            // Ollama 的条目是 { name, model, modified_at, size, details: {...} }
            let Some(id) = item
                .get("model")
                .or_else(|| item.get("name"))
                .and_then(|value| value.as_str())
            else {
                continue;
            };
            models.push(DiscoveredModel {
                id: id.to_string(),
                display_name: item
                    .get("name")
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
                context_window: None,
                max_output_tokens: None,
                input_modalities: None,
            });
        }
        Ok(DiscoveryOutcome {
            models: dedupe(models),
            warnings: Vec::new(),
        })
    }
}

/// 按协议挑适配器；未知协议不猜，直接报不支持。
pub fn discovery_for(protocol: &str) -> Option<Box<dyn ProviderModelDiscovery>> {
    match protocol {
        "openai-compatible" => Some(Box::new(OpenAiCompatibleDiscovery)),
        "anthropic-messages" => Some(Box::new(AnthropicDiscovery)),
        "ollama" => Some(Box::new(OllamaDiscovery)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_openai_data_array() {
        let payload = br#"{"data":[{"id":"a","name":"A","context_window":1000},{"id":"b"}]}"#;
        let models = parse_openai_payload(payload).unwrap();
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].id, "a");
        assert_eq!(models[0].context_window, Some(1000));
        assert_eq!(models[0].display_name.as_deref(), Some("A"));
    }

    #[test]
    fn parses_models_object_map() {
        let payload = br#"{"models":{"x":{"context_window":10},"y":{}}}"#;
        let models = parse_openai_payload(payload).unwrap();
        let ids: Vec<&str> = models.iter().map(|model| model.id.as_str()).collect();
        assert_eq!(ids, vec!["x", "y"]);
    }

    #[test]
    fn dedupes_and_drops_empty_ids() {
        let payload = br#"{"data":[{"id":"a"},{"id":"a"},{"id":""},{"id":"b"}]}"#;
        let models = parse_openai_payload(payload).unwrap();
        let ids: Vec<&str> = models.iter().map(|model| model.id.as_str()).collect();
        assert_eq!(ids, vec!["a", "b"]);
    }

    #[test]
    fn empty_payload_is_not_an_error() {
        assert!(parse_openai_payload(br#"{"data":[]}"#).unwrap().is_empty());
        assert!(parse_openai_payload(br#"{}"#).unwrap().is_empty());
    }

    #[test]
    fn invalid_json_is_reported() {
        let error = parse_openai_payload(b"not json").unwrap_err();
        assert_eq!(error.code(), "INVALID_RESPONSE");
    }

    #[test]
    fn status_codes_map_to_stable_error_codes() {
        assert_eq!(map_status(401).code(), "AUTH_FAILED");
        assert_eq!(map_status(403).code(), "FORBIDDEN");
        assert_eq!(map_status(404).code(), "NOT_SUPPORTED");
        assert_eq!(map_status(429).code(), "RATE_LIMITED");
        assert_eq!(map_status(302).code(), "INVALID_RESPONSE");
        assert_eq!(map_status(503).code(), "NETWORK_ERROR");
    }

    #[test]
    fn protocol_lookup_never_guesses() {
        assert!(discovery_for("openai-compatible").is_some());
        assert!(discovery_for("anthropic-messages").is_some());
        assert!(
            discovery_for("ollama").is_some(),
            "Ollama 有独立适配器，不能猜成 openai"
        );
        assert!(discovery_for("openai").is_none());
    }

    #[test]
    fn join_handles_trailing_slash() {
        assert_eq!(
            join("https://api.deepseek.com/v1/", "models"),
            "https://api.deepseek.com/v1/models"
        );
        assert_eq!(
            join("https://gw.example.com", "models"),
            "https://gw.example.com/models"
        );
    }
}
