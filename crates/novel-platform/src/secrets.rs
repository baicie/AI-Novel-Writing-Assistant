#![allow(dead_code)]
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
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

/// 加密文件头：用于区分「加密后的密文」与「历史明文」。
const MAGIC: &[u8] = b"P16ENC1";

fn random_bytes<const N: usize>() -> [u8; N] {
    let mut out = [0_u8; N];
    let mut filled = 0;
    while filled < N {
        let chunk = uuid::Uuid::new_v4();
        let bytes = chunk.as_bytes();
        let take = (N - filled).min(bytes.len());
        out[filled..filled + take].copy_from_slice(&bytes[..take]);
        filled += take;
    }
    out
}

/// 主密钥：优先放系统钥匙库；不可用时落 0600 文件。
fn parse_key_hex(text: &str) -> Option<[u8; 32]> {
    let bytes = hex::decode(text.trim()).ok()?;
    let array: [u8; 32] = bytes.try_into().ok()?;
    Some(array)
}

static MASTER_KEY_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// 主密钥：以应用数据目录的 0600 文件为唯一权威（钥匙库只做尽力而为的副本）。
///
/// 之所以不让钥匙库和文件各当一次来源：钥匙库在部分签名环境下「写入成功但读不回来」，
/// 两处都当权威会出现同一进程内拿到两把不同密钥，导致自己刚写的密文解不开。
fn master_key(dir: &std::path::Path) -> Result<[u8; 32], SecretError> {
    // 并发下必须串行：否则两个线程可能各建一把密钥，互相解不开。
    let _guard = MASTER_KEY_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());

    let path = dir.join("master.key");
    if let Some(key) = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| parse_key_hex(&text))
    {
        return Ok(key);
    }

    let key = random_bytes::<32>();
    let encoded = hex::encode(key);
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(mut file) => {
            use std::io::Write;
            file.write_all(encoded.as_bytes())
                .map_err(|_| SecretError::Unavailable)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
            }
            mirror_master_key_to_keychain(&encoded);
            Ok(key)
        }
        // 并发创建时后到者直接读赢家的密钥。
        Err(_) => std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| parse_key_hex(&text))
            .ok_or(SecretError::Unavailable),
    }
}

/// 尽力而为：把主密钥副本放一份到系统钥匙库，失败完全忽略（文件才是权威）。
fn mirror_master_key_to_keychain(encoded: &str) {
    #[cfg(feature = "os-keychain")]
    {
        if let Ok(entry) = keyring::Entry::new("Biz Novel Studio", "local-master-key") {
            let _ = entry.set_password(encoded);
        }
    }
    #[cfg(not(feature = "os-keychain"))]
    {
        let _ = encoded;
    }
}

fn write_fallback(id: &str, secret: &str) -> Result<(), SecretError> {
    let path = fallback_file(id).ok_or(SecretError::Unavailable)?;
    let dir = path.parent().ok_or(SecretError::Unavailable)?;
    let key = master_key(dir)?;
    let cipher = ChaCha20Poly1305::new((&key).into());
    let nonce_bytes = random_bytes::<12>();
    let nonce = Nonce::from_slice(&nonce_bytes);
    let sealed = cipher
        .encrypt(
            nonce,
            Payload {
                msg: secret.as_bytes(),
                aad: id.as_bytes(),
            },
        )
        .map_err(|_| SecretError::Unavailable)?;
    let mut payload = Vec::with_capacity(MAGIC.len() + nonce_bytes.len() + sealed.len());
    payload.extend_from_slice(MAGIC);
    payload.extend_from_slice(&nonce_bytes);
    payload.extend_from_slice(&sealed);
    std::fs::write(&path, &payload).map_err(|_| SecretError::Unavailable)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn read_fallback(id: &str) -> Option<String> {
    let path = fallback_file(id)?;
    let raw = std::fs::read(&path).ok()?;
    if raw.len() < MAGIC.len() + 12 || !raw.starts_with(MAGIC) {
        // 历史明文：读得出来就顺手加密写回，完成迁移。
        let legacy = String::from_utf8(raw)
            .ok()
            .filter(|value| !value.is_empty())?;
        let _ = write_fallback(id, &legacy);
        return Some(legacy);
    }
    let dir = path.parent()?;
    let key = master_key(dir).ok()?;
    let cipher = ChaCha20Poly1305::new((&key).into());
    let nonce_bytes: [u8; 12] = raw[MAGIC.len()..MAGIC.len() + 12].try_into().ok()?;
    let nonce = Nonce::from_slice(&nonce_bytes);
    let plain = cipher
        .decrypt(
            nonce,
            Payload {
                msg: &raw[MAGIC.len() + 12..],
                aad: id.as_bytes(),
            },
        )
        .ok()?;
    String::from_utf8(plain)
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

    /// 回退目录是进程级全局配置，测试共用一个目录，避免主密钥被反复重建。
    fn temp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("p16-secrets-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        configure_secret_fallback(dir.clone());
        dir
    }

    #[cfg(unix)]
    fn mode_of(path: &std::path::Path) -> u32 {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    /// 写入后是密文、能读回、删除后读不到，且主密钥与密文文件都是 0600。
    #[test]
    fn fallback_round_trip_is_encrypted_and_private() {
        let dir = temp_dir();
        write_fallback("probe-id", "sk-probe-secret").unwrap();

        let path = dir.join("probe-id.secret");
        let raw = std::fs::read(&path).unwrap();
        assert!(raw.starts_with(MAGIC), "密文必须带文件头");
        assert!(
            !String::from_utf8_lossy(&raw).contains("sk-probe-secret"),
            "磁盘上不能出现明文密钥"
        );
        #[cfg(unix)]
        {
            assert_eq!(mode_of(&path), 0o600, "密文文件必须 0600");
            assert_eq!(
                mode_of(&dir.join("master.key")),
                0o600,
                "主密钥文件必须 0600"
            );
        }
        assert_eq!(
            read_fallback("probe-id").as_deref(),
            Some("sk-probe-secret")
        );

        delete_fallback("probe-id").unwrap();
        assert_eq!(read_fallback("probe-id"), None);
    }

    /// 历史明文文件读一次就会升级成密文，值不变。
    #[test]
    fn legacy_plaintext_is_migrated_on_read() {
        let dir = temp_dir();
        let path = dir.join("legacy-id.secret");
        std::fs::write(&path, b"legacy-secret").unwrap();

        assert_eq!(read_fallback("legacy-id").as_deref(), Some("legacy-secret"));
        let raw = std::fs::read(&path).unwrap();
        assert!(raw.starts_with(MAGIC), "读一次之后应已加密");
        assert_eq!(read_fallback("legacy-id").as_deref(), Some("legacy-secret"));
        assert!(
            !String::from_utf8_lossy(&raw).contains("legacy-secret"),
            "迁移后不应再存明文"
        );
    }

    /// 密文被改动后必须读不出来，不能把损坏数据当密钥用。
    #[test]
    fn tampered_ciphertext_is_rejected() {
        let dir = temp_dir();
        write_fallback("tamper-id", "sk-original").unwrap();
        let path = dir.join("tamper-id.secret");
        let mut raw = std::fs::read(&path).unwrap();
        let last = raw.len() - 1;
        raw[last] ^= 0b0000_0001;
        std::fs::write(&path, &raw).unwrap();

        assert_eq!(read_fallback("tamper-id"), None, "篡改后必须拒绝解密");
    }

    /// 不同槽位的密文不能互相冒充（AAD 绑定 id）。
    #[test]
    fn ciphertext_is_bound_to_slot_id() {
        let dir = temp_dir();
        write_fallback("slot-a", "sk-a").unwrap();
        let stolen = std::fs::read(dir.join("slot-a.secret")).unwrap();
        std::fs::write(dir.join("slot-b.secret"), &stolen).unwrap();

        assert_eq!(read_fallback("slot-b"), None, "换个槽位不能解开");
    }
}
