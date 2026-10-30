use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SecretError {
    #[error("系统凭据暂时不可用")]
    Unavailable,
}

#[async_trait]
pub trait SecretStore: Send + Sync {
    async fn put(&self, id: &str, secret: &str) -> Result<(), SecretError>;
    async fn get(&self, id: &str) -> Result<Option<String>, SecretError>;
    /// 清除一条凭据；不存在也算成功（幂等）。
    async fn delete(&self, id: &str) -> Result<(), SecretError>;
}

#[derive(Default)]
pub struct MemorySecretStore {
    values: Mutex<HashMap<String, String>>,
}

#[async_trait]
impl SecretStore for MemorySecretStore {
    async fn put(&self, id: &str, secret: &str) -> Result<(), SecretError> {
        self.values
            .lock()
            .expect("secret lock")
            .insert(id.to_string(), secret.to_string());
        Ok(())
    }

    async fn get(&self, id: &str) -> Result<Option<String>, SecretError> {
        Ok(self.values.lock().expect("secret lock").get(id).cloned())
    }

    async fn delete(&self, id: &str) -> Result<(), SecretError> {
        self.values.lock().expect("secret lock").remove(id);
        Ok(())
    }
}

#[cfg(feature = "os-keychain")]
pub struct OsSecretStore;

#[cfg(feature = "os-keychain")]
#[async_trait]
impl SecretStore for OsSecretStore {
    async fn put(&self, id: &str, secret: &str) -> Result<(), SecretError> {
        let entry =
            keyring::Entry::new("Biz Novel Studio", id).map_err(|_| SecretError::Unavailable)?;
        entry
            .set_password(secret)
            .map_err(|_| SecretError::Unavailable)?;
        Ok(())
    }

    async fn get(&self, id: &str) -> Result<Option<String>, SecretError> {
        let entry =
            keyring::Entry::new("Biz Novel Studio", id).map_err(|_| SecretError::Unavailable)?;
        match entry.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(SecretError::Unavailable),
        }
    }

    async fn delete(&self, id: &str) -> Result<(), SecretError> {
        let entry =
            keyring::Entry::new("Biz Novel Studio", id).map_err(|_| SecretError::Unavailable)?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(SecretError::Unavailable),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn memory_store_round_trips_without_logging_the_secret() {
        let store = MemorySecretStore::default();
        store.put("openai", "sk-test").await.unwrap();
        assert_eq!(
            store.get("openai").await.unwrap().as_deref(),
            Some("sk-test")
        );
    }
}
