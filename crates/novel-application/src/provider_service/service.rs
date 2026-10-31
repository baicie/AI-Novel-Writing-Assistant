//! ProviderService：来源/模型/绑定编排与路由解析。

use std::sync::Arc;

use super::error::*;
use super::model_route::*;

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
        self.upsert_provider_with_confirmation(draft, credential, expected_revision, now_ms, false)
            .await
    }

    /// 明文 HTTP 的授权绑在主机上：改到别的主机时必须显式再确认一次。
    pub async fn upsert_provider_with_confirmation(
        &self,
        draft: ProviderDraft,
        credential: CredentialAction,
        expected_revision: Option<i64>,
        now_ms: i64,
        confirm_plaintext_host_change: bool,
    ) -> Result<ProviderView, ProviderError> {
        if draft.allow_localhost_http
            && !confirm_plaintext_host_change
            && let Ok(existing) = self.get_provider(&draft.id).await
            && plaintext_host_of(&existing.base_url) != plaintext_host_of(&draft.base_url)
        {
            return Err(ProviderError::validation(format!(
                "新地址的主机是 {}，与已允许明文连接的 {} 不同；确认后重新保存",
                plaintext_host_of(&draft.base_url),
                plaintext_host_of(&existing.base_url)
            )));
        }
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
        let providers = self.registry.list_providers().await?;
        if let Some(provider) = providers
            .iter()
            .find(|item| item.id == binding.provider_id)
            .filter(|provider| !protocol_supports_generation(provider.protocol))
        {
            return Err(ProviderError {
                code: ProviderErrorCode::NotSupported,
                message: format!(
                    "{} 协议目前只能读取模型列表，还不能用于创作生成",
                    provider.protocol.as_str()
                ),
                retryable: false,
                details: None,
            });
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

impl ProviderService {
    /// 把当前所有绑定解析成「环节 -> 冻结路由」，用于 Run 创建时一次性冻结。
    pub async fn frozen_step_routes(
        &self,
    ) -> Result<std::collections::BTreeMap<String, FrozenStepRoute>, ProviderError> {
        let mut routes = std::collections::BTreeMap::new();
        for binding in self.list_bindings().await? {
            if let Some(route) = self.resolve_route(&binding.task_type).await? {
                routes.insert(binding.task_type, FrozenStepRoute::from_route(&route));
            }
        }
        Ok(routes)
    }

    /// 注册表里是否已经有「可创建第一本书」的来源：有密钥且有至少一个模型。
    pub async fn ready_provider(&self) -> Result<Option<(String, String, bool)>, ProviderError> {
        for profile in self.registry.list_providers().await? {
            let present = self.vault.present(&profile.credential.reference).await?;
            if !present {
                continue;
            }
            let models = self.registry.list_models(&profile.id).await?;
            if let Some(model) = models.first() {
                return Ok(Some((
                    profile.base_url,
                    model.model_id.clone(),
                    profile.allow_localhost_http,
                )));
            }
        }
        Ok(None)
    }

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

/// 从地址里取主机（含端口）：只用于「明文授权是否换了目标」这一件事。
fn plaintext_host_of(endpoint: &str) -> String {
    let trimmed = endpoint.trim();
    let without_scheme = trimmed
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(trimmed);
    without_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
}
