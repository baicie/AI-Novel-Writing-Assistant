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
}

impl ProviderService {
    pub fn new(registry: Arc<dyn ProviderRegistry>, vault: Arc<dyn CredentialVault>) -> Self {
        Self { registry, vault }
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

        // 先处理凭据，失败就不动数据库，避免「库里有一条指向不存在密钥」的配置。
        match &credential {
            CredentialAction::Keep => {}
            CredentialAction::Replace(secret) => {
                self.vault
                    .write(&profile.credential.reference, secret.trim())
                    .await?;
            }
            CredentialAction::Clear => {
                self.vault.delete(&profile.credential.reference).await?;
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

    fn service() -> (ProviderService, Arc<FakeRegistry>, Arc<FakeVault>) {
        let registry = Arc::new(FakeRegistry::default());
        let vault = Arc::new(FakeVault::default());
        (
            ProviderService::new(registry.clone(), vault.clone()),
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
}
