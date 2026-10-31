//! P16.2.1 内置 Provider Catalog：版本化的服务商模板与已知模型元数据。
//!
//! 三条边界（必须遵守）：
//! 1. 这里只是「候选与已知元数据」，**不是**用户网关的真实模型列表；
//! 2. 模型 id 保留服务商原始写法，不做归一化猜测；
//! 3. 目录值只在用户没有数据时作为初值，任何时候都不覆盖用户已有设置。

use serde::Serialize;

/// 目录版本：内容变化时递增，前端可据此判断是否需要提示刷新。
pub const CATALOG_VERSION: &str = "2026-10-30.1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogModel {
    pub id: &'static str,
    pub display_name: &'static str,
    pub context_window: Option<u32>,
    pub max_output_tokens: Option<u32>,
    pub supports_tools: Option<bool>,
    pub supports_reasoning: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogProvider {
    /// 稳定 id，与 provider_credential.id 同域。
    pub id: &'static str,
    pub display_name: &'static str,
    /// 协议判别字段，不能靠模型名猜。
    pub protocol: &'static str,
    pub base_url: &'static str,
    pub requires_api_key: bool,
    /// 是否通常是本地服务（Ollama 之类）。
    pub local: bool,
    pub models: &'static [CatalogModel],
}

pub const BUILTIN_PROVIDERS: &[CatalogProvider] = &[
    CatalogProvider {
        id: "deepseek",
        display_name: "DeepSeek",
        protocol: "openai-compatible",
        base_url: "https://api.deepseek.com/v1",
        requires_api_key: true,
        local: false,
        models: &[
            CatalogModel {
                id: "deepseek-chat",
                display_name: "DeepSeek Chat",
                context_window: Some(64_000),
                max_output_tokens: Some(8_000),
                supports_tools: Some(true),
                supports_reasoning: Some(false),
            },
            CatalogModel {
                id: "deepseek-reasoner",
                display_name: "DeepSeek Reasoner",
                context_window: Some(64_000),
                max_output_tokens: Some(8_000),
                supports_tools: Some(false),
                supports_reasoning: Some(true),
            },
        ],
    },
    CatalogProvider {
        id: "openai",
        display_name: "OpenAI",
        protocol: "openai-compatible",
        base_url: "https://api.openai.com/v1",
        requires_api_key: true,
        local: false,
        models: &[CatalogModel {
            id: "gpt-4o-mini",
            display_name: "GPT-4o mini",
            context_window: Some(128_000),
            max_output_tokens: Some(16_384),
            supports_tools: Some(true),
            supports_reasoning: Some(false),
        }],
    },
    CatalogProvider {
        id: "anthropic",
        display_name: "Anthropic",
        protocol: "anthropic-messages",
        base_url: "https://api.anthropic.com",
        requires_api_key: true,
        local: false,
        models: &[CatalogModel {
            id: "claude-3-5-sonnet-latest",
            display_name: "Claude 3.5 Sonnet",
            context_window: Some(200_000),
            max_output_tokens: Some(8_192),
            supports_tools: Some(true),
            supports_reasoning: Some(false),
        }],
    },
    CatalogProvider {
        id: "moonshot",
        display_name: "月之暗面",
        protocol: "openai-compatible",
        base_url: "https://api.moonshot.cn/v1",
        requires_api_key: true,
        local: false,
        models: &[CatalogModel {
            id: "moonshot-v1-8k",
            display_name: "Moonshot v1 8K",
            context_window: Some(8_192),
            max_output_tokens: Some(4_096),
            supports_tools: Some(false),
            supports_reasoning: Some(false),
        }],
    },
    CatalogProvider {
        id: "ollama",
        display_name: "Ollama（本地）",
        protocol: "ollama",
        base_url: "http://127.0.0.1:11434",
        requires_api_key: false,
        local: true,
        models: &[],
    },
];

pub fn catalog_provider(id: &str) -> Option<&'static CatalogProvider> {
    BUILTIN_PROVIDERS.iter().find(|provider| provider.id == id)
}

pub fn catalog_models(provider_id: &str) -> &'static [CatalogModel] {
    catalog_provider(provider_id)
        .map(|provider| provider.models)
        .unwrap_or(&[])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_ids_are_unique_and_lowercase() {
        let mut seen = std::collections::HashSet::new();
        for provider in BUILTIN_PROVIDERS {
            assert!(
                provider.id.chars().all(|ch| ch.is_ascii_lowercase()
                    || ch.is_ascii_digit()
                    || ch == '-'
                    || ch == '_'),
                "provider id 必须是小写 slug：{}",
                provider.id
            );
            assert!(
                seen.insert(provider.id),
                "provider id 重复：{}",
                provider.id
            );
        }
    }

    #[test]
    fn catalog_protocols_are_supported_by_discovery() {
        for provider in BUILTIN_PROVIDERS {
            assert!(
                crate::discovery::discovery_for(provider.protocol).is_some(),
                "目录里的协议必须有对应适配器：{} ({})",
                provider.id,
                provider.protocol
            );
        }
    }

    #[test]
    fn model_ids_are_kept_verbatim() {
        // 目录里保存的是服务商原始写法，不做任何归一化
        assert!(
            catalog_models("deepseek")
                .iter()
                .any(|model| model.id == "deepseek-chat")
        );
        assert!(
            catalog_models("anthropic")
                .iter()
                .any(|model| model.id == "claude-3-5-sonnet-latest")
        );
    }

    #[test]
    fn unknown_provider_has_no_models() {
        assert!(catalog_models("not-exists").is_empty());
        assert!(catalog_provider("not-exists").is_none());
    }

    #[test]
    fn local_providers_are_flagged() {
        let ollama = catalog_provider("ollama").expect("ollama 在目录里");
        assert!(ollama.local, "Ollama 必须标记为本地服务");
        assert!(!ollama.requires_api_key);
    }
}
