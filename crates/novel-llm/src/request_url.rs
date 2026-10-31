//! 生成请求的最终 URL：把 baseURL 与协议的操作路径分开处理。
//!
//! 关键不变量：
//! - 用户配置的是 baseURL（例如 https://api.deepseek.com/v1），生成器负责补上操作路径；
//! - 历史配置里已经写明完整操作路径（.../chat/completions、.../api/chat）的，原样使用，不重复追加；
//! - 网关自带的路径前缀必须保留（https://gw.example.com/proxy/v1 → .../proxy/v1/chat/completions）；
//! - 发现模型走 /models，生成走协议对应的操作路径，两者语义一致但路径独立。

use crate::error::LlmError;

/// OpenAI Chat Completions 的操作路径（相对 baseURL）。
pub const OPENAI_CHAT_PATH: &str = "chat/completions";
/// Ollama 原生对话的操作路径（相对 baseURL）。
pub const OLLAMA_CHAT_PATH: &str = "api/chat";

fn base_url(endpoint: &str) -> Result<url::Url, LlmError> {
    let trimmed = endpoint.trim();
    if trimmed.is_empty() {
        return Err(LlmError::EndpointRejected("模型地址不能为空".into()));
    }
    let url = url::Url::parse(trimmed)
        .map_err(|_| LlmError::EndpointRejected("模型地址无法识别".into()))?;
    if url.query().is_some() || url.fragment().is_some() {
        return Err(LlmError::EndpointRejected(
            "模型地址不能带查询参数或锚点".into(),
        ));
    }
    match url.scheme() {
        "http" | "https" => {}
        other => {
            return Err(LlmError::EndpointRejected(format!(
                "模型地址只接受 http 或 https（收到 {other}）"
            )));
        }
    }
    if url.host_str().unwrap_or_default().is_empty() {
        return Err(LlmError::EndpointRejected("模型地址缺少主机".into()));
    }
    Ok(url)
}

fn ends_with_path(path: &str, suffix: &str) -> bool {
    path == format!("/{suffix}") || path.ends_with(&format!("/{suffix}"))
}

/// 生成请求的最终 URL：按协议选择操作路径，并保留 baseURL 里已有的路径前缀。
pub fn generation_url(endpoint: &str, protocol: &str) -> Result<String, LlmError> {
    let mut url = base_url(endpoint)?;
    let path = url.path().trim_end_matches('/').to_string();

    let next = match protocol {
        // Ollama 原生走 /api/chat；但若用户把 baseURL 写成 /v1（Ollama 的 OpenAI 兼容模式），
        // 说明他期望的是 /v1/chat/completions，不能强行改成原生路径。
        "ollama" => {
            if ends_with_path(&path, OPENAI_CHAT_PATH) || ends_with_path(&path, OLLAMA_CHAT_PATH) {
                return Ok(url.to_string());
            }
            if path.ends_with("/v1") {
                format!("{path}/{OPENAI_CHAT_PATH}")
            } else {
                format!("{path}/{OLLAMA_CHAT_PATH}")
            }
        }
        _ => {
            if ends_with_path(&path, OPENAI_CHAT_PATH) {
                return Ok(url.to_string());
            }
            format!("{path}/{OPENAI_CHAT_PATH}")
        }
    };

    url.set_path(&next);
    Ok(url.to_string())
}

/// 发现模型列表的最终 URL（与生成路径独立构造）。
pub fn models_url(endpoint: &str) -> Result<String, LlmError> {
    let mut url = base_url(endpoint)?;
    let path = url.path().trim_end_matches('/').to_string();
    url.set_path(&format!("{path}/models"));
    Ok(url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_base_urls_get_chat_completions() {
        for (base, expect) in [
            (
                "https://api.deepseek.com/v1",
                "https://api.deepseek.com/v1/chat/completions",
            ),
            (
                "https://api.openai.com/v1",
                "https://api.openai.com/v1/chat/completions",
            ),
            (
                "https://api.moonshot.cn/v1",
                "https://api.moonshot.cn/v1/chat/completions",
            ),
        ] {
            assert_eq!(generation_url(base, "openai-compatible").unwrap(), expect);
        }
    }

    #[test]
    fn full_operation_path_is_not_duplicated() {
        let endpoint = "https://api.example.com/v1/chat/completions";
        assert_eq!(
            generation_url(endpoint, "openai-compatible").unwrap(),
            endpoint
        );
    }

    #[test]
    fn gateway_path_prefix_is_preserved() {
        assert_eq!(
            generation_url("https://gw.example.com/proxy/v1", "openai-compatible").unwrap(),
            "https://gw.example.com/proxy/v1/chat/completions"
        );
    }

    #[test]
    fn trailing_slashes_are_normalised() {
        assert_eq!(
            generation_url("https://api.deepseek.com/v1///", "openai-compatible").unwrap(),
            "https://api.deepseek.com/v1/chat/completions"
        );
    }

    #[test]
    fn host_without_path_still_works() {
        assert_eq!(
            generation_url("https://api.example.com", "openai-compatible").unwrap(),
            "https://api.example.com/chat/completions"
        );
    }

    #[test]
    fn ipv4_and_ipv6_hosts_are_preserved() {
        assert_eq!(
            generation_url("http://127.0.0.1:8080/v1", "openai-compatible").unwrap(),
            "http://127.0.0.1:8080/v1/chat/completions"
        );
        assert_eq!(
            generation_url("http://[::1]:8080/v1", "openai-compatible").unwrap(),
            "http://[::1]:8080/v1/chat/completions"
        );
    }

    #[test]
    fn ollama_native_and_openai_compatible_modes() {
        assert_eq!(
            generation_url("http://127.0.0.1:11434", "ollama").unwrap(),
            "http://127.0.0.1:11434/api/chat"
        );
        assert_eq!(
            generation_url("http://127.0.0.1:11434/v1", "ollama").unwrap(),
            "http://127.0.0.1:11434/v1/chat/completions"
        );
        assert_eq!(
            generation_url("http://127.0.0.1:11434/api/chat", "ollama").unwrap(),
            "http://127.0.0.1:11434/api/chat"
        );
    }

    #[test]
    fn models_url_is_independent_from_generation() {
        assert_eq!(
            models_url("https://api.deepseek.com/v1").unwrap(),
            "https://api.deepseek.com/v1/models"
        );
    }

    #[test]
    fn invalid_endpoints_are_rejected() {
        assert!(generation_url("", "openai-compatible").is_err());
        assert!(generation_url("not-a-url", "openai-compatible").is_err());
        assert!(generation_url("https://api.example.com/v1?x=1", "openai-compatible").is_err());
        assert!(generation_url("file:///tmp/model", "openai-compatible").is_err());
    }
}
