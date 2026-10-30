#![allow(dead_code)]
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SecretError {
    #[error("系统凭据暂时不可用")]
    Unavailable,
}

/// 系统钥匙库不可用时的回退目录（应用数据目录下 secrets/）。
static FALLBACK_DIR: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();

/// 启动时调用一次：设置回退目录。目录权限设为 0700，文件 0600。
pub fn configure_secret_fallback(dir: std::path::PathBuf) {
    let _ = std::fs::create_dir_all(&dir);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    }
    let _ = FALLBACK_DIR.set(dir);
}

fn fallback_file(id: &str) -> Option<std::path::PathBuf> {
    let dir = FALLBACK_DIR.get()?;
    // 文件名只保留安全字符，避免路径逃逸。
    let safe: String = id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    Some(dir.join(format!("{safe}.secret")))
}

fn write_fallback(id: &str, secret: &str) -> Result<(), SecretError> {
    let path = fallback_file(id).ok_or(SecretError::Unavailable)?;
    std::fs::write(&path, secret.as_bytes()).map_err(|_| SecretError::Unavailable)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn read_fallback(id: &str) -> Option<String> {
    let path = fallback_file(id)?;
    std::fs::read_to_string(path)
        .ok()
        .filter(|value| !value.is_empty())
}

fn delete_fallback(id: &str) -> Result<(), SecretError> {
    if let Some(path) = fallback_file(id) {
        let _ = std::fs::remove_file(path);
    }
    Ok(())
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
        // 先把回退写入落盘：钥匙库在部分签名环境下会「写入成功但读不回来」。
        if write_fallback(id, secret).is_err() {
            return Err(SecretError::Unavailable);
        }
        let keychain_holds_secret =
            keyring::Entry::new("Biz Novel Studio", id).is_ok_and(|entry| {
                entry.set_password(secret).is_ok()
                    && entry.get_password().ok().as_deref() == Some(secret)
            });
        if keychain_holds_secret {
            // 钥匙库确实可用，回退文件只是保险，保留即可。
            return Ok(());
        }
        // 钥匙库不可靠时仍算成功：回退文件已写入，读取会走回退。
        Ok(())
    }

    async fn get(&self, id: &str) -> Result<Option<String>, SecretError> {
        let from_keychain = keyring::Entry::new("Biz Novel Studio", id)
            .ok()
            .and_then(|entry| entry.get_password().ok());
        if let Some(secret) = from_keychain {
            return Ok(Some(secret));
        }
        Ok(read_fallback(id))
    }

    async fn delete(&self, id: &str) -> Result<(), SecretError> {
        if let Ok(entry) = keyring::Entry::new("Biz Novel Studio", id) {
            let _ = entry.delete_credential();
        }
        delete_fallback(id)
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

#[cfg(test)]
mod fallback_tests {
    use super::*;

    /// 钥匙库不可用时，回退文件必须能一写一读，且删除后读不到。
    #[test]
    fn fallback_round_trip_works_without_keychain() {
        let dir = std::env::temp_dir().join(format!("p16-secrets-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        configure_secret_fallback(dir.clone());

        write_fallback("probe-id", "sk-probe").unwrap();
        assert_eq!(read_fallback("probe-id").as_deref(), Some("sk-probe"));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(dir.join("probe-id.secret"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600, "回退文件必须是 0600");
        }

        delete_fallback("probe-id").unwrap();
        assert_eq!(read_fallback("probe-id"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
