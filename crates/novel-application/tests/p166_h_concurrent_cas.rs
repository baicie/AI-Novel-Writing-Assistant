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
