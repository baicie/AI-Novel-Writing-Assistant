//! 错误码、凭据三态与协议能力判定。

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
    NotSupported,
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

    /// 没有可用密钥：这是「配置不完整」，不是可重试的临时失败。
    pub fn credential_missing(message: impl Into<String>) -> Self {
        Self::new(ProviderErrorCode::CredentialMissing, message, false)
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

/// 具备真实生成适配器的协议白名单：新增协议必须先补适配器再放开绑定。
pub fn protocol_supports_generation(protocol: novel_domain::ProviderProtocol) -> bool {
    matches!(
        protocol,
        novel_domain::ProviderProtocol::OpenAiCompatible | novel_domain::ProviderProtocol::Ollama
    )
}

/// 路由配置的一次性读取结果。
#[derive(Debug, Clone, Default)]
pub struct RouteSnapshotRows {
    pub providers: Vec<novel_domain::ProviderProfile>,
    pub models: Vec<novel_domain::ModelProfile>,
    pub bindings: Vec<novel_domain::ModelBinding>,
}

/// 存储端口：SQLite adapter 实现它。
#[async_trait]
pub trait ProviderRegistry: Send + Sync {
    /// 一次性读取路由所需的全部配置。
    ///
    /// 默认实现只是连续调用三次，**不具备事务一致性**；SQLite 适配器会覆盖成单事务读取。
    async fn route_snapshot_rows(&self) -> Result<RouteSnapshotRows, PortError> {
        let providers = self.list_providers().await?;
        let mut models = Vec::new();
        for provider in &providers {
            models.extend(self.list_models(&provider.id).await?);
        }
        let bindings = self.list_bindings().await?;
        Ok(RouteSnapshotRows {
            providers,
            models,
            bindings,
        })
    }

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
