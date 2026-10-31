//! R2.1-J：删除模型来源时的「未完成任务」保护。

use novel_storage_sqlite::SqliteKernel;

async fn seed_provider(kernel: &SqliteKernel, id: &str) {
    use novel_domain::{CredentialKind, CredentialReference, ProviderProfile, ProviderProtocol};
    let profile = ProviderProfile {
        id: id.into(),
        display_name: Some(id.into()),
        protocol: ProviderProtocol::OpenAiCompatible,
        base_url: "https://gw.example.com/v1".into(),
        credential: CredentialReference {
            kind: CredentialKind::OsSecret,
            reference: format!("{id}#1"),
        },
        is_builtin: false,
        allow_localhost_http: false,
        sort_order: 0,
    };
    kernel.upsert_provider(&profile, 1).await.unwrap();
}

async fn seed_run(kernel: &SqliteKernel, run_id: &str, provider: &str, status: &str) {
    // novel 表有多个必填列，这里只在缺失时按最小字段建一本。
    let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM novel WHERE id = 'n1'")
        .fetch_one(kernel.pool())
        .await
        .unwrap();
    if exists == 0 {
        sqlx::query(
            "INSERT INTO novel(id, title, created_at, updated_at) VALUES ('n1', 't', 1, 1)",
        )
        .execute(kernel.pool())
        .await
        .unwrap();
    }
    let contract = format!(
        r#"{{"run_id":"{run_id}","provider":"{provider}","credential_ref":"{provider}#1"}}"#
    );
    sqlx::query(
        "INSERT INTO workflow_run(id, novel_id, plan_version, contract_json, created_at)
         VALUES (?, 'n1', 'v1', ?, 1)",
    )
    .bind(run_id)
    .bind(contract)
    .execute(kernel.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO workflow_control(run_id, novel_id, status, version)
         VALUES (?, 'n1', ?, 0)",
    )
    .bind(run_id)
    .bind(status)
    .execute(kernel.pool())
    .await
    .unwrap();
}

#[tokio::test]
async fn deleting_provider_with_paused_run_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = SqliteKernel::open(&dir.path().join("delete-guard.sqlite"))
        .await
        .unwrap();
    seed_provider(&kernel, "gw").await;
    seed_run(&kernel, "r-paused", "gw", "paused").await;

    let error = kernel
        .delete_provider_guarded("gw", false)
        .await
        .expect_err("还有暂停任务时不允许删除");
    match error {
        novel_application::PortError::Conflict(message) => {
            assert!(message.contains("未完成"), "提示要说明原因：{message}");
        }
        other => panic!("应为 Conflict，实际 {other:?}"),
    }
    assert_eq!(
        kernel.list_providers().await.unwrap().len(),
        1,
        "拒绝删除时数据必须还在"
    );
}

#[tokio::test]
async fn waiting_approval_and_queued_runs_also_block_deletion() {
    for status in ["waiting_approval", "queued", "running"] {
        let dir = tempfile::tempdir().unwrap();
        let kernel = SqliteKernel::open(&dir.path().join("guard2.sqlite"))
            .await
            .unwrap();
        seed_provider(&kernel, "gw").await;
        seed_run(&kernel, "r-1", "gw", status).await;
        assert!(
            kernel.delete_provider_guarded("gw", false).await.is_err(),
            "{status} 状态必须阻止删除"
        );
    }
}

#[tokio::test]
async fn finished_run_does_not_block_deletion() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = SqliteKernel::open(&dir.path().join("guard3.sqlite"))
        .await
        .unwrap();
    seed_provider(&kernel, "gw").await;
    seed_run(&kernel, "r-done", "gw", "succeeded").await;

    kernel
        .delete_provider_guarded("gw", false)
        .await
        .expect("已完成的 Run 不应阻止删除");
    assert!(kernel.list_providers().await.unwrap().is_empty());
}

#[tokio::test]
async fn force_still_allows_deletion_of_provider_with_active_run() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = SqliteKernel::open(&dir.path().join("guard4.sqlite"))
        .await
        .unwrap();
    seed_provider(&kernel, "gw").await;
    seed_run(&kernel, "r-paused", "gw", "paused").await;

    kernel
        .delete_provider_guarded("gw", true)
        .await
        .expect("force 必须能删除，但由调用方承担后果");
    assert!(kernel.list_providers().await.unwrap().is_empty());
    // Run 记录仍在：历史审计不被删除动作抹掉。
    let remaining: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM workflow_run WHERE id = 'r-paused'")
            .fetch_one(kernel.pool())
            .await
            .unwrap();
    assert_eq!(remaining, 1);
}
