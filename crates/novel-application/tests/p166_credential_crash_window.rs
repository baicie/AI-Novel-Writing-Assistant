//! R8.5：凭据 CAS / Vault 崩溃窗口（真实 SQLite + 真实进程）。
//!
//! 覆盖两件事：
//!   1) Vault 写入失败时数据库必须补偿回旧配置，旧密钥仍可用；
//!   2) 在数据库提交前被 SIGKILL，不得留下半条配置或新密钥槽引用。

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use novel_application::{
    CredentialAction, CredentialIntents, CredentialVault, PendingCredentialIntent, PortError,
    ProviderDraft, ProviderRegistry, ProviderService,
};
use novel_domain::ProviderProtocol;
use novel_storage_sqlite::SqliteKernel;

/// 正常的测试租约库：记录引用 → 密钥。
#[derive(Default)]
struct TestVault {
    values: std::sync::Mutex<std::collections::HashMap<String, String>>,
}

#[async_trait]
impl CredentialVault for TestVault {
    async fn write(&self, reference: &str, secret: &str) -> Result<(), PortError> {
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

/// 写入永久失败的租约库：用来模拟「数据库已提交、密钥写不进去」。
struct FailingVault {
    inner: TestVault,
    attempts: AtomicUsize,
}

#[async_trait]
impl CredentialVault for FailingVault {
    async fn write(&self, _reference: &str, _secret: &str) -> Result<(), PortError> {
        self.attempts.fetch_add(1, Ordering::SeqCst);
        Err(PortError::Storage("密钥库写入失败（测试注入）".into()))
    }
    async fn delete(&self, reference: &str) -> Result<(), PortError> {
        self.inner.delete(reference).await
    }
    async fn present(&self, reference: &str) -> Result<bool, PortError> {
        self.inner.present(reference).await
    }
}

#[derive(Default)]
struct TestIntents {
    finished: std::sync::Mutex<Vec<(String, String)>>,
}

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
        id: &str,
        state: &str,
        _last_error: Option<&str>,
        _now_ms: i64,
    ) -> Result<(), PortError> {
        self.finished
            .lock()
            .unwrap()
            .push((id.to_string(), state.to_string()));
        Ok(())
    }
    async fn pending(&self) -> Result<Vec<PendingCredentialIntent>, PortError> {
        Ok(Vec::new())
    }
}

fn draft(base_url: &str) -> ProviderDraft {
    ProviderDraft {
        id: "gw".into(),
        display_name: Some("凭据网关".into()),
        protocol: ProviderProtocol::OpenAiCompatible,
        base_url: base_url.into(),
        allow_localhost_http: false,
        is_builtin: false,
        sort_order: 0,
    }
}

#[tokio::test]
async fn vault_write_failure_keeps_old_credential_readable() {
    let directory = tempfile::tempdir().unwrap();
    let kernel = Arc::new(
        SqliteKernel::open(&directory.path().join("vault-fail.sqlite"))
            .await
            .unwrap(),
    );

    // 先用正常库写入 v1。
    let good = Arc::new(TestVault::default());
    let service = ProviderService::new(
        kernel.clone() as Arc<dyn ProviderRegistry>,
        good.clone() as Arc<dyn CredentialVault>,
        Arc::new(TestIntents::default()),
    );
    service
        .upsert_provider(
            draft("https://gw.example.com/v1"),
            CredentialAction::Replace("key-v1".into()),
            None,
            1,
        )
        .await
        .unwrap();
    assert_eq!(
        good.values.lock().unwrap().get("gw#1").cloned(),
        Some("key-v1".into())
    );

    // 换成会失败的库再保存：数据库必须补偿回旧配置。
    let failing = Arc::new(FailingVault {
        inner: TestVault::default(),
        attempts: AtomicUsize::new(0),
    });
    let service = ProviderService::new(
        kernel.clone() as Arc<dyn ProviderRegistry>,
        failing.clone() as Arc<dyn CredentialVault>,
        Arc::new(TestIntents::default()),
    );
    let error = service
        .upsert_provider(
            draft("https://gw2.example.com/v1"),
            CredentialAction::Replace("key-v2".into()),
            Some(1),
            2,
        )
        .await
        .expect_err("密钥写不进去时必须报错");
    assert_eq!(failing.attempts.load(Ordering::SeqCst), 1);

    // 数据库不能指向不存在的密钥槽：必须还是旧引用。
    let rows: Vec<(String, i64)> =
        sqlx::query_as("SELECT secret_ref, revision FROM provider_credential WHERE id='gw'")
            .fetch_all(kernel.pool())
            .await
            .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0, "gw#1", "失败后必须回到旧密钥槽：{rows:?}");
    // revision 是 CAS 版本号：补偿回滚本身也是一次写入，因此会继续前进；
    // 关键是配置内容必须回到旧值，且数据库不能指向不存在的密钥槽。
    assert!(rows[0].1 >= 2, "补偿写入应当发生：{rows:?}");

    // 旧密钥仍然可读，旧 Run 仍能生成。
    assert_eq!(
        good.values.lock().unwrap().get("gw#1").cloned(),
        Some("key-v1".into())
    );
    let provider = service.get_provider("gw").await.unwrap();
    assert_eq!(
        provider.base_url, "https://gw.example.com/v1",
        "失败后地址必须回到旧值"
    );
    assert!(
        failing.inner.values.lock().unwrap().is_empty(),
        "失败者不得在任何槽位留下密钥"
    );
    let _ = error;
}

#[tokio::test]
async fn crash_before_commit_leaves_provider_untouched() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("vault-crash.sqlite");
    let db = path.to_string_lossy().to_string();

    // 先建立初始配置。
    let kernel = Arc::new(SqliteKernel::open(&path).await.unwrap());
    let good = Arc::new(TestVault::default());
    let service = ProviderService::new(
        kernel.clone() as Arc<dyn ProviderRegistry>,
        good.clone() as Arc<dyn CredentialVault>,
        Arc::new(TestIntents::default()),
    );
    service
        .upsert_provider(
            draft("https://gw.example.com/v1"),
            CredentialAction::Replace("key-v1".into()),
            None,
            1,
        )
        .await
        .unwrap();
    drop(service);

    // 父进程持有写锁，让子进程的保存停在提交之前。
    let mut lock = kernel.pool().acquire().await.unwrap();
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut *lock)
        .await
        .unwrap();

    let exe = std::env::current_exe().unwrap();
    let mut child = std::process::Command::new(exe)
        .args(["--exact", "credential_child_process", "--nocapture"])
        .env("P166_C_DB", &db)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(900)).await;
    let _ = child.kill();
    let _ = child.wait();
    let _ = sqlx::query("ROLLBACK").execute(&mut *lock).await;
    drop(lock);

    // 数据库保持旧状态，没有半条新配置。
    let rows: Vec<(String, i64)> =
        sqlx::query_as("SELECT secret_ref, revision FROM provider_credential WHERE id='gw'")
            .fetch_all(kernel.pool())
            .await
            .unwrap();
    assert_eq!(rows, vec![("gw#1".to_string(), 1)], "崩溃后不得出现新版本");
    let integrity: String = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(kernel.pool())
        .await
        .unwrap();
    assert_eq!(integrity, "ok");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn credential_child_process() {
    let Some(db) = std::env::var("P166_C_DB").ok() else {
        return;
    };
    let kernel = Arc::new(SqliteKernel::open(std::path::Path::new(&db)).await.unwrap());
    let vault = Arc::new(TestVault::default());
    let service = ProviderService::new(
        kernel as Arc<dyn ProviderRegistry>,
        vault as Arc<dyn CredentialVault>,
        Arc::new(TestIntents::default()),
    );
    let _ = service
        .upsert_provider(
            draft("https://gw3.example.com/v1"),
            CredentialAction::Replace("key-v3".into()),
            Some(1),
            3,
        )
        .await;
    tokio::time::sleep(std::time::Duration::from_secs(600)).await;
}
