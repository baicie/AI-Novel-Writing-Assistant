//! 把 SQLite 内核适配成 `ProviderRegistry` 端口。

use async_trait::async_trait;
use novel_application::{CredentialIntents, PendingCredentialIntent, PortError, ProviderRegistry};
use novel_domain::{ModelBinding, ModelProfile, ProviderProfile};

use crate::db::SqliteKernel;

#[async_trait]
impl ProviderRegistry for SqliteKernel {
    async fn list_providers(&self) -> Result<Vec<ProviderProfile>, PortError> {
        SqliteKernel::list_providers(self).await
    }

    async fn provider_revision(&self, provider_id: &str) -> Result<Option<i64>, PortError> {
        SqliteKernel::provider_revision(self, provider_id).await
    }

    async fn upsert_provider(
        &self,
        profile: &ProviderProfile,
        expected_revision: Option<i64>,
        now_ms: i64,
    ) -> Result<i64, PortError> {
        self.upsert_provider_cas(profile, expected_revision, now_ms)
            .await
    }

    async fn delete_provider(&self, provider_id: &str, force: bool) -> Result<(), PortError> {
        self.delete_provider_guarded(provider_id, force).await
    }

    async fn list_models(&self, provider_id: &str) -> Result<Vec<ModelProfile>, PortError> {
        SqliteKernel::list_models(self, provider_id).await
    }

    async fn upsert_model(
        &self,
        model: &ModelProfile,
        user_edit: bool,
        expected_revision: Option<i64>,
        now_ms: i64,
    ) -> Result<(), PortError> {
        if let Some(expected) = expected_revision {
            let current = self
                .provider_revision(&model.provider_id)
                .await?
                .ok_or_else(|| {
                    PortError::NotFound(format!("模型来源不存在：{}", model.provider_id))
                })?;
            if current != expected {
                return Err(PortError::Conflict(format!(
                    "配置已被修改（期望版本 {expected}，当前 {current}），请刷新后再试"
                )));
            }
        }
        SqliteKernel::upsert_model(self, model, user_edit, now_ms).await
    }

    async fn delete_model(
        &self,
        provider_id: &str,
        model_id: &str,
        expected_revision: Option<i64>,
    ) -> Result<(), PortError> {
        self.delete_model_cas(provider_id, model_id, expected_revision)
            .await
    }

    /// 覆盖默认实现：在单个读事务里取路由配置，保证快照内部一致。
    async fn route_snapshot_rows(&self) -> Result<novel_application::RouteSnapshotRows, PortError> {
        SqliteKernel::route_snapshot_rows(self).await
    }

    async fn list_bindings(&self) -> Result<Vec<ModelBinding>, PortError> {
        SqliteKernel::list_model_bindings(self).await
    }

    async fn set_binding(
        &self,
        binding: &ModelBinding,
        expected_revision: Option<i64>,
        now_ms: i64,
    ) -> Result<(), PortError> {
        self.set_binding_cas(binding, expected_revision, now_ms)
            .await
    }

    async fn binding_count(&self, provider_id: &str) -> Result<i64, PortError> {
        SqliteKernel::binding_count(self, provider_id).await
    }
}
#[async_trait]
impl CredentialIntents for SqliteKernel {
    async fn begin(
        &self,
        id: &str,
        provider_id: &str,
        secret_ref: &str,
        action: &str,
        now_ms: i64,
    ) -> Result<(), PortError> {
        self.insert_credential_intent(id, provider_id, secret_ref, action, now_ms)
            .await
    }

    async fn finish(
        &self,
        id: &str,
        state: &str,
        last_error: Option<&str>,
        now_ms: i64,
    ) -> Result<(), PortError> {
        self.finish_credential_intent(id, state, last_error, now_ms)
            .await
    }

    async fn pending(&self) -> Result<Vec<PendingCredentialIntent>, PortError> {
        Ok(self
            .pending_credential_intents()
            .await?
            .into_iter()
            .map(|intent| PendingCredentialIntent {
                id: intent.id,
                provider_id: intent.provider_id,
                secret_ref: intent.secret_ref,
                action: intent.action,
            })
            .collect())
    }
}
