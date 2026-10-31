//! R6-H：同一 revision 上的并发保存（真实 SQLite），验证赢家唯一且失败者不碰密钥库。

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use novel_application::{
    CredentialAction, CredentialIntents, CredentialVault, PendingCredentialIntent, PortError,
    ProviderDraft, ProviderRegistry, ProviderService,
};
use novel_domain::ProviderProtocol;
use novel_storage_sqlite::SqliteKernel;

#[derive(Default)]
struct TestVault {
    values: std::sync::Mutex<std::collections::HashMap<String, String>>,
    writes: AtomicUsize,
}

#[async_trait]
impl CredentialVault for TestVault {
    async fn write(&self, reference: &str, secret: &str) -> Result<(), PortError> {
        self.writes.fetch_add(1, Ordering::SeqCst);
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

#[derive(Default)]
struct TestIntents;

#[async_trait]
impl CredentialIntents for TestIntents {
    async fn begin(
        &self,
        _id: &str,
        _provider_id: &str,
        _secret_ref: &str,
        _action: &str,
        _now_ms: i64,
    ) -> Result<(), PortError> {
        Ok(())
    }
    async fn finish(
        &self,
        _id: &str,
        _state: &str,
        _last_error: Option<&str>,
        _now_ms: i64,
    ) -> Result<(), PortError> {
        Ok(())
    }
    async fn pending(&self) -> Result<Vec<PendingCredentialIntent>, PortError> {
        Ok(Vec::new())
    }
}

fn draft(base_url: &str) -> ProviderDraft {
    ProviderDraft {
        id: "gw".into(),
        display_name: Some("并发网关".into()),
        protocol: ProviderProtocol::OpenAiCompatible,
        base_url: base_url.into(),
        allow_localhost_http: false,
        is_builtin: false,
        sort_order: 0,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_saves_with_same_revision_only_change_one_slot() {
    let directory = tempfile::tempdir().unwrap();
    let kernel = Arc::new(
        SqliteKernel::open(&directory.path().join("cas.sqlite"))
            .await
            .unwrap(),
    );
    let vault = Arc::new(TestVault::default());
    let service = ProviderService::new(
        kernel.clone() as Arc<dyn ProviderRegistry>,
        vault.clone() as Arc<dyn CredentialVault>,
        Arc::new(TestIntents),
    );

    // 初始配置：revision 1，密钥 key-old。
    service
        .upsert_provider(
            draft("https://gw.example.com/v1"),
            CredentialAction::Replace("key-old".into()),
            None,
            1,
        )
        .await
        .unwrap();
    assert_eq!(
        vault.values.lock().unwrap().get("gw#1").cloned(),
        Some("key-old".to_string())
    );

    // 两个服务实例共享同一个数据库与密钥库，用同一 revision 同时保存。
    let barrier = Arc::new(tokio::sync::Barrier::new(2));
    let mut handles = Vec::new();
    for (key, url) in [
        ("key-alpha", "https://alpha.example.com/v1"),
        ("key-beta", "https://beta.example.com/v1"),
    ] {
        let service = ProviderService::new(
            kernel.clone() as Arc<dyn ProviderRegistry>,
            vault.clone() as Arc<dyn CredentialVault>,
            Arc::new(TestIntents),
        );
        let barrier = barrier.clone();
        handles.push(tokio::spawn(async move {
            barrier.wait().await;
            let result = service
                .upsert_provider(
                    draft(url),
                    CredentialAction::Replace(key.into()),
                    Some(1),
                    2,
                )
                .await;
            (key, result)
        }));
    }

    let mut winners = Vec::new();
    let mut losers = Vec::new();
    for handle in handles {
        let (key, result) = handle.await.unwrap();
        match result {
            Ok(view) => winners.push((key, view.revision)),
            Err(error) => losers.push((key, error.code)),
        }
    }

    assert_eq!(
        winners.len(),
        1,
        "同 revision 并发保存只能有一个成功：{winners:?} {losers:?}"
    );
    assert_eq!(losers.len(), 1, "另一个必须失败");
    assert_eq!(winners[0].1, 2, "赢家应把版本推进到 2");

    // 赢家的密钥写在 gw#2；失败者的密钥不能出现在任何槽位里。
    let values = vault.values.lock().unwrap().clone();
    assert_eq!(
        values.get("gw#2").cloned(),
        Some(winners[0].0.to_string()),
        "gw#2 必须是赢家的密钥，实际 {values:?}"
    );
    assert_eq!(
        values.get("gw#1").cloned(),
        Some("key-old".to_string()),
        "旧槽不能被覆盖"
    );
    let loser_key = losers[0].0;
    assert!(
        !values.values().any(|value| value == loser_key),
        "失败者的密钥不得写入任何槽位：{values:?}"
    );
    drop(values);

    // 数据库里只有一条记录，且指向赢家的槽位。
    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM provider_credential WHERE id = 'gw'")
        .fetch_one(kernel.pool())
        .await
        .unwrap();
    assert_eq!(rows, 1, "并发保存不能复制出多条配置");
    let reference: String =
        sqlx::query_scalar("SELECT secret_ref FROM provider_credential WHERE id = 'gw'")
            .fetch_one(kernel.pool())
            .await
            .unwrap();
    assert_eq!(reference, "gw#2");

    let integrity: String = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(kernel.pool())
        .await
        .unwrap();
    assert_eq!(integrity, "ok");
}

// ============================================================
// R6-H（进程级）：两个独立进程用同一 revision 并发保存同一 Provider。
// 子进程复用本测试二进制，通过环境变量传入数据库、密钥文件与统一起跑时刻。
// ============================================================

/// 文件版密钥库：跨进程共享，写入用「临时文件 + rename」保证原子。
struct FileVault {
    path: std::path::PathBuf,
}

impl FileVault {
    fn load(&self) -> std::collections::HashMap<String, String> {
        std::fs::read_to_string(&self.path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    fn store(&self, values: &std::collections::HashMap<String, String>) -> std::io::Result<()> {
        let tmp = self.path.with_extension("tmp");
        std::fs::write(&tmp, serde_json::to_string(values).unwrap())?;
        std::fs::rename(&tmp, &self.path)
    }
}

#[async_trait]
impl CredentialVault for FileVault {
    async fn write(&self, reference: &str, secret: &str) -> Result<(), PortError> {
        let mut values = self.load();
        values.insert(reference.to_string(), secret.to_string());
        self.store(&values)
            .map_err(|error| PortError::Storage(format!("写入测试密钥文件失败：{error}")))
    }
    async fn delete(&self, reference: &str) -> Result<(), PortError> {
        let mut values = self.load();
        values.remove(reference);
        let _ = self.store(&values);
        Ok(())
    }
    async fn present(&self, reference: &str) -> Result<bool, PortError> {
        Ok(self.load().contains_key(reference))
    }
}

fn h_env() -> Option<(String, String, String, String, i64, i64)> {
    let db = std::env::var("P166_H_DB").ok()?;
    let vault = std::env::var("P166_H_VAULT").ok()?;
    let key = std::env::var("P166_H_KEY").ok()?;
    let url = std::env::var("P166_H_URL").ok()?;
    let expected: i64 = std::env::var("P166_H_EXPECTED").ok()?.parse().ok()?;
    let start_ms: i64 = std::env::var("P166_H_START_MS").ok()?.parse().ok()?;
    Some((db, vault, key, url, expected, start_ms))
}

/// 子进程主体：只有环境变量存在时才真正执行。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn h_child_process() {
    let Some((db, vault_path, key, url, expected, start_ms)) = h_env() else {
        return;
    };
    let kernel = Arc::new(SqliteKernel::open(std::path::Path::new(&db)).await.unwrap());
    let vault = Arc::new(FileVault {
        path: std::path::PathBuf::from(vault_path),
    });
    let service = ProviderService::new(
        kernel as Arc<dyn ProviderRegistry>,
        vault as Arc<dyn CredentialVault>,
        Arc::new(TestIntents),
    );

    // 两个进程等到同一个时刻再提交，制造真实竞争。
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    if start_ms > now {
        tokio::time::sleep(std::time::Duration::from_millis((start_ms - now) as u64)).await;
    }

    match service
        .upsert_provider(
            draft(&url),
            CredentialAction::Replace(key),
            Some(expected),
            2,
        )
        .await
    {
        Ok(_) => std::process::exit(0),
        Err(error) if error.code == novel_application::ProviderErrorCode::Conflict => {
            std::process::exit(2)
        }
        Err(_) => std::process::exit(1),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_processes_racing_on_the_same_revision_keep_one_key() {
    let directory = tempfile::tempdir().unwrap();
    let db_path = directory.path().join("race.sqlite");
    let vault_path = directory.path().join("vault.json");
    let db = db_path.to_string_lossy().to_string();
    let vault_arg = vault_path.to_string_lossy().to_string();

    // 初始配置：revision 1，密钥 key-old（直接写库与文件，避免依赖子进程）。
    let kernel = Arc::new(SqliteKernel::open(&db_path).await.unwrap());
    let vault = Arc::new(FileVault {
        path: vault_path.clone(),
    });
    let service = ProviderService::new(
        kernel.clone() as Arc<dyn ProviderRegistry>,
        vault as Arc<dyn CredentialVault>,
        Arc::new(TestIntents),
    );
    service
        .upsert_provider(
            draft("https://gw.example.com/v1"),
            CredentialAction::Replace("key-old".into()),
            None,
            1,
        )
        .await
        .unwrap();
    drop(service);
    drop(kernel);

    let start_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
        + 1200;
    let exe = std::env::current_exe().unwrap();
    let mut children = Vec::new();
    for (key, url) in [
        ("key-alpha", "https://alpha.example.com/v1"),
        ("key-beta", "https://beta.example.com/v1"),
    ] {
        children.push(
            std::process::Command::new(&exe)
                .args(["--exact", "h_child_process", "--nocapture"])
                .env("P166_H_DB", &db)
                .env("P166_H_VAULT", &vault_arg)
                .env("P166_H_KEY", key)
                .env("P166_H_URL", url)
                .env("P166_H_EXPECTED", "1")
                .env("P166_H_START_MS", start_ms.to_string())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .unwrap(),
        );
    }

    let mut codes = Vec::new();
    for mut child in children {
        codes.push(child.wait().unwrap().code().unwrap_or(-1));
    }
    codes.sort_unstable();
    assert_eq!(
        codes,
        vec![0, 2],
        "同时用同一 revision 保存：必须一个成功、一个冲突，实际 {codes:?}"
    );

    // 密钥文件：赢家的密钥在 gw#2，失败者的密钥没有出现在任何槽位。
    let values: std::collections::HashMap<String, String> =
        serde_json::from_str(&std::fs::read_to_string(&vault_path).unwrap()).unwrap();
    let stored = values.get("gw#2").cloned().expect("赢家必须写入 gw#2");
    assert!(
        stored == "key-alpha" || stored == "key-beta",
        "gw#2 必须是某个赢家的密钥：{values:?}"
    );
    let loser = if stored == "key-alpha" {
        "key-beta"
    } else {
        "key-alpha"
    };
    assert!(
        !values.values().any(|value| value == loser),
        "失败者的密钥不得写入任何槽位：{values:?}"
    );
    assert_eq!(
        values.get("gw#1").cloned(),
        Some("key-old".to_string()),
        "旧槽不能被覆盖"
    );

    // 数据库：一条配置、指向赢家槽位、结构完好。
    let kernel = Arc::new(SqliteKernel::open(&db_path).await.unwrap());
    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM provider_credential WHERE id = 'gw'")
        .fetch_one(kernel.pool())
        .await
        .unwrap();
    assert_eq!(rows, 1);
    let reference: String =
        sqlx::query_scalar("SELECT secret_ref FROM provider_credential WHERE id = 'gw'")
            .fetch_one(kernel.pool())
            .await
            .unwrap();
    assert_eq!(reference, "gw#2");
    let revision: i64 =
        sqlx::query_scalar("SELECT revision FROM provider_credential WHERE id = 'gw'")
            .fetch_one(kernel.pool())
            .await
            .unwrap();
    assert_eq!(revision, 2);
    let integrity: String = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(kernel.pool())
        .await
        .unwrap();
    assert_eq!(integrity, "ok");
}
