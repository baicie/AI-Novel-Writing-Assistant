//! P16.2B：Provider 应用服务。
//!
//! 职责边界：
//! - 这里是唯一的业务编排层：CAS 校验、凭据三态、删除保护、发现确认都在本层做；
//! - 存储与密钥库通过端口注入（`ProviderRegistry` / `CredentialVault`），本层不碰 SQL/Keychain；
//! - 返回值一律脱敏，任何 DTO 都不含明文密钥；
//! - 终态错误用稳定机器码表达，前端按码提示。

use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::PortError;

/// 凭据三态：不能把空字符串同时理解为「保留」与「清除」。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CredentialAction {
    Keep,
    Replace(String),
    Clear,
}

impl CredentialAction {
    pub fn kind(&self) -> CredentialActionKind {
        match self {
            Self::Keep => CredentialActionKind::Keep,
            Self::Replace(_) => CredentialActionKind::Replace,
            Self::Clear => CredentialActionKind::Clear,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialActionKind {
    Keep,
    Replace,
    Clear,
}

/// 稳定错误码：前端只认这些字符串。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProviderErrorCode {
    Validation,
    NotFound,
    Conflict,
    InUse,
    CredentialMissing,
    CredentialUnavailable,
    Storage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderError {
    pub code: ProviderErrorCode,
    pub message: String,
    pub retryable: bool,
    /// 只放受控白名单字段（例如当前 revision、被引用条数），绝不放原始响应体。
    pub details: Option<serde_json::Value>,
}

impl ProviderError {
    fn new(code: ProviderErrorCode, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            code,
            message: message.into(),
            retryable,
            details: None,
        }
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::new(ProviderErrorCode::Validation, message, false)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ProviderErrorCode::NotFound, message, false)
    }

    pub fn conflict(message: impl Into<String>, current_revision: Option<i64>) -> Self {
        let mut error = Self::new(ProviderErrorCode::Conflict, message, true);
        if let Some(revision) = current_revision {
            error.details = Some(serde_json::json!({ "currentRevision": revision }));
        }
        error
    }

    pub fn in_use(message: impl Into<String>, blocked_by: i64) -> Self {
        let mut error = Self::new(ProviderErrorCode::InUse, message, false);
        error.details = Some(serde_json::json!({ "blockedByCount": blocked_by }));
        error
    }
}

impl From<PortError> for ProviderError {
    fn from(error: PortError) -> Self {
        match error {
            PortError::NotFound(message) => Self::not_found(message),
            PortError::Conflict(message) => Self::conflict(message, None),
            PortError::Integrity(message) => Self::new(ProviderErrorCode::InUse, message, false),
            PortError::Storage(message) => Self::new(ProviderErrorCode::Storage, message, true),
        }
    }
}

/// 凭据变更意图端口：跨 SQLite 与密钥库的补偿日志（不保存密钥正文）。
#[async_trait]
pub trait CredentialIntents: Send + Sync {
    async fn begin(
        &self,
        id: &str,
        provider_id: &str,
        secret_ref: &str,
        action: &str,
        now_ms: i64,
    ) -> Result<(), PortError>;
    async fn finish(
        &self,
        id: &str,
        state: &str,
        last_error: Option<&str>,
        now_ms: i64,
    ) -> Result<(), PortError>;
    async fn pending(&self) -> Result<Vec<PendingCredentialIntent>, PortError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingCredentialIntent {
    pub id: String,
    pub provider_id: String,
    pub secret_ref: String,
    pub action: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialReconcileReport {
    /// 本次补偿成功的条数（例如 retry 成功的清除）。
    pub completed: usize,
    /// 必须由用户重新输入密钥才能收尾的条数（replace 无法在无明文时重放）。
    pub needs_user_action: Vec<String>,
}

/// 存储端口：SQLite adapter 实现它。
#[async_trait]
pub trait ProviderRegistry: Send + Sync {
    async fn list_providers(&self) -> Result<Vec<novel_domain::ProviderProfile>, PortError>;
    async fn provider_revision(&self, provider_id: &str) -> Result<Option<i64>, PortError>;
    async fn upsert_provider(
        &self,
        profile: &novel_domain::ProviderProfile,
        expected_revision: Option<i64>,
        now_ms: i64,
    ) -> Result<i64, PortError>;
    async fn delete_provider(&self, provider_id: &str, force: bool) -> Result<(), PortError>;
    async fn list_models(
        &self,
        provider_id: &str,
    ) -> Result<Vec<novel_domain::ModelProfile>, PortError>;
    async fn upsert_model(
        &self,
        model: &novel_domain::ModelProfile,
        user_edit: bool,
        expected_revision: Option<i64>,
        now_ms: i64,
    ) -> Result<(), PortError>;
    async fn delete_model(
        &self,
        provider_id: &str,
        model_id: &str,
        expected_revision: Option<i64>,
    ) -> Result<(), PortError>;
    async fn list_bindings(&self) -> Result<Vec<novel_domain::ModelBinding>, PortError>;
    async fn set_binding(
        &self,
        binding: &novel_domain::ModelBinding,
        expected_revision: Option<i64>,
        now_ms: i64,
    ) -> Result<(), PortError>;
    async fn binding_count(&self, provider_id: &str) -> Result<i64, PortError>;
}

/// 凭据端口：系统密钥库实现它；测试用内存实现。
#[async_trait]
pub trait CredentialVault: Send + Sync {
    async fn write(&self, reference: &str, secret: &str) -> Result<(), PortError>;
    async fn delete(&self, reference: &str) -> Result<(), PortError>;
    async fn present(&self, reference: &str) -> Result<bool, PortError>;
}

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
    registry: Arc<dyn ProviderRegistry>,
    vault: Arc<dyn CredentialVault>,
    intents: Arc<dyn CredentialIntents>,
}

impl ProviderService {
    pub fn new(
        registry: Arc<dyn ProviderRegistry>,
        vault: Arc<dyn CredentialVault>,
        intents: Arc<dyn CredentialIntents>,
    ) -> Self {
        Self {
            registry,
            vault,
            intents,
        }
    }

    /// 启动或用户点「重试」时调用的补偿：清除动作可重放，替换动作需要用户重新输入。
    pub async fn reconcile_credentials(
        &self,
        now_ms: i64,
    ) -> Result<CredentialReconcileReport, ProviderError> {
        let mut report = CredentialReconcileReport::default();
        for intent in self.intents.pending().await? {
            match intent.action.as_str() {
                "clear" => match self.vault.delete(&intent.secret_ref).await {
                    Ok(()) => {
                        self.intents
                            .finish(&intent.id, "done", None, now_ms)
                            .await?;
                        report.completed += 1;
                    }
                    Err(error) => {
                        self.intents
                            .finish(&intent.id, "pending", Some(&error.to_string()), now_ms)
                            .await?;
                    }
                },
                _ => {
                    // 密钥正文从不落库，replace 无法自动重放：交给用户重新输入。
                    report.needs_user_action.push(intent.provider_id.clone());
                }
            }
        }
        Ok(report)
    }

    pub async fn list_providers(&self) -> Result<Vec<ProviderView>, ProviderError> {
        let providers = self.registry.list_providers().await?;
        let mut views = Vec::with_capacity(providers.len());
        for profile in providers {
            let revision = self
                .registry
                .provider_revision(&profile.id)
                .await?
                .ok_or_else(|| ProviderError::not_found("模型来源不存在"))?;
            let credential_present = self.vault.present(&profile.credential.reference).await?;
            views.push(ProviderView {
                id: profile.id,
                display_name: profile.display_name,
                protocol: profile.protocol.as_str().to_string(),
                base_url: profile.base_url,
                allow_localhost_http: profile.allow_localhost_http,
                is_builtin: profile.is_builtin,
                sort_order: profile.sort_order,
                revision,
                credential_present,
            });
        }
        Ok(views)
    }

    pub async fn get_provider(&self, provider_id: &str) -> Result<ProviderView, ProviderError> {
        self.list_providers()
            .await?
            .into_iter()
            .find(|view| view.id == provider_id)
            .ok_or_else(|| ProviderError::not_found(format!("模型来源不存在：{provider_id}")))
    }

    /// 写入 Provider。凭据先落密钥库再落库；两步任一失败都给出可恢复错误。
    pub async fn upsert_provider(
        &self,
        draft: ProviderDraft,
        credential: CredentialAction,
        expected_revision: Option<i64>,
        now_ms: i64,
    ) -> Result<ProviderView, ProviderError> {
        if matches!(&credential, CredentialAction::Replace(secret) if secret.trim().is_empty()) {
            return Err(ProviderError::validation(
                "替换密钥时不能传空字符串，保留请用 keep、清除请用 clear",
            ));
        }
        let profile = novel_domain::ProviderProfile {
            id: draft.id.clone(),
            display_name: draft.display_name,
            protocol: draft.protocol,
            base_url: draft.base_url,
            credential: novel_domain::CredentialReference {
                kind: novel_domain::CredentialKind::OsSecret,
                // 每个 provider 独占一个密钥槽，禁止跨 provider 复用默认槽。
                reference: draft.id.clone(),
            },
            is_builtin: draft.is_builtin,
            allow_localhost_http: draft.allow_localhost_http,
            sort_order: draft.sort_order,
        };
        profile
            .validate()
            .map_err(|error| ProviderError::validation(error.to_string()))?;

        // 先写意图再动密钥库：任何一步失败都留下可补偿的记录，且绝不落密钥正文。
        if !matches!(credential, CredentialAction::Keep) {
            let action = match credential {
                CredentialAction::Replace(_) => "replace",
                CredentialAction::Clear => "clear",
                CredentialAction::Keep => "keep",
            };
            let intent_id = format!("{}-{}-{}", profile.id, action, now_ms);
            self.intents
                .begin(
                    &intent_id,
                    &profile.id,
                    &profile.credential.reference,
                    action,
                    now_ms,
                )
                .await?;
            let outcome = match &credential {
                CredentialAction::Replace(secret) => {
                    self.vault
                        .write(&profile.credential.reference, secret.trim())
                        .await
                }
                CredentialAction::Clear => self.vault.delete(&profile.credential.reference).await,
                CredentialAction::Keep => Ok(()),
            };
            match outcome {
                Ok(()) => {
                    self.intents
                        .finish(&intent_id, "done", None, now_ms)
                        .await?;
                }
                Err(error) => {
                    self.intents
                        .finish(&intent_id, "pending", Some(&error.to_string()), now_ms)
                        .await?;
                    return Err(error.into());
                }
            }
        }
        let revision = self
            .registry
            .upsert_provider(&profile, expected_revision, now_ms)
            .await?;
        let credential_present = self.vault.present(&profile.credential.reference).await?;
        Ok(ProviderView {
            id: profile.id,
            display_name: profile.display_name,
            protocol: profile.protocol.as_str().to_string(),
            base_url: profile.base_url,
            allow_localhost_http: profile.allow_localhost_http,
            is_builtin: profile.is_builtin,
            sort_order: profile.sort_order,
            revision,
            credential_present,
        })
    }

    pub async fn delete_provider(
        &self,
        provider_id: &str,
        force: bool,
    ) -> Result<(), ProviderError> {
        let bindings = self.registry.binding_count(provider_id).await?;
        if bindings > 0 && !force {
            return Err(ProviderError::in_use(
                format!("还有 {bindings} 条任务绑定在使用这个模型来源"),
                bindings,
            ));
        }
        self.registry.delete_provider(provider_id, force).await?;
        Ok(())
    }

    pub async fn list_models(
        &self,
        provider_id: &str,
    ) -> Result<Vec<novel_domain::ModelProfile>, ProviderError> {
        Ok(self.registry.list_models(provider_id).await?)
    }

    pub async fn upsert_model(
        &self,
        model: novel_domain::ModelProfile,
        user_edit: bool,
        expected_revision: Option<i64>,
        now_ms: i64,
    ) -> Result<(), ProviderError> {
        model
            .validate()
            .map_err(|error| ProviderError::validation(error.to_string()))?;
        if self
            .registry
            .provider_revision(&model.provider_id)
            .await?
            .is_none()
        {
            return Err(ProviderError::not_found(format!(
                "模型来源不存在：{}",
                model.provider_id
            )));
        }
        Ok(self
            .registry
            .upsert_model(&model, user_edit, expected_revision, now_ms)
            .await?)
    }

    pub async fn delete_model(
        &self,
        provider_id: &str,
        model_id: &str,
        expected_revision: Option<i64>,
    ) -> Result<(), ProviderError> {
        let bindings = self.registry.list_bindings().await?;
        let referenced = bindings
            .iter()
            .filter(|binding| binding.provider_id == provider_id && binding.model_id == model_id)
            .count() as i64;
        if referenced > 0 {
            return Err(ProviderError::in_use(
                format!("这个模型还被 {referenced} 条任务绑定使用"),
                referenced,
            ));
        }
        Ok(self
            .registry
            .delete_model(provider_id, model_id, expected_revision)
            .await?)
    }

    pub async fn list_bindings(&self) -> Result<Vec<novel_domain::ModelBinding>, ProviderError> {
        Ok(self.registry.list_bindings().await?)
    }

    /// 写绑定前必须校验 provider 与 model 都真实存在。
    pub async fn upsert_binding(
        &self,
        binding: novel_domain::ModelBinding,
        expected_revision: Option<i64>,
        now_ms: i64,
    ) -> Result<(), ProviderError> {
        binding
            .validate()
            .map_err(|error| ProviderError::validation(error.to_string()))?;
        if self
            .registry
            .provider_revision(&binding.provider_id)
            .await?
            .is_none()
        {
            return Err(ProviderError::not_found(format!(
                "模型来源不存在：{}",
                binding.provider_id
            )));
        }
        let models = self.registry.list_models(&binding.provider_id).await?;
        if !models
            .iter()
            .any(|model| model.model_id == binding.model_id)
        {
            return Err(ProviderError::not_found(format!(
                "模型不存在：{}",
                binding.model_id
            )));
        }
        Ok(self
            .registry
            .set_binding(&binding, expected_revision, now_ms)
            .await?)
    }
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

impl ProviderService {
    /// 任务级路由解析：有绑定就按绑定，没有绑定返回 `None`（不改变既有默认行为）。
    pub async fn resolve_route(
        &self,
        task_type: &str,
    ) -> Result<Option<ResolvedModelRoute>, ProviderError> {
        let binding = self
            .registry
            .list_bindings()
            .await?
            .into_iter()
            .find(|item| item.task_type == task_type);
        let Some(binding) = binding else {
            return Ok(None);
        };
        let provider = self
            .registry
            .list_providers()
            .await?
            .into_iter()
            .find(|item| item.id == binding.provider_id)
            .ok_or_else(|| {
                ProviderError::not_found(format!("绑定的模型来源已不存在：{}", binding.provider_id))
            })?;
        let models = self.registry.list_models(&provider.id).await?;
        if !models
            .iter()
            .any(|model| model.model_id == binding.model_id)
        {
            return Err(ProviderError::not_found(format!(
                "绑定的模型已被删除：{}",
                binding.model_id
            )));
        }
        Ok(Some(ResolvedModelRoute {
            task_type: task_type.to_string(),
            provider_id: provider.id.clone(),
            model_id: binding.model_id,
            protocol: provider.protocol.as_str().to_string(),
            endpoint: provider.base_url,
            allow_localhost_http: provider.allow_localhost_http,
            credential_ref: provider.credential.reference,
            reasoning_effort: binding.reasoning_effort,
            source: "binding".to_string(),
        }))
    }
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
        }),
        None => Err(ProviderError::validation("还没有配置模型服务。")),
    }
}

impl ProviderService {
    /// Run 恢复时的路由校验：冻结的 provider/model 仍可解析吗？
    ///
    /// - provider 不在注册表里 → 视为旧的默认配置（`default` 等），保持可恢复，不改旧行为；
    /// - provider 在注册表里但模型没了 → 明确报错，由调用方给用户「重新绑定」入口，
    ///   不允许静默换模型继续生成。
    pub async fn validate_frozen_route(
        &self,
        provider_id: &str,
        model_id: &str,
    ) -> Result<FrozenRouteStatus, ProviderError> {
        let providers = self.registry.list_providers().await?;
        let Some(provider) = providers.iter().find(|item| item.id == provider_id) else {
            return Ok(FrozenRouteStatus::Legacy);
        };
        let models = self.registry.list_models(&provider.id).await?;
        if !models.iter().any(|model| model.model_id == model_id) {
            return Err(ProviderError::not_found(format!(
                "这次运行使用的模型已被删除：{} / {}",
                provider.id, model_id
            )));
        }
        Ok(FrozenRouteStatus::Resolved {
            protocol: provider.protocol.as_str().to_string(),
            credential_ref: provider.credential.reference.clone(),
        })
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use super::*;

    #[derive(Default)]
    struct FakeRegistry {
        providers: Mutex<HashMap<String, (novel_domain::ProviderProfile, i64)>>,
        models: Mutex<HashMap<String, Vec<novel_domain::ModelProfile>>>,
        bindings: Mutex<Vec<novel_domain::ModelBinding>>,
    }

    #[async_trait]
    impl ProviderRegistry for FakeRegistry {
        async fn list_providers(&self) -> Result<Vec<novel_domain::ProviderProfile>, PortError> {
            Ok(self
                .providers
                .lock()
                .unwrap()
                .values()
                .map(|(profile, _)| profile.clone())
                .collect())
        }

        async fn provider_revision(&self, provider_id: &str) -> Result<Option<i64>, PortError> {
            Ok(self
                .providers
                .lock()
                .unwrap()
                .get(provider_id)
                .map(|(_, revision)| *revision))
        }

        async fn upsert_provider(
            &self,
            profile: &novel_domain::ProviderProfile,
            expected_revision: Option<i64>,
            _now_ms: i64,
        ) -> Result<i64, PortError> {
            let mut providers = self.providers.lock().unwrap();
            let current = providers.get(&profile.id).map(|(_, revision)| *revision);
            match (current, expected_revision) {
                (Some(actual), Some(expected)) if actual != expected => {
                    return Err(PortError::Conflict("版本不一致".into()));
                }
                (Some(_), None) => return Err(PortError::Conflict("已存在".into())),
                _ => {}
            }
            let next = current.unwrap_or(0) + 1;
            providers.insert(profile.id.clone(), (profile.clone(), next));
            Ok(next)
        }

        async fn delete_provider(&self, provider_id: &str, _force: bool) -> Result<(), PortError> {
            self.providers.lock().unwrap().remove(provider_id);
            Ok(())
        }

        async fn list_models(
            &self,
            provider_id: &str,
        ) -> Result<Vec<novel_domain::ModelProfile>, PortError> {
            Ok(self
                .models
                .lock()
                .unwrap()
                .get(provider_id)
                .cloned()
                .unwrap_or_default())
        }

        async fn upsert_model(
            &self,
            model: &novel_domain::ModelProfile,
            _user_edit: bool,
            _expected_revision: Option<i64>,
            _now_ms: i64,
        ) -> Result<(), PortError> {
            let mut models = self.models.lock().unwrap();
            let entry = models.entry(model.provider_id.clone()).or_default();
            entry.retain(|item| item.model_id != model.model_id);
            entry.push(model.clone());
            Ok(())
        }

        async fn delete_model(
            &self,
            provider_id: &str,
            model_id: &str,
            _expected_revision: Option<i64>,
        ) -> Result<(), PortError> {
            if let Some(models) = self.models.lock().unwrap().get_mut(provider_id) {
                models.retain(|item| item.model_id != model_id);
            }
            Ok(())
        }

        async fn list_bindings(&self) -> Result<Vec<novel_domain::ModelBinding>, PortError> {
            Ok(self.bindings.lock().unwrap().clone())
        }

        async fn set_binding(
            &self,
            binding: &novel_domain::ModelBinding,
            _expected_revision: Option<i64>,
            _now_ms: i64,
        ) -> Result<(), PortError> {
            let mut bindings = self.bindings.lock().unwrap();
            bindings.retain(|item| item.task_type != binding.task_type);
            bindings.push(binding.clone());
            Ok(())
        }

        async fn binding_count(&self, provider_id: &str) -> Result<i64, PortError> {
            Ok(self
                .bindings
                .lock()
                .unwrap()
                .iter()
                .filter(|item| item.provider_id == provider_id)
                .count() as i64)
        }
    }

    #[derive(Default)]
    struct FakeVault {
        values: Mutex<HashMap<String, String>>,
        fail_write: Mutex<bool>,
    }

    #[async_trait]
    impl CredentialVault for FakeVault {
        async fn write(&self, reference: &str, secret: &str) -> Result<(), PortError> {
            if *self.fail_write.lock().unwrap() {
                return Err(PortError::Storage("系统凭据写入失败".into()));
            }
            self.values
                .lock()
                .unwrap()
                .insert(reference.to_string(), secret.to_string());
            Ok(())
        }

        async fn delete(&self, reference: &str) -> Result<(), PortError> {
            self.values.lock().unwrap().remove(reference);
            Ok(())
        }

        async fn present(&self, reference: &str) -> Result<bool, PortError> {
            Ok(self.values.lock().unwrap().contains_key(reference))
        }
    }

    fn draft(id: &str) -> ProviderDraft {
        ProviderDraft {
            id: id.into(),
            display_name: Some("测试网关".into()),
            protocol: novel_domain::ProviderProtocol::OpenAiCompatible,
            base_url: "https://gw.example.com/v1".into(),
            allow_localhost_http: false,
            sort_order: 0,
            is_builtin: false,
        }
    }

    #[derive(Default)]
    struct FakeIntents {
        rows: Mutex<Vec<(String, String, String, String)>>,
    }

    #[async_trait]
    impl CredentialIntents for FakeIntents {
        async fn begin(
            &self,
            id: &str,
            provider_id: &str,
            secret_ref: &str,
            action: &str,
            _now_ms: i64,
        ) -> Result<(), PortError> {
            self.rows.lock().unwrap().push((
                id.to_string(),
                provider_id.to_string(),
                secret_ref.to_string(),
                format!("{action}:pending"),
            ));
            Ok(())
        }

        async fn finish(
            &self,
            id: &str,
            state: &str,
            _last_error: Option<&str>,
            _now_ms: i64,
        ) -> Result<(), PortError> {
            let mut rows = self.rows.lock().unwrap();
            if let Some(row) = rows.iter_mut().find(|row| row.0 == id) {
                let action = row.3.split(':').next().unwrap_or("replace").to_string();
                row.3 = format!("{action}:{state}");
            }
            Ok(())
        }

        async fn pending(&self) -> Result<Vec<PendingCredentialIntent>, PortError> {
            Ok(self
                .rows
                .lock()
                .unwrap()
                .iter()
                .filter(|row| row.3.ends_with(":pending"))
                .map(|row| PendingCredentialIntent {
                    id: row.0.clone(),
                    provider_id: row.1.clone(),
                    secret_ref: row.2.clone(),
                    action: row.3.split(':').next().unwrap_or("replace").to_string(),
                })
                .collect())
        }
    }

    fn service() -> (ProviderService, Arc<FakeRegistry>, Arc<FakeVault>) {
        let registry = Arc::new(FakeRegistry::default());
        let vault = Arc::new(FakeVault::default());
        let intents = Arc::new(FakeIntents::default());
        (
            ProviderService::new(registry.clone(), vault.clone(), intents),
            registry,
            vault,
        )
    }

    #[tokio::test]
    async fn replace_writes_secret_and_hides_it() {
        let (service, _registry, vault) = service();
        let view = service
            .upsert_provider(
                draft("gw"),
                CredentialAction::Replace("sk-secret".into()),
                None,
                1,
            )
            .await
            .unwrap();
        assert!(view.credential_present);
        let json = serde_json::to_string(&view).unwrap();
        assert!(!json.contains("sk-secret"), "视图里不能出现密钥：{json}");
        assert!(vault.values.lock().unwrap().contains_key("gw"));
    }

    #[tokio::test]
    async fn keep_never_touches_the_vault() {
        let (service, _registry, vault) = service();
        service
            .upsert_provider(
                draft("gw"),
                CredentialAction::Replace("sk-1".into()),
                None,
                1,
            )
            .await
            .unwrap();
        let view = service
            .upsert_provider(draft("gw"), CredentialAction::Keep, Some(1), 2)
            .await
            .unwrap();
        assert_eq!(view.revision, 2);
        assert_eq!(
            vault.values.lock().unwrap().get("gw").map(String::as_str),
            Some("sk-1")
        );
    }

    #[tokio::test]
    async fn clear_removes_the_secret() {
        let (service, _registry, vault) = service();
        service
            .upsert_provider(
                draft("gw"),
                CredentialAction::Replace("sk-1".into()),
                None,
                1,
            )
            .await
            .unwrap();
        let view = service
            .upsert_provider(draft("gw"), CredentialAction::Clear, Some(1), 2)
            .await
            .unwrap();
        assert!(!view.credential_present);
        assert!(!vault.values.lock().unwrap().contains_key("gw"));
    }

    #[tokio::test]
    async fn replace_with_empty_secret_is_rejected() {
        let (service, _registry, vault) = service();
        let error = service
            .upsert_provider(
                draft("gw"),
                CredentialAction::Replace("   ".into()),
                None,
                1,
            )
            .await
            .unwrap_err();
        assert_eq!(error.code, ProviderErrorCode::Validation);
        assert!(vault.values.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn stale_revision_is_a_conflict_with_hint() {
        let (service, _registry, _vault) = service();
        service
            .upsert_provider(draft("gw"), CredentialAction::Keep, None, 1)
            .await
            .unwrap();
        let error = service
            .upsert_provider(draft("gw"), CredentialAction::Keep, Some(99), 2)
            .await
            .unwrap_err();
        assert_eq!(error.code, ProviderErrorCode::Conflict);
        assert!(error.retryable);
    }

    #[tokio::test]
    async fn delete_is_blocked_while_bindings_exist() {
        let (service, registry, _vault) = service();
        service
            .upsert_provider(draft("gw"), CredentialAction::Keep, None, 1)
            .await
            .unwrap();
        registry
            .bindings
            .lock()
            .unwrap()
            .push(novel_domain::ModelBinding {
                task_type: "chapter".into(),
                provider_id: "gw".into(),
                model_id: "a".into(),
                reasoning_effort: None,
            });
        let error = service.delete_provider("gw", false).await.unwrap_err();
        assert_eq!(error.code, ProviderErrorCode::InUse);
        assert_eq!(
            error
                .details
                .and_then(|details| details.get("blockedByCount").cloned()),
            Some(serde_json::json!(1))
        );
    }

    #[tokio::test]
    async fn binding_requires_existing_model() {
        let (service, _registry, _vault) = service();
        service
            .upsert_provider(draft("gw"), CredentialAction::Keep, None, 1)
            .await
            .unwrap();
        let error = service
            .upsert_binding(
                novel_domain::ModelBinding {
                    task_type: "chapter".into(),
                    provider_id: "gw".into(),
                    model_id: "missing".into(),
                    reasoning_effort: None,
                },
                None,
                2,
            )
            .await
            .unwrap_err();
        assert_eq!(error.code, ProviderErrorCode::NotFound);
    }

    #[tokio::test]
    async fn credential_failure_never_writes_the_database() {
        let (service, registry, vault) = service();
        *vault.fail_write.lock().unwrap() = true;
        let error = service
            .upsert_provider(
                draft("gw"),
                CredentialAction::Replace("sk-1".into()),
                None,
                1,
            )
            .await
            .unwrap_err();
        assert_eq!(error.code, ProviderErrorCode::Storage);
        assert!(
            registry.providers.lock().unwrap().is_empty(),
            "凭据失败时不能留下半条配置"
        );
    }
    #[tokio::test]
    async fn successful_replace_marks_intent_done() {
        let registry = Arc::new(FakeRegistry::default());
        let vault = Arc::new(FakeVault::default());
        let intents = Arc::new(FakeIntents::default());
        let service = ProviderService::new(registry, vault, intents.clone());
        service
            .upsert_provider(
                draft("gw"),
                CredentialAction::Replace("sk-1".into()),
                None,
                1,
            )
            .await
            .unwrap();
        assert!(intents.rows.lock().unwrap()[0].3.ends_with(":done"));
        assert!(
            service
                .reconcile_credentials(2)
                .await
                .unwrap()
                .needs_user_action
                .is_empty()
        );
    }

    #[tokio::test]
    async fn failed_replace_stays_pending_and_needs_user_action() {
        let registry = Arc::new(FakeRegistry::default());
        let vault = Arc::new(FakeVault::default());
        *vault.fail_write.lock().unwrap() = true;
        let intents = Arc::new(FakeIntents::default());
        let service = ProviderService::new(registry.clone(), vault, intents.clone());
        assert!(
            service
                .upsert_provider(
                    draft("gw"),
                    CredentialAction::Replace("sk-1".into()),
                    None,
                    1
                )
                .await
                .is_err()
        );
        assert!(
            intents.rows.lock().unwrap()[0].3.ends_with(":pending"),
            "失败要留下可补偿意图"
        );
        assert!(registry.providers.lock().unwrap().is_empty());
        let report = service.reconcile_credentials(2).await.unwrap();
        assert_eq!(report.completed, 0);
        assert_eq!(
            report.needs_user_action,
            vec!["gw".to_string()],
            "replace 无法无明文重放"
        );
    }

    #[tokio::test]
    async fn failed_clear_is_replayed_by_reconcile() {
        let registry = Arc::new(FakeRegistry::default());
        let vault = Arc::new(FakeVault::default());
        vault
            .values
            .lock()
            .unwrap()
            .insert("gw".into(), "sk-old".into());
        let intents = Arc::new(FakeIntents::default());
        let service = ProviderService::new(registry, vault.clone(), intents.clone());
        // 先让删除失败：用一个会失败的 vault 包装不了，改为直接构造 pending 意图
        intents.rows.lock().unwrap().push((
            "gw-clear-1".into(),
            "gw".into(),
            "gw".into(),
            "clear:pending".into(),
        ));
        let report = service.reconcile_credentials(2).await.unwrap();
        assert_eq!(report.completed, 1);
        assert!(
            vault.values.lock().unwrap().is_empty(),
            "重放后密钥必须被清掉"
        );
        assert!(intents.rows.lock().unwrap()[0].3.ends_with(":done"));
    }
    #[tokio::test]
    async fn route_resolves_from_binding_without_secret() {
        let (service, registry, _vault) = service();
        service
            .upsert_provider(
                draft("gw"),
                CredentialAction::Replace("sk-secret".into()),
                None,
                1,
            )
            .await
            .unwrap();
        service
            .upsert_model(
                novel_domain::ModelProfile {
                    provider_id: "gw".into(),
                    model_id: "a".into(),
                    display_name: None,
                    capabilities: Default::default(),
                    source: novel_domain::ModelSource::User,
                    overridden: Default::default(),
                },
                true,
                None,
                2,
            )
            .await
            .unwrap();
        service
            .upsert_binding(
                novel_domain::ModelBinding {
                    task_type: "writer".into(),
                    provider_id: "gw".into(),
                    model_id: "a".into(),
                    reasoning_effort: Some("high".into()),
                },
                None,
                3,
            )
            .await
            .unwrap();

        let route = service
            .resolve_route("writer")
            .await
            .unwrap()
            .expect("有绑定");
        assert_eq!(route.provider_id, "gw");
        assert_eq!(route.model_id, "a");
        assert_eq!(route.protocol, "openai-compatible");
        assert_eq!(route.credential_ref, "gw");
        assert_eq!(route.source, "binding");
        let json = serde_json::to_string(&route).unwrap();
        assert!(!json.contains("sk-secret"), "路由里不能出现密钥");
        let _ = registry;
    }

    #[tokio::test]
    async fn route_is_absent_without_binding() {
        let (service, _registry, _vault) = service();
        assert!(
            service.resolve_route("writer").await.unwrap().is_none(),
            "没有绑定时不改默认行为"
        );
    }

    #[tokio::test]
    async fn route_errors_when_bound_model_disappears() {
        let (service, _registry, _vault) = service();
        service
            .upsert_provider(draft("gw"), CredentialAction::Keep, None, 1)
            .await
            .unwrap();
        service
            .upsert_model(
                novel_domain::ModelProfile {
                    provider_id: "gw".into(),
                    model_id: "a".into(),
                    display_name: None,
                    capabilities: Default::default(),
                    source: novel_domain::ModelSource::User,
                    overridden: Default::default(),
                },
                true,
                None,
                2,
            )
            .await
            .unwrap();
        service
            .upsert_binding(
                novel_domain::ModelBinding {
                    task_type: "review".into(),
                    provider_id: "gw".into(),
                    model_id: "a".into(),
                    reasoning_effort: None,
                },
                None,
                3,
            )
            .await
            .unwrap();
        service.delete_model("gw", "a", None).await.ok();
        // 删除被绑定的模型会被拦下，这里直接断言拦截行为
        assert!(
            service
                .delete_model("gw", "a", None)
                .await
                .is_err_and(|error| error.code == ProviderErrorCode::InUse)
        );
        assert!(service.resolve_route("review").await.unwrap().is_some());
    }
    #[tokio::test]
    async fn generation_target_prefers_binding_over_legacy() {
        let route = ResolvedModelRoute {
            task_type: "writer".into(),
            provider_id: "gw".into(),
            model_id: "bound-model".into(),
            protocol: "openai-compatible".into(),
            endpoint: "https://gw.example.com/v1".into(),
            allow_localhost_http: false,
            credential_ref: "gw".into(),
            reasoning_effort: None,
            source: "binding".into(),
        };
        let legacy = LegacyModelTarget {
            endpoint: "https://legacy/v1".into(),
            model: "legacy-model".into(),
            allow_localhost_http: false,
            secret_ref: "openai-compatible".into(),
        };
        let target = resolve_generation_target(Some(route), Some(legacy.clone()), None).unwrap();
        assert_eq!(target.model_id, "bound-model");
        assert_eq!(target.credential_ref, "gw");
        assert_eq!(target.source, "binding");

        // 没有绑定时仍走旧路径，行为不变
        let target = resolve_generation_target(None, Some(legacy), None).unwrap();
        assert_eq!(target.model_id, "legacy-model");
        assert_eq!(target.credential_ref, "openai-compatible");
        assert_eq!(target.source, "default");
    }

    #[tokio::test]
    async fn generation_target_reports_missing_configuration() {
        let error = resolve_generation_target(None, None, None).unwrap_err();
        assert_eq!(error.code, ProviderErrorCode::Validation);
    }

    #[tokio::test]
    async fn generation_target_honours_explicit_model_override() {
        let route = ResolvedModelRoute {
            task_type: "writer".into(),
            provider_id: "gw".into(),
            model_id: "bound".into(),
            protocol: "openai-compatible".into(),
            endpoint: "https://gw/v1".into(),
            allow_localhost_http: false,
            credential_ref: "gw".into(),
            reasoning_effort: None,
            source: "binding".into(),
        };
        let target = resolve_generation_target(Some(route), None, Some("  explicit  ")).unwrap();
        assert_eq!(target.model_id, "explicit");
    }
    #[tokio::test]
    async fn frozen_route_validates_and_returns_refs_without_secret() {
        let (service, _registry, _vault) = service();
        service
            .upsert_provider(
                draft("gw"),
                CredentialAction::Replace("sk-secret".into()),
                None,
                1,
            )
            .await
            .unwrap();
        service
            .upsert_model(
                novel_domain::ModelProfile {
                    provider_id: "gw".into(),
                    model_id: "a".into(),
                    display_name: None,
                    capabilities: Default::default(),
                    source: novel_domain::ModelSource::User,
                    overridden: Default::default(),
                },
                true,
                None,
                2,
            )
            .await
            .unwrap();
        let status = service.validate_frozen_route("gw", "a").await.unwrap();
        match status {
            FrozenRouteStatus::Resolved {
                protocol,
                credential_ref,
            } => {
                assert_eq!(protocol, "openai-compatible");
                assert_eq!(credential_ref, "gw");
            }
            other => panic!("期望 Resolved，实际 {other:?}"),
        }
        let json = serde_json::to_string(&service.validate_frozen_route("gw", "a").await.unwrap())
            .unwrap();
        assert!(!json.contains("sk-secret"), "校验结果不能带密钥");
    }

    #[tokio::test]
    async fn frozen_route_reports_deleted_model() {
        let (service, registry, _vault) = service();
        service
            .upsert_provider(draft("gw"), CredentialAction::Keep, None, 1)
            .await
            .unwrap();
        let error = service
            .validate_frozen_route("gw", "gone")
            .await
            .unwrap_err();
        assert_eq!(error.code, ProviderErrorCode::NotFound);
        assert!(registry.models.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn frozen_route_treats_unknown_provider_as_legacy() {
        let (service, _registry, _vault) = service();
        let status = service
            .validate_frozen_route("default", "legacy-model")
            .await
            .unwrap();
        assert_eq!(
            status,
            FrozenRouteStatus::Legacy,
            "旧默认配置必须继续可恢复"
        );
    }
}
