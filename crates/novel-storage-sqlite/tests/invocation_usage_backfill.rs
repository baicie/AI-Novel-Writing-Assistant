//! Sync-A1-R2：历史调用记录的用量分类正确性。
//!
//! 迁移 0028 给 `usage_state` 加了 `NOT NULL DEFAULT 'known'` 默认值，于是
//! **0010–0027 期间所有历史调用行**（包括 provider 没返回 usage、token 全为 NULL
//! 的行）升级后都被标成 `known`。结果是书级 / 运行级用量把「未知」当成「已确认」，
//! `known_calls` 虚高、`prompt_tokens` 被 0 掩盖。
//!
//! 0028 已经发布，不能在原地改写它；修正必须是一条新的版本化迁移 0029。
//!
//! 本套件锁定以下契约：
//!   1. 历史有完整用量的行仍是 `known`；
//!   2. 历史没有任何用量的行降级为 `unknown`；
//!   3. 部分字段缺失的行不得算作「完整已知」；
//!   4. 总调用条数不变，历史记录一条不丢；
//!   5. 迁移可重复执行，重复启动幂等；
//!   6. 已经由 0028 之后的新代码写入的 `known` / `unbilled` 行不被 0029 误改。

#![cfg(feature = "test-harness")]

use novel_application::WorkflowRepository;
use novel_storage_sqlite::SqliteKernel;

/// 在 0027 的历史结构上插入一批调用记录（没有 usage_state 列）。
async fn seed_legacy_invocations(kernel: &SqliteKernel) {
    // (id, prompt, completion, status)
    let rows: [(&str, Option<i64>, Option<i64>, &str); 5] = [
        ("inv-full", Some(30), Some(12), "succeeded"),
        ("inv-null", None, None, "succeeded"),
        ("inv-prompt-only", Some(7), None, "succeeded"),
        ("inv-completion-only", None, Some(4), "succeeded"),
        ("inv-failed-null", None, None, "unknown"),
    ];
    for (id, prompt, completion, status) in rows {
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
        .execute(kernel.pool())
        .await
        .unwrap();
    }
}

async fn build_legacy_database(dir: &tempfile::TempDir) -> std::path::PathBuf {
    let path = dir.path().join("legacy-usage.sqlite");
    let legacy = SqliteKernel::open_at_migration(&path, 27).await.unwrap();
    seed_legacy_invocations(&legacy).await;
    legacy.pool().close().await;
    path
}

/// 复现用：历史 NULL 用量行在升级后被算进 known。
#[tokio::test]
async fn legacy_rows_without_usage_are_not_counted_as_known() {
    let dir = tempfile::tempdir().unwrap();
    let path = build_legacy_database(&dir).await;
    let upgraded = SqliteKernel::open(&path).await.unwrap();

    let usage = WorkflowRepository::run_usage(&upgraded, "legacy-run")
        .await
        .unwrap();

    // 总条数不变：历史记录一条不丢。
    assert_eq!(usage.calls, 5, "升级不能丢弃历史调用记录");
    // 只有两列都有值的行才算「完整已知」。
    assert_eq!(
        usage.known_calls, 1,
        "只有 prompt 与 completion 都存在的历史行才是 known"
    );
    assert_eq!(
        usage.unknown_calls, 4,
        "NULL 用量与部分字段缺失的历史行都属于未知"
    );
    assert_eq!(usage.unbilled_calls, 0);
    // 汇总额只来自真正完整的那一行。
    assert_eq!(usage.prompt_tokens, 30);
    assert_eq!(usage.completion_tokens, 12);
}

/// 部分字段缺失不能伪装成完整已知：否则「输入已知、输出未知」会被当成
/// 一次完整计费，输出侧永远是 0。
#[tokio::test]
async fn partially_reported_legacy_usage_is_not_complete() {
    let dir = tempfile::tempdir().unwrap();
    let path = build_legacy_database(&dir).await;
    let upgraded = SqliteKernel::open(&path).await.unwrap();

    let partial: (String, Option<i64>, Option<i64>) = sqlx::query_as(
        "SELECT usage_state, prompt_tokens, completion_tokens FROM llm_invocation WHERE id = ?",
    )
    .bind("inv-prompt-only")
    .fetch_one(upgraded.pool())
    .await
    .unwrap();
    assert_eq!(
        partial.0, "unknown",
        "只报告了输入 token 的行不能算完整已知"
    );
    // 已有的数值保留，不因为降级而清空——未知不等于丢弃。
    assert_eq!(partial.1, Some(7));
    assert_eq!(partial.2, None);
}

/// 0028 之后新写入的行必须保持原分类，0029 不能把它们一起降级。
#[tokio::test]
async fn newer_known_and_unbilled_rows_are_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let path = build_legacy_database(&dir).await;
    let upgraded = SqliteKernel::open(&path).await.unwrap();

    // 模拟 0028 之后的新代码：零 token 但明确是 known / unbilled 的行。
    for (id, prompt, completion, state) in [
        ("new-known-zero", 0_i64, 0_i64, "known"),
        ("new-unbilled", 0, 0, "unbilled"),
    ] {
        sqlx::query(
            "INSERT INTO llm_invocation(id, request_id, run_id, step_id, provider, model,
                input_summary, started_at, finished_at, prompt_tokens, completion_tokens,
                status, usage_state)
             VALUES (?, ?, 'new-run', 'draft', 'mock', 'mock', 'x', 1, 2, ?, ?, 'succeeded', ?)",
        )
        .bind(id)
        .bind(format!("req-{id}"))
        .bind(prompt)
        .bind(completion)
        .bind(state)
        .execute(upgraded.pool())
        .await
        .unwrap();
    }

    // 重开触发一次完整迁移链（0029 已在首次升级时执行，这里验证幂等）。
    upgraded.pool().close().await;
    let reopened = SqliteKernel::open(&path).await.unwrap();
    let usage = WorkflowRepository::run_usage(&reopened, "new-run")
        .await
        .unwrap();
    assert_eq!(usage.known_calls, 1, "真实的零用量仍是已知");
    assert_eq!(usage.unbilled_calls, 1);
    assert_eq!(usage.unknown_calls, 0);
}

/// 0029 只执行一次；重复启动不会重复改写。
#[tokio::test]
async fn observability_backfill_runs_once() {
    let dir = tempfile::tempdir().unwrap();
    let path = build_legacy_database(&dir).await;

    let upgraded = SqliteKernel::open(&path).await.unwrap();
    let applied: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM schema_migration WHERE version = '0029'")
            .fetch_one(upgraded.pool())
            .await
            .unwrap();
    assert_eq!(applied, 1, "0029 只应记录一次");
    upgraded.pool().close().await;

    let reopened = SqliteKernel::open(&path).await.unwrap();
    let applied: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM schema_migration WHERE version = '0029'")
            .fetch_one(reopened.pool())
            .await
            .unwrap();
    assert_eq!(applied, 1, "重复启动不应重复执行 0029");
    let usage = WorkflowRepository::run_usage(&reopened, "legacy-run")
        .await
        .unwrap();
    assert_eq!(usage.calls, 5);
    assert_eq!(usage.known_calls, 1);
    assert_eq!(usage.unknown_calls, 4);
}
