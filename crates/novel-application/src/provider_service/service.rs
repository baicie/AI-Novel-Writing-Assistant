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
        // 密钥槽按版本命名：每次替换都写新槽，旧槽继续给历史 Run 用。
        // 这样即使数据库 CAS 失败、或者用户事后改了密钥，已提交的 Run 也不会被换成新密钥。
        let previous = self
            .registry
            .list_providers()
            .await?
            .into_iter()
            .find(|profile| profile.id == draft.id);
        let base_revision = self
            .registry
            .provider_revision(&draft.id)
            .await?
            .unwrap_or(0);
        let next_reference = format!("{}#{}", draft.id, base_revision + 1);
        let reference = match (&credential, previous.as_ref()) {
            // 保留：沿用当前槽，不动密钥。
            (CredentialAction::Keep, Some(profile)) => profile.credential.reference.clone(),
            (CredentialAction::Keep, None) => next_reference.clone(),
            // 清除：写入一个空的新槽标记，旧槽保留给历史 Run。
            (CredentialAction::Clear, _) => format!("{next_reference}#cleared"),
            // 替换：写新槽，提交成功后才成为当前配置。
            (CredentialAction::Replace(_), _) => next_reference.clone(),
        };
        let profile = novel_domain::ProviderProfile {
            id: draft.id.clone(),
            display_name: draft.display_name,
            protocol: draft.protocol,
            base_url: draft.base_url,
            credential: novel_domain::CredentialReference {
                kind: novel_domain::CredentialKind::OsSecret,
                reference: reference.clone(),
            },
            is_builtin: draft.is_builtin,
            allow_localhost_http: draft.allow_localhost_http,
            sort_order: draft.sort_order,
        };
        profile
            .validate()
            .map_err(|error| ProviderError::validation(error.to_string()))?;

        // 先写意图，再提交数据库，最后才落密钥正文。
        //
        // 顺序很重要：新槽名由 revision 推导，两个窗口并发保存会算出同一个名字。
        // 若先写密钥再提交，输的一方会覆盖赢一方的密钥，出现「配置指向 gw#N，密钥却是失败者的」。
        // 改成先提交后写入：输的一方在 CAS 阶段就失败，根本不会碰密钥库。
        let intent_id = if matches!(credential, CredentialAction::Keep) {
            None
        } else {
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
            Some(intent_id)
        };

        let revision = match self
            .registry
            .upsert_provider(&profile, expected_revision, now_ms)
            .await
        {
            Ok(revision) => revision,
            Err(error) => {
                // 版本不匹配或写库失败：这次没有动过任何密钥，旧配置与旧密钥完全不受影响。
                if let Some(intent_id) = &intent_id {
                    let _ = self
                        .intents
                        .finish(intent_id, "pending", Some(&error.to_string()), now_ms)
                        .await;
                }
                return Err(error.into());
            }
        };

        if let (CredentialAction::Replace(secret), Some(intent_id)) = (&credential, &intent_id) {
            match self.vault.write(&reference, secret.trim()).await {
                Ok(()) => {
                    self.intents.finish(intent_id, "done", None, now_ms).await?;
                }
                Err(error) => {
                    // 配置已指向新槽但密钥没写成功：先补偿数据库，避免留下「有配置没密钥」的半条记录。
                    // 旧槽与旧配置在这条路径上从未被改动，历史 Run 不受影响。
                    if let Some(previous) = previous.as_ref() {
                        let _ = self
                            .registry
                            .upsert_provider(previous, Some(revision), now_ms)
                            .await;
                    } else {
                        let _ = self.registry.delete_provider(&profile.id, true).await;
                    }
                    self.intents
                        .finish(intent_id, "pending", Some(&error.to_string()), now_ms)
                        .await?;
                    return Err(error.into());
                }
            }
        } else if let Some(intent_id) = &intent_id {
            self.intents.finish(intent_id, "done", None, now_ms).await?;
        }

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
        let snapshot = self.route_snapshot().await?;
        Self::resolve_from_snapshot(&snapshot, task_type)
    }

    /// 一次性读取路由所需的全部配置，后续解析只在这个快照内进行。
    ///
    /// 这样同一批解析（例如创建 Run 时冻结所有环节）不会因为并发保存配置而拼出混合版本。
    pub(crate) async fn route_snapshot(&self) -> Result<RouteSnapshot, ProviderError> {
        // 走端口的一致性读取：SQLite 适配器在单个读事务里取 providers + models + bindings，
        // 避免并发保存配置时拼出「新 Provider + 旧 Binding」这种混合快照。
        let rows = self.registry.route_snapshot_rows().await?;
        let mut models: std::collections::HashMap<String, Vec<novel_domain::ModelProfile>> =
            std::collections::HashMap::new();
        for model in rows.models {
            models
                .entry(model.provider_id.clone())
                .or_default()
                .push(model);
        }
        Ok(RouteSnapshot {
            bindings: rows.bindings,
            providers: rows.providers,
            models,
        })
    }

    pub(crate) fn resolve_from_snapshot(
        snapshot: &RouteSnapshot,
        task_type: &str,
    ) -> Result<Option<ResolvedModelRoute>, ProviderError> {
        let binding = snapshot
            .bindings
            .iter()
            .find(|item| item.task_type == task_type);
        let Some(binding) = binding else {
            return Ok(None);
        };
        let provider = snapshot
            .providers
            .iter()
            .find(|item| item.id == binding.provider_id)
            .ok_or_else(|| {
                ProviderError::not_found(format!("绑定的模型来源已不存在：{}", binding.provider_id))
            })?;
        let models = snapshot
            .models
            .get(&provider.id)
            .map(Vec::as_slice)
            .unwrap_or_default();
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
            model_id: binding.model_id.clone(),
            protocol: provider.protocol.as_str().to_string(),
            endpoint: provider.base_url.clone(),
            allow_localhost_http: provider.allow_localhost_http,
            credential_ref: provider.credential.reference.clone(),
            reasoning_effort: binding.reasoning_effort.clone(),
            source: "binding".to_string(),
        }))
    }
}

/// 某一时刻的完整路由配置快照。
pub(crate) struct RouteSnapshot {
    bindings: Vec<novel_domain::ModelBinding>,
    providers: Vec<novel_domain::ProviderProfile>,
    models: std::collections::HashMap<String, Vec<novel_domain::ModelProfile>>,
}

impl ProviderService {
    /// 把当前所有绑定解析成「环节 -> 冻结路由」，用于 Run 创建时一次性冻结。
    pub async fn frozen_step_routes(
        &self,
    ) -> Result<std::collections::BTreeMap<String, FrozenStepRoute>, ProviderError> {
        let snapshot = self.route_snapshot().await?;
        let mut routes = std::collections::BTreeMap::new();
        for binding in snapshot.bindings.iter() {
            if let Some(route) = Self::resolve_from_snapshot(&snapshot, &binding.task_type)? {
                routes.insert(
                    binding.task_type.clone(),
                    FrozenStepRoute::from_route(&route),
                );
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
