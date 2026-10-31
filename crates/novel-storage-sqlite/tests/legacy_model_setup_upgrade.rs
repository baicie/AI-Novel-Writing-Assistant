//! R3.1-B1：历史「模型设置」升级到 Provider 注册表后的兼容性。
//!
//! 用真实的历史 schema 起点（迁移 21，注册表上线之前）建库，再升级到当前 schema，
//! 验证：旧配置可读、密钥槽仍是历史默认槽、连续两次重启迁移幂等、数据库完好。

#![cfg(feature = "test-harness")]

use novel_storage_sqlite::{MODEL_SECRET_REF, SqliteKernel};

async fn count_rows(kernel: &SqliteKernel, table: &str) -> i64 {
    sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
        .fetch_one(kernel.pool())
        .await
        .unwrap()
}

#[tokio::test]
async fn legacy_model_setup_survives_upgrade_and_restarts() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("legacy-setup.sqlite");

    // 1. 历史起点：Provider 注册表（0022）之前。
    let legacy = SqliteKernel::open_at_migration(&path, 21).await.unwrap();
    // 迁移 21 的表结构里还没有 revision / protocol 等新列，必须按当时的历史结构写入，
    // 否则就是用当前 schema 的 API 倒推伪造旧库。
    sqlx::query(
        "INSERT INTO provider_credential(id, provider, secret_ref, endpoint, model, allow_localhost_http, created_at)
         VALUES ('default', 'openai-compatible', ?, 'https://api.example.com/v1', 'legacy-model', 0, 1)",
    )
    .bind(novel_storage_sqlite::MODEL_SECRET_REF)
    .execute(legacy.pool())
    .await
    .unwrap();
    legacy.pool().close().await;

    // 2. 第一次升级。
    let upgraded = SqliteKernel::open(&path).await.unwrap();
    let setup = upgraded
        .load_model_setup()
        .await
        .unwrap()
        .expect("升级后旧配置必须还在");
    assert_eq!(setup.endpoint, "https://api.example.com/v1");
    assert_eq!(setup.model, "legacy-model");
    assert!(!setup.allow_localhost_http);
    assert_eq!(
        setup.secret_ref, MODEL_SECRET_REF,
        "历史配置仍然指向历史默认密钥槽，不能改写成别的 provider 槽位"
    );

    let integrity: String = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(upgraded.pool())
        .await
        .unwrap();
    assert_eq!(integrity, "ok");
    let fk: Vec<String> = sqlx::query_scalar("PRAGMA foreign_key_check")
        .fetch_all(upgraded.pool())
        .await
        .unwrap();
    assert!(fk.is_empty(), "外键必须完好：{fk:?}");

    let providers_after_first = count_rows(&upgraded, "provider_credential").await;
    let migration_after_first = count_rows(&upgraded, "schema_migration").await;
    upgraded.pool().close().await;

    // 3. 第二次启动：迁移必须幂等，配置与行数都不变。
    let restarted = SqliteKernel::open(&path).await.unwrap();
    let again = restarted.load_model_setup().await.unwrap().unwrap();
    assert_eq!(again.endpoint, setup.endpoint);
    assert_eq!(again.model, setup.model);
    assert_eq!(again.secret_ref, setup.secret_ref);
    assert_eq!(
        count_rows(&restarted, "provider_credential").await,
        providers_after_first,
        "重复启动不能复制配置行"
    );
    assert_eq!(
        count_rows(&restarted, "schema_migration").await,
        migration_after_first,
        "迁移记录不能重复写入"
    );
    let versions: Vec<String> =
        sqlx::query_scalar("SELECT version FROM schema_migration ORDER BY version")
            .fetch_all(restarted.pool())
            .await
            .unwrap();
    let mut deduped = versions.clone();
    deduped.dedup();
    assert_eq!(deduped, versions, "迁移版本必须唯一");
    assert!(
        versions.iter().any(|version| version == "0022"),
        "必须执行到 Provider 注册表迁移"
    );
}
