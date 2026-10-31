//! 模型来源视图、任务路由解析与生成目标。

use std::sync::Arc;

use serde::Serialize;

use super::error::*;

/// 返回给上层的 Provider 视图：脱敏，不含任何密钥。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderView {
    pub id: String,
    pub display_name: Option<String>,
    pub protocol: String,
    pub base_url: String,
    pub allow_localhost_http: bool,
    pub is_builtin: bool,
    pub sort_order: i32,
    pub revision: i64,
    pub credential_present: bool,
}

/// 写入参数：`expected_revision` 为 None 表示「这是新建」。
#[derive(Debug, Clone)]
pub struct ProviderDraft {
    pub id: String,
    pub display_name: Option<String>,
    pub protocol: novel_domain::ProviderProtocol,
    pub base_url: String,
    pub allow_localhost_http: bool,
    pub sort_order: i32,
    pub is_builtin: bool,
}

pub struct ProviderService {
    pub(super) registry: Arc<dyn ProviderRegistry>,
    pub(super) vault: Arc<dyn CredentialVault>,
    pub(super) intents: Arc<dyn CredentialIntents>,
}

/// 解析出的调用路由：只含引用与地址，**不含任何密钥正文**。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedModelRoute {
    pub task_type: String,
    pub provider_id: String,
    pub model_id: String,
    pub protocol: String,
    pub endpoint: String,
    pub allow_localhost_http: bool,
    /// 凭据只给引用，执行时再去系统密钥库取。
    pub credential_ref: String,
    pub reasoning_effort: Option<String>,
    /// `binding` 表示任务级绑定；没有绑定时返回 `None`，调用方沿用原有默认配置。
    pub source: String,
}

/// 生成时真正使用的目标：协议、地址、模型与凭据引用，都是可执行字段。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationTarget {
    pub provider_id: String,
    pub model_id: String,
    pub protocol: String,
    pub endpoint: String,
    pub allow_localhost_http: bool,
    pub credential_ref: String,
    /// `binding` = 任务级绑定；`default` = 既有默认配置。
    pub source: String,
    /// 绑定里选定的推理强度；旧默认配置没有这个概念。
    pub reasoning_effort: Option<String>,
}

/// 旧默认配置的解析结果（来自 `load_model_setup`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyModelTarget {
    pub endpoint: String,
    pub model: String,
    pub allow_localhost_http: bool,
    pub secret_ref: String,
}

/// 生成目标解析：任务绑定优先，没有绑定沿用默认配置，两者都没有就明确报错。
///
/// 这里不改变任何既有行为：没有传入任务类型时，结果与旧路径完全一致。
pub fn resolve_generation_target(
    route: Option<ResolvedModelRoute>,
    legacy: Option<LegacyModelTarget>,
    requested_model: Option<&str>,
) -> Result<GenerationTarget, ProviderError> {
    if let Some(route) = route {
        let model_id = requested_model
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| route.model_id.clone());
        return Ok(GenerationTarget {
            provider_id: route.provider_id,
            model_id,
            protocol: route.protocol,
            endpoint: route.endpoint,
            allow_localhost_http: route.allow_localhost_http,
            credential_ref: route.credential_ref,
            source: route.source,
            reasoning_effort: route.reasoning_effort,
        });
    }
    match legacy {
        Some(legacy) => Ok(GenerationTarget {
            provider_id: "default".to_string(),
            model_id: requested_model
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .unwrap_or(legacy.model),
            protocol: "openai-compatible".to_string(),
            endpoint: legacy.endpoint,
            allow_localhost_http: legacy.allow_localhost_http,
            credential_ref: legacy.secret_ref,
            source: "default".to_string(),
            reasoning_effort: None,
        }),
        None => Err(ProviderError::validation("还没有配置模型服务。")),
    }
}

/// 冻结路由的校验结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum FrozenRouteStatus {
    /// 仍然可解析：返回当前协议与凭据引用（引用不含密钥）。
    Resolved {
        protocol: String,
        credential_ref: String,
    },
    /// 旧的默认配置记录，不属于注册表：保持可恢复。
    Legacy,
}

/// 冻结进 Run 的路由信息：只含引用，不含密钥。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FrozenRoute {
    pub protocol: Option<String>,
    pub credential_ref: Option<String>,
    pub reasoning_effort: Option<String>,
}

/// 把任务路由套到一次 Run 的模型参数上。
///
/// 有绑定就整体替换 provider/model/endpoint/本地地址许可，并冻结协议与凭据引用；
/// 没有绑定则原样返回，调用方行为与旧版本一致。
pub fn apply_route_to_run(
    provider: String,
    model: String,
    endpoint: String,
    allow_localhost_http: bool,
    route: Option<ResolvedModelRoute>,
) -> (String, String, String, bool, FrozenRoute) {
    match route {
        Some(route) => (
            route.provider_id,
            route.model_id,
            route.endpoint,
            route.allow_localhost_http,
            FrozenRoute {
                protocol: Some(route.protocol),
                credential_ref: Some(route.credential_ref),
                reasoning_effort: route.reasoning_effort,
            },
        ),
        None => (
            provider,
            model,
            endpoint,
            allow_localhost_http,
            FrozenRoute::default(),
        ),
    }
}

/// 冻结在 Run 合同里的单环节路由：只含引用与地址，不含任何密钥正文。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrozenStepRoute {
    pub provider_id: String,
    pub model_id: String,
    pub protocol: String,
    pub endpoint: String,
    pub allow_localhost_http: bool,
    pub credential_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
}

impl FrozenStepRoute {
    pub fn from_route(route: &ResolvedModelRoute) -> Self {
        Self {
            provider_id: route.provider_id.clone(),
            model_id: route.model_id.clone(),
            protocol: route.protocol.clone(),
            endpoint: route.endpoint.clone(),
            allow_localhost_http: route.allow_localhost_http,
            credential_ref: route.credential_ref.clone(),
            reasoning_effort: route.reasoning_effort.clone(),
        }
    }
}

/// 某一步骤真正要用的路由来源：命中冻结的环节表，或回落到合同的单一路由。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectiveRoute<'a> {
    Step(&'a FrozenStepRoute),
    Contract,
}

/// 历史合同只写了 provider 名、没有 credential_ref。只有这些明确的历史默认来源允许回退；
/// 其它情况必须带精确引用：否则改一个 provider 名就可能读到别人的密钥槽。
pub const LEGACY_SECRET_PROVIDERS: [&str; 2] = ["openai-compatible", "default"];

/// 解析合同里的密钥引用；历史回退只对白名单来源生效，其余一律报错。
pub fn resolve_contract_credential_ref<'a>(
    contract_provider: &'a str,
    contract_ref: Option<&'a str>,
) -> Result<&'a str, ProviderError> {
    if let Some(reference) = contract_ref {
        return Ok(reference);
    }
    if LEGACY_SECRET_PROVIDERS.contains(&contract_provider) {
        return Ok(contract_provider);
    }
    Err(ProviderError::validation(
        "这个运行记录没有密钥引用，无法安全确定用哪一把密钥；请重新保存模型来源后再继续",
    ))
}

/// 判断某个地址是不是本机回环地址。
fn is_loopback_host(endpoint: &str) -> bool {
    let Ok(url) = url::Url::parse(endpoint.trim()) else {
        return false;
    };
    match url.host() {
        Some(url::Host::Ipv4(address)) => address.is_loopback(),
        Some(url::Host::Ipv6(address)) => address.is_loopback(),
        Some(url::Host::Domain(domain)) => domain.eq_ignore_ascii_case("localhost"),
        None => false,
    }
}

/// 没有密钥时是否允许继续：只对「本机 Ollama」开放，远程来源一律拒绝，
/// 避免把「缺密钥」伪装成可用的远程 Provider。
pub fn allow_keyless(protocol: &str, endpoint: &str) -> bool {
    protocol == "ollama" && is_loopback_host(endpoint)
}

/// 调用前的密钥策略检查：有密钥直接通过，没密钥时按协议与地址判定。
pub fn ensure_credential(
    protocol: &str,
    endpoint: &str,
    api_key: &str,
) -> Result<(), ProviderError> {
    if !api_key.trim().is_empty() || allow_keyless(protocol, endpoint) {
        return Ok(());
    }
    Err(ProviderError::credential_missing(
        "这个模型来源没有可用的密钥，无法调用；请先在设置里填写密钥",
    ))
}

impl<'a> EffectiveRoute<'a> {
    pub fn credential_ref(
        &self,
        contract_provider: &'a str,
        contract_ref: Option<&'a str>,
    ) -> &'a str {
        match self {
            Self::Step(step) => step.credential_ref.as_str(),
            Self::Contract => contract_ref.unwrap_or(contract_provider),
        }
    }

    /// 与 credential_ref 相同，但历史回退受白名单约束，解析不出来时直接报错。
    pub fn credential_ref_checked(
        &self,
        contract_provider: &'a str,
        contract_ref: Option<&'a str>,
    ) -> Result<&'a str, ProviderError> {
        match self {
            Self::Step(step) => Ok(step.credential_ref.as_str()),
            Self::Contract => resolve_contract_credential_ref(contract_provider, contract_ref),
        }
    }

    pub fn model_id(&self, contract_model: &'a str) -> &'a str {
        match self {
            Self::Step(step) => step.model_id.as_str(),
            Self::Contract => contract_model,
        }
    }

    pub fn endpoint(&self, contract_endpoint: &'a str) -> &'a str {
        match self {
            Self::Step(step) => step.endpoint.as_str(),
            Self::Contract => contract_endpoint,
        }
    }

    pub fn allow_localhost_http(&self, contract_allow: bool) -> bool {
        match self {
            Self::Step(step) => step.allow_localhost_http,
            Self::Contract => contract_allow,
        }
    }

    pub fn protocol<'b>(&'b self, contract_protocol: Option<&'b str>) -> &'b str {
        match self {
            Self::Step(step) => step.protocol.as_str(),
            Self::Contract => contract_protocol.unwrap_or("openai-compatible"),
        }
    }

    pub fn reasoning_effort<'b>(&'b self, contract_effort: Option<&'b str>) -> Option<&'b str> {
        match self {
            Self::Step(step) => step.reasoning_effort.as_deref(),
            Self::Contract => contract_effort,
        }
    }
}
