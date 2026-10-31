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

/// 把引用名映射为文件名：清洗路径字符，并在改名时追加短哈希避免碰撞。
fn safe_stem(id: &str) -> String {
    id.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn short_hash(id: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(id.as_bytes());
    hex::encode(&digest[..4])
}

fn fallback_file(id: &str) -> Option<std::path::PathBuf> {
    let dir = FALLBACK_DIR.get()?;
    let stem = safe_stem(id);
    // 清洗会改变原名时，追加哈希，避免 "a/b" 与 "a_b" 这类引用落到同一个文件。
    let stem = if stem == id {
        stem
    } else {
        format!("{stem}-{}", short_hash(id))
    };
    Some(dir.join(format!("{stem}.secret")))
}

/// 历史文件名（没有哈希后缀）：只在读取与删除时兼容访问。
fn legacy_fallback_file(id: &str) -> Option<std::path::PathBuf> {
    let dir = FALLBACK_DIR.get()?;
    Some(dir.join(format!("{}.secret", safe_stem(id))))
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
/// 目录里是否已经存在「加密后的」密钥文件。
///
/// 只看前缀是不是 MAGIC：历史明文文件不算密文，不会被误判成「主密钥丢了」。
fn has_ciphertext(dir: &std::path::Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    entries.flatten().any(|entry| {
        let path = entry.path();
        if path
            .extension()
            .is_none_or(|extension| extension != "secret")
        {
            return false;
        }
        std::fs::read(&path).is_ok_and(|bytes| bytes.starts_with(MAGIC))
    })
}

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
    if !path.exists() && has_ciphertext(dir) {
        // 主密钥丢了但密文还在：重建只会让旧密文永久无法解密，必须显式失败。
        return Err(SecretError::Unavailable);
    }
    if let Some(key) = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| parse_key_hex(&text))
    {
        return Ok(key);
    }

    let key = random_bytes::<32>();
    let encoded = hex::encode(key);
    // 权限必须在写入敏感字节之前就正确：create_new + mode(0600) 一次到位，
    // 避免「先按 umask 建出 0644，再 chmod」之间的可读窗口。
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    match options.open(&path) {
        Ok(mut file) => {
            use std::io::Write;
            file.write_all(encoded.as_bytes())
                .map_err(|_| SecretError::Unavailable)?;
            file.flush().map_err(|_| SecretError::Unavailable)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                // 兜底：显式收紧一次，防止上层 umask 或文件系统语义差异。
                let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
                let mode = std::fs::metadata(&path)
                    .map(|meta| meta.permissions().mode() & 0o777)
                    .unwrap_or(0);
                if mode != 0o600 {
                    return Err(SecretError::Unavailable);
                }
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

    // 权限在写入敏感字节之前就建立：用临时文件 + rename，避免出现「先 0644 再 chmod」的窗口。
    let tmp = dir.join(format!(".{}.tmp", short_hash(id)));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&tmp).map_err(|_| SecretError::Unavailable)?;
    {
        use std::io::Write;
        file.write_all(&payload)
            .map_err(|_| SecretError::Unavailable)?;
        file.sync_all().map_err(|_| SecretError::Unavailable)?;
    }
    drop(file);
    std::fs::rename(&tmp, &path).map_err(|_| SecretError::Unavailable)?;
    Ok(())
}

/// 文件侧读取结果：区分「没有」「读到了」「有文件但读不出来」。
#[derive(Debug, PartialEq, Eq)]
enum FileSecret {
    Missing,
    Found(String),
    Corrupt,
}

/// 权威优先级：本地加密文件 > 历史钥匙库副本。
///
/// 文件存在但读不出来时直接失败，绝不回退到钥匙库：那里可能是一把过期版本的密钥，
/// 静默返回会让调用方拿着旧密钥去请求，表现为难以诊断的 401。
fn resolve_secret(
    file: FileSecret,
    keychain: Option<String>,
) -> Result<Option<String>, SecretError> {
    match file {
        FileSecret::Found(secret) => Ok(Some(secret)),
        FileSecret::Corrupt => Err(SecretError::Unavailable),
        FileSecret::Missing => Ok(keychain),
    }
}

fn read_fallback_state(id: &str) -> FileSecret {
    let path = match fallback_file(id) {
        Some(path) if path.exists() => path,
        // 兼容升级前写入的文件名（没有哈希后缀）
        _ => match legacy_fallback_file(id).filter(|path| path.exists()) {
            Some(path) => path,
            None => return FileSecret::Missing,
        },
    };
    let Ok(raw) = std::fs::read(&path) else {
        return FileSecret::Corrupt;
    };
    if raw.len() < MAGIC.len() + 12 || !raw.starts_with(MAGIC) {
        // 历史明文：读得出来就顺手加密写回，完成迁移。
        return match String::from_utf8(raw)
            .ok()
            .filter(|value| !value.is_empty())
        {
            Some(legacy) => {
                let _ = write_fallback(id, &legacy);
                FileSecret::Found(legacy)
            }
            None => FileSecret::Corrupt,
        };
    }
    let Some(dir) = path.parent() else {
        return FileSecret::Corrupt;
    };
    let Ok(key) = master_key(dir) else {
        return FileSecret::Corrupt;
    };
    let cipher = ChaCha20Poly1305::new((&key).into());
    let Ok(nonce_bytes) = <[u8; 12]>::try_from(&raw[MAGIC.len()..MAGIC.len() + 12]) else {
        return FileSecret::Corrupt;
    };
    let nonce = Nonce::from_slice(&nonce_bytes);
    let Ok(plain) = cipher.decrypt(
        nonce,
        Payload {
            msg: &raw[MAGIC.len() + 12..],
            aad: id.as_bytes(),
        },
    ) else {
        return FileSecret::Corrupt;
    };
    match String::from_utf8(plain)
        .ok()
        .filter(|value| !value.is_empty())
    {
        Some(secret) => FileSecret::Found(secret),
        None => FileSecret::Corrupt,
    }
}

fn read_fallback(id: &str) -> Option<String> {
    match read_fallback_state(id) {
        FileSecret::Found(secret) => Some(secret),
        FileSecret::Missing | FileSecret::Corrupt => None,
    }
}

fn delete_fallback(id: &str) -> Result<(), SecretError> {
    if let Some(path) = fallback_file(id) {
        let _ = std::fs::remove_file(path);
    }
    if let Some(path) = legacy_fallback_file(id) {
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
        // 本地加密文件是唯一权威；钥匙库只用来兼容升级前只写进钥匙库的老数据。
        // 文件存在却解不开时直接失败，不能让可能过期的钥匙库副本悄悄顶上去。
        let file = read_fallback_state(id);
        let keychain = match &file {
            // 只有文件缺失时才去问钥匙库，省掉一次无意义的钥匙库调用。
            FileSecret::Missing => keyring::Entry::new("Biz Novel Studio", id)
                .ok()
                .and_then(|entry| entry.get_password().ok()),
            _ => None,
        };
        resolve_secret(file, keychain)
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

    /// 清洗后会重名的两个引用必须落到不同文件。
    #[test]
    fn reference_names_do_not_collide_after_sanitising() {
        let dir = temp_dir();
        write_fallback("a/b", "sk-slash").unwrap();
        write_fallback("a_b", "sk-underscore").unwrap();
        assert_eq!(read_fallback("a/b").as_deref(), Some("sk-slash"));
        assert_eq!(read_fallback("a_b").as_deref(), Some("sk-underscore"));
        let count = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "secret"))
            .count();
        assert!(count >= 2, "两个引用必须写到两个文件，实际 {count}");
    }

    /// 主密钥丢失但密文还在时，不允许悄悄重建（否则旧密文永久解不开）。
    #[test]
    fn missing_master_key_is_not_silently_recreated() {
        let dir = std::env::temp_dir().join(format!("p16-master-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // 不调用 configure_secret_fallback：它会改全局回退目录，影响其它用例。

        // 构造一份「只有密文、没有主密钥」的目录。
        let ciphertext = [MAGIC, &[0_u8; 12], &[1_u8; 16]].concat();
        std::fs::write(dir.join("orphan.secret"), ciphertext).unwrap();
        assert!(master_key(&dir).is_err(), "有密文时必须拒绝重建主密钥");

        let _ = std::fs::remove_dir_all(&dir);
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

#[cfg(test)]
mod authority_tests {
    use super::*;

    /// 与 fallback_tests 共用同一个进程级目录，且**不删除**它：
    /// 这些用例并行执行，删除目录会让别的用例突然读不到主密钥。
    fn shared_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("p16-secrets-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        configure_secret_fallback(dir.clone());
        dir
    }

    #[test]
    fn local_file_wins_over_stale_keychain_copy() {
        let resolved = resolve_secret(
            FileSecret::Found("sk-current".into()),
            Some("sk-stale-from-keychain".into()),
        );
        assert_eq!(resolved.unwrap().as_deref(), Some("sk-current"));
    }

    #[test]
    fn corrupt_file_fails_instead_of_falling_back_to_keychain() {
        let resolved = resolve_secret(FileSecret::Corrupt, Some("sk-old".into()));
        assert!(
            resolved.is_err(),
            "文件损坏时不能静默返回可能过期的钥匙库副本"
        );
    }

    #[test]
    fn missing_file_still_reads_legacy_keychain_only_data() {
        let resolved = resolve_secret(FileSecret::Missing, Some("sk-legacy".into()));
        assert_eq!(resolved.unwrap().as_deref(), Some("sk-legacy"));
        assert_eq!(resolve_secret(FileSecret::Missing, None).unwrap(), None);
    }

    #[tokio::test]
    async fn rotated_slots_keep_old_and_new_readable() {
        shared_dir();
        write_fallback("authority-gw#1", "key-v1").unwrap();
        write_fallback("authority-gw#2", "key-v2").unwrap();
        assert_eq!(read_fallback("authority-gw#1").as_deref(), Some("key-v1"));
        assert_eq!(read_fallback("authority-gw#2").as_deref(), Some("key-v2"));

        write_fallback("authority-gw#2", "key-v2b").unwrap();
        assert_eq!(read_fallback("authority-gw#2").as_deref(), Some("key-v2b"));
        assert_eq!(read_fallback("authority-gw#1").as_deref(), Some("key-v1"));
    }

    #[tokio::test]
    async fn corrupted_ciphertext_is_reported_as_corrupt_not_missing() {
        shared_dir();
        write_fallback("authority-bad#9", "key-v9").unwrap();
        let path = fallback_file("authority-bad#9").unwrap();
        let mut raw = std::fs::read(&path).unwrap();
        let last = raw.len() - 1;
        raw[last] ^= 0xff;
        std::fs::write(&path, raw).unwrap();

        assert_eq!(read_fallback_state("authority-bad#9"), FileSecret::Corrupt);
        assert_eq!(read_fallback("authority-bad#9"), None);
    }

    #[cfg(unix)]
    #[test]
    fn master_key_file_is_created_with_owner_only_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("p16-master-mode-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let _ = master_key(&dir).unwrap();
        let mode = std::fs::metadata(dir.join("master.key"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "主密钥文件必须是 0600，实际 {mode:o}");
    }
}
