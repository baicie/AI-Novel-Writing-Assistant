//! Upstream-Sync-A1：调用观测字段的历史库升级与回读兼容。
//!
//! 迁移 0028 只新增列与索引，不改写既有行。用真实历史起点（0027）建库，
//! 写入旧结构下的调用记录，再升级到当前 schema，验证：
//! 旧记录仍可读、旧行不会被当成「已计费 0 用量」、迁移可重复打开。

#![cfg(feature = "test-harness")]

use novel_application::WorkflowRepository;
use novel_storage_sqlite::SqliteKernel;

#[tokio::test]
async fn legacy_invocations_survive_observability_upgrade() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("legacy-observability.sqlite");

    // 1. 历史起点：观测字段（0028）之前。
    let legacy = SqliteKernel::open_at_migration(&path, 27).await.unwrap();
    // 按当时的历史结构写入：只有 12 列，没有 audit_status / usage_state。
    for (id, prompt, completion, status) in [
        ("inv-known", Some(30_i64), Some(12_i64), "succeeded"),
        ("inv-empty", None, None, "succeeded"),
    ] {
        sqlx::query(
            "INSERT INTO llm_invocation(id, request_id, run_id, step_id, provider, model,
                input_summary, started_at, finished_at, prompt_tokens, completion_tokens, status)
             VALUES (?, ?, 'legacy-run', 'story_macro', 'mock', 'mock', 'x', 1, 2, ?, ?, ?)",
        )
        .bind(id)
        .bind(format!("req-{id}"))
        .bind(prompt)
        .bind(completion)
        .bind(status)
        .execute(legacy.pool())
        .await
        .unwrap();
    }
    legacy.pool().close().await;

    // 2. 升级到当前 schema。
    let upgraded = SqliteKernel::open(&path).await.unwrap();

    // 3. 新列必须带默认值落位，旧行不丢失。
    let migrated: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM schema_migration WHERE version = '0028'")
            .fetch_one(upgraded.pool())
            .await
            .unwrap();
    assert_eq!(migrated, 1, "0028 只应执行一次");
    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM llm_invocation")
        .fetch_one(upgraded.pool())
        .await
        .unwrap();
    assert_eq!(rows, 2, "升级不能丢历史调用记录");

    // 4. 旧行的 audit_status 回落到 succeeded，usage_state 回落到 known；
    //    没有用量的旧行仍是未知，不会被算成 0 计费。
    let usage = WorkflowRepository::run_usage(&upgraded, "legacy-run")
        .await
        .unwrap();
    assert_eq!(usage.calls, 2);
    assert_eq!(usage.known_calls, 2);
    assert_eq!(usage.unknown_calls, 0);
    // 汇总额来自那一行真的有用量的记录。
    assert_eq!(usage.prompt_tokens, 30);
    assert_eq!(usage.completion_tokens, 12);

    // 5. 再次打开必须幂等。
    upgraded.pool().close().await;
    let reopened = SqliteKernel::open(&path).await.unwrap();
    let usage = WorkflowRepository::run_usage(&reopened, "legacy-run")
        .await
        .unwrap();
    assert_eq!(usage.calls, 2);
    let applied: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM schema_migration WHERE version = '0028'")
            .fetch_one(reopened.pool())
            .await
            .unwrap();
    assert_eq!(applied, 1);
}
