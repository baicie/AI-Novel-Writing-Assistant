//! 把系统密钥库适配成应用层的 `CredentialVault` 端口。

use std::sync::Arc;

use async_trait::async_trait;
use novel_application::{CredentialVault, PortError};
use novel_platform::SecretStore;

pub struct OsCredentialVault {
    store: Arc<dyn SecretStore>,
}

impl OsCredentialVault {
    pub fn new(store: Arc<dyn SecretStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl CredentialVault for OsCredentialVault {
    async fn write(&self, reference: &str, secret: &str) -> Result<(), PortError> {
        self.store
            .put(reference, secret)
            .await
            .map_err(|error| PortError::Storage(format!("系统凭据写入失败：{error}")))
    }

    async fn delete(&self, reference: &str) -> Result<(), PortError> {
        self.store
            .delete(reference)
            .await
            .map_err(|error| PortError::Storage(format!("系统凭据删除失败：{error}")))
    }

    async fn present(&self, reference: &str) -> Result<bool, PortError> {
        let value = self
            .store
            .get(reference)
            .await
            .map_err(|error| PortError::Storage(format!("系统凭据读取失败：{error}")))?;
        Ok(value.is_some_and(|secret| !secret.trim().is_empty()))
    }
}
