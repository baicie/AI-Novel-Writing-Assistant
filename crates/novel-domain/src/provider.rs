//! Provider / 模型注册表的领域模型。
//!
//! 设计要点（见 `docs/design/P16-provider-model-registry.md`）：
//! - provider id 是稳定标识；模型身份由 `(provider_id, model_id)` 联合确定，模型 id 原样保留远端写法；
//! - 能力字段一律三态：`None` 表示未知，任何消费方都不得把未知解释成支持或不支持；
//! - 能力来源有优先级：内置 < 远端发现 < 外部补充 < 用户覆盖，用户覆盖永远最高。

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::DomainError;

/// 协议判别字段：决定用哪个适配器发请求，绝不靠模型名猜。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderProtocol {
    OpenAiCompatible,
    AnthropicMessages,
    Ollama,
}

impl ProviderProtocol {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OpenAiCompatible => "openai-compatible",
            Self::AnthropicMessages => "anthropic-messages",
            Self::Ollama => "ollama",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "openai-compatible" => Some(Self::OpenAiCompatible),
            "anthropic-messages" => Some(Self::AnthropicMessages),
            "ollama" => Some(Self::Ollama),
            _ => None,
        }
    }
}

/// 凭据只存引用：系统凭据条目名或环境变量名，永远不存明文。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialReference {
    pub kind: CredentialKind,
    pub reference: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CredentialKind {
    OsSecret,
    EnvVar,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderProfile {
    pub id: String,
    pub display_name: Option<String>,
    pub protocol: ProviderProtocol,
    pub base_url: String,
    pub credential: CredentialReference,
    pub is_builtin: bool,
    pub allow_localhost_http: bool,
    pub sort_order: i32,
}

impl ProviderProfile {
    pub fn validate(&self) -> Result<(), DomainError> {
        ensure_slug(&self.id)?;
        ensure_base_url(&self.base_url)?;
        if matches!(self.credential.kind, CredentialKind::EnvVar)
            && self.credential.reference.is_empty()
        {
            return Err(DomainError::Invariant("环境变量凭据必须给出变量名"));
        }
        Ok(())
    }
}

/// 能力的来源，决定合并优先级。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModelSource {
    Builtin,
    Discovered,
    Supplemental,
    User,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Modality {
    Text,
    Image,
    Audio,
    Video,
}

/// 模型能力：全部三态，`None` 即未知。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelCapabilities {
    pub context_window: Option<u32>,
    pub max_output_tokens: Option<u32>,
    pub supports_streaming: Option<bool>,
    pub supports_tools: Option<bool>,
    pub supports_reasoning: Option<bool>,
    pub input_modalities: Option<Vec<Modality>>,
    pub reasoning_efforts: Option<BTreeSet<String>>,
}

/// 一个能力字段的名字，用于记录「用户覆盖了哪些字段」。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CapabilityField {
    ContextWindow,
    MaxOutputTokens,
    SupportsStreaming,
    SupportsTools,
    SupportsReasoning,
    InputModalities,
    ReasoningEfforts,
}

impl ModelCapabilities {
    /// 逐字段合并：只有 `Some` 才覆盖，未知不会抹掉已知。
    pub fn overlay(&self, higher: &Self) -> Self {
        Self {
            context_window: higher.context_window.or(self.context_window),
            max_output_tokens: higher.max_output_tokens.or(self.max_output_tokens),
            supports_streaming: higher.supports_streaming.or(self.supports_streaming),
            supports_tools: higher.supports_tools.or(self.supports_tools),
            supports_reasoning: higher.supports_reasoning.or(self.supports_reasoning),
            input_modalities: higher
                .input_modalities
                .clone()
                .or_else(|| self.input_modalities.clone()),
            reasoning_efforts: higher
                .reasoning_efforts
                .clone()
                .or_else(|| self.reasoning_efforts.clone()),
        }
    }

    pub fn field(&self, field: CapabilityField) -> Option<CapabilityValue> {
        match field {
            CapabilityField::ContextWindow => self.context_window.map(CapabilityValue::Number),
            CapabilityField::MaxOutputTokens => self.max_output_tokens.map(CapabilityValue::Number),
            CapabilityField::SupportsStreaming => {
                self.supports_streaming.map(CapabilityValue::Flag)
            }
            CapabilityField::SupportsTools => self.supports_tools.map(CapabilityValue::Flag),
            CapabilityField::SupportsReasoning => {
                self.supports_reasoning.map(CapabilityValue::Flag)
            }
            CapabilityField::InputModalities => self
                .input_modalities
                .as_ref()
                .map(|items| CapabilityValue::Modalities(items.clone())),
            CapabilityField::ReasoningEfforts => self
                .reasoning_efforts
                .as_ref()
                .map(|items| CapabilityValue::Efforts(items.clone())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityValue {
    Number(u32),
    Flag(bool),
    Modalities(Vec<Modality>),
    Efforts(BTreeSet<String>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelProfile {
    pub provider_id: String,
    pub model_id: String,
    pub display_name: Option<String>,
    pub capabilities: ModelCapabilities,
    pub source: ModelSource,
    /// 被用户显式覆盖过的字段：刷新发现结果时这些字段不动。
    pub overridden: BTreeSet<CapabilityField>,
}

impl ModelProfile {
    pub fn validate(&self) -> Result<(), DomainError> {
        ensure_slug(&self.provider_id)?;
        if self.model_id.trim().is_empty() {
            return Err(DomainError::Invariant("模型 id 不能为空"));
        }
        Ok(())
    }

    /// 把一份「发现的」资料合并进当前资料：用户覆盖过的字段保持原值。
    pub fn apply_discovered(&self, discovered: &ModelProfile) -> ModelProfile {
        let mut merged = self.clone();
        merged.display_name = if self
            .overridden
            .contains(&CapabilityField::SupportsStreaming)
        {
            self.display_name.clone()
        } else {
            discovered
                .display_name
                .clone()
                .or_else(|| self.display_name.clone())
        };
        let mut caps = self.capabilities.clone();
        for field in [
            CapabilityField::ContextWindow,
            CapabilityField::MaxOutputTokens,
            CapabilityField::SupportsStreaming,
            CapabilityField::SupportsTools,
            CapabilityField::SupportsReasoning,
            CapabilityField::InputModalities,
            CapabilityField::ReasoningEfforts,
        ] {
            if self.overridden.contains(&field) {
                continue;
            }
            caps = caps.overlay(&discovered.capabilities_of(field));
        }
        merged.capabilities = caps;
        merged
    }

    fn capabilities_of(&self, field: CapabilityField) -> ModelCapabilities {
        let mut caps = ModelCapabilities::default();
        match field {
            CapabilityField::ContextWindow => {
                caps.context_window = self.capabilities.context_window
            }
            CapabilityField::MaxOutputTokens => {
                caps.max_output_tokens = self.capabilities.max_output_tokens
            }
            CapabilityField::SupportsStreaming => {
                caps.supports_streaming = self.capabilities.supports_streaming
            }
            CapabilityField::SupportsTools => {
                caps.supports_tools = self.capabilities.supports_tools
            }
            CapabilityField::SupportsReasoning => {
                caps.supports_reasoning = self.capabilities.supports_reasoning
            }
            CapabilityField::InputModalities => {
                caps.input_modalities = self.capabilities.input_modalities.clone()
            }
            CapabilityField::ReasoningEfforts => {
                caps.reasoning_efforts = self.capabilities.reasoning_efforts.clone()
            }
        }
        caps
    }
}

/// 任务级绑定：某个创作环节用哪个模型。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelBinding {
    pub task_type: String,
    pub provider_id: String,
    pub model_id: String,
    pub reasoning_effort: Option<String>,
}

impl ModelBinding {
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.task_type.trim().is_empty() {
            return Err(DomainError::Invariant("绑定必须给出任务类型"));
        }
        ensure_slug(&self.provider_id)?;
        if self.model_id.trim().is_empty() {
            return Err(DomainError::Invariant("绑定必须给出模型 id"));
        }
        Ok(())
    }
}

/// 发现失败的错误码：显式、可映射到界面提示。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiscoveryErrorCode {
    AuthFailed,
    Forbidden,
    RateLimited,
    NotSupported,
    InvalidResponse,
    NetworkError,
    Timeout,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDiscoveryOutcome {
    pub models: Vec<ModelProfile>,
    /// 部分失败时保留已有配置，只把原因带出来。
    pub warnings: Vec<String>,
}

fn ensure_slug(value: &str) -> Result<(), DomainError> {
    let ok = !value.is_empty()
        && value.len() <= 64
        && value
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_');
    if ok {
        Ok(())
    } else {
        Err(DomainError::Invariant(
            "provider id 只能是小写字母、数字、- 和 _",
        ))
    }
}

fn ensure_base_url(value: &str) -> Result<(), DomainError> {
    let trimmed = value.trim();
    if !(trimmed.starts_with("http://") || trimmed.starts_with("https://")) {
        return Err(DomainError::Invariant(
            "接口地址必须以 http:// 或 https:// 开头",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider(id: &str, base_url: &str) -> ProviderProfile {
        ProviderProfile {
            id: id.into(),
            display_name: None,
            protocol: ProviderProtocol::OpenAiCompatible,
            base_url: base_url.into(),
            credential: CredentialReference {
                kind: CredentialKind::OsSecret,
                reference: id.into(),
            },
            is_builtin: false,
            allow_localhost_http: false,
            sort_order: 0,
        }
    }

    fn caps(context: Option<u32>, tools: Option<bool>) -> ModelCapabilities {
        ModelCapabilities {
            context_window: context,
            supports_tools: tools,
            ..Default::default()
        }
    }

    #[test]
    fn protocol_round_trips() {
        for protocol in [
            ProviderProtocol::OpenAiCompatible,
            ProviderProtocol::AnthropicMessages,
            ProviderProtocol::Ollama,
        ] {
            assert_eq!(ProviderProtocol::parse(protocol.as_str()), Some(protocol));
        }
        assert_eq!(ProviderProtocol::parse("openai"), None);
    }

    #[test]
    fn provider_validation_rejects_bad_input() {
        assert!(
            provider("deepseek", "https://api.deepseek.com/v1")
                .validate()
                .is_ok()
        );
        assert!(
            provider("DeepSeek", "https://api.deepseek.com/v1")
                .validate()
                .is_err()
        );
        assert!(provider("deepseek", "api.deepseek.com").validate().is_err());
        let mut env = provider("gw", "https://gw.example.com/v1");
        env.credential = CredentialReference {
            kind: CredentialKind::EnvVar,
            reference: String::new(),
        };
        assert!(env.validate().is_err());
    }

    #[test]
    fn overlay_keeps_known_values_when_higher_is_unknown() {
        let base = caps(Some(128_000), Some(true));
        let higher = caps(None, None);
        let merged = base.overlay(&higher);
        assert_eq!(merged.context_window, Some(128_000));
        assert_eq!(merged.supports_tools, Some(true));
    }

    #[test]
    fn overlay_takes_higher_when_present() {
        let base = caps(Some(8_000), Some(false));
        let higher = caps(Some(200_000), Some(true));
        let merged = base.overlay(&higher);
        assert_eq!(merged.context_window, Some(200_000));
        assert_eq!(merged.supports_tools, Some(true));
    }

    #[test]
    fn unknown_capability_stays_unknown() {
        let empty = ModelCapabilities::default();
        assert_eq!(empty.field(CapabilityField::SupportsTools), None);
        assert_eq!(empty.field(CapabilityField::ContextWindow), None);
    }

    #[test]
    fn discovered_values_do_not_override_user_fields() {
        let mut current = ModelProfile {
            provider_id: "deepseek".into(),
            model_id: "deepseek-chat".into(),
            display_name: Some("我改过的名字".into()),
            capabilities: caps(Some(64_000), None),
            source: ModelSource::User,
            overridden: BTreeSet::from([CapabilityField::ContextWindow]),
        };
        let discovered = ModelProfile {
            provider_id: "deepseek".into(),
            model_id: "deepseek-chat".into(),
            display_name: Some("远端名字".into()),
            capabilities: caps(Some(1_000_000), Some(true)),
            source: ModelSource::Discovered,
            overridden: BTreeSet::new(),
        };
        let merged = current.apply_discovered(&discovered);
        assert_eq!(
            merged.capabilities.context_window,
            Some(64_000),
            "用户覆盖的上下文不能被覆盖"
        );
        assert_eq!(
            merged.capabilities.supports_tools,
            Some(true),
            "未覆盖字段应吸收新值"
        );
        // 清掉覆盖后可以发现值生效
        current.overridden.clear();
        let merged = current.apply_discovered(&discovered);
        assert_eq!(merged.capabilities.context_window, Some(1_000_000));
    }

    #[test]
    fn binding_requires_task_and_model() {
        let ok = ModelBinding {
            task_type: "chapter".into(),
            provider_id: "deepseek".into(),
            model_id: "deepseek-chat".into(),
            reasoning_effort: None,
        };
        assert!(ok.validate().is_ok());
        assert!(
            ModelBinding {
                task_type: " ".into(),
                ..ok.clone()
            }
            .validate()
            .is_err()
        );
        assert!(
            ModelBinding {
                model_id: "".into(),
                ..ok
            }
            .validate()
            .is_err()
        );
    }
}
