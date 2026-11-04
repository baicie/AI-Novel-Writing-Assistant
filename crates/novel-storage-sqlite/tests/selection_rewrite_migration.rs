#![cfg(feature = "test-harness")]
//! P17.2 RC1：选区改写计费记录表的迁移兼容性。
//!
//! 新表是纯增量（迁移 0025），必须验证：
//! - 从 0024 升级后旧业务数据不受影响；
//! - 重复打开数据库不会重复执行迁移，也不会丢记录。

use novel_application::{NewNovel, NovelRepository, SelectionRewriteAttemptState};
use novel_storage_sqlite::SqliteKernel;

// ---- 迁移兼容性：从 0024 升级到 0025 ----

#[tokio::test]
async fn upgrade_from_0024_creates_the_attempt_table_without_touching_existing_data() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("upgrade.sqlite");

    // 先停在 0024（还没有计费记录表），写入一份真实业务数据。
    {
        let old = SqliteKernel::open_at_migration(&db_path, 24).await.unwrap();
        NovelRepository::create_novel(
            &old,
            NewNovel {
                id: "novel-old".into(),
                title: "升级前的书".into(),
                description: String::new(),
                now_ms: 1,
            },
        )
        .await
        .unwrap();
        let exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'chapter_selection_rewrite_attempt'",
        )
        .fetch_one(old.pool())
        .await
        .unwrap();
        assert_eq!(exists, 0, "0024 上不应已经存在新表");
    }

    // 升级到最新：新表出现，旧数据仍在。
    let upgraded = SqliteKernel::open(&db_path).await.unwrap();
    let novel = NovelRepository::get_novel(&upgraded, "novel-old")
        .await
        .unwrap();
    assert_eq!(novel.title, "升级前的书", "升级不能影响已有业务数据");

    upgraded
        .begin_selection_rewrite("req-after-upgrade", "novel-old", "chapter-x", 100)
        .await
        .expect("升级后新表可以正常写入");
    let attempt = upgraded
        .selection_rewrite_attempt("req-after-upgrade")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(attempt.state, SelectionRewriteAttemptState::Sending);
}

#[tokio::test]
async fn reopening_an_upgraded_database_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("idempotent.sqlite");

    let first = SqliteKernel::open(&db_path).await.unwrap();
    first
        .begin_selection_rewrite("req-keep", "novel-1", "chapter-1", 10)
        .await
        .unwrap();
    drop(first);

    // 再次打开不能重复执行迁移，也不能清空已有记录。
    let second = SqliteKernel::open(&db_path).await.unwrap();
    let attempt = second
        .selection_rewrite_attempt("req-keep")
        .await
        .unwrap()
        .expect("重复打开后记录仍在");
    assert_eq!(attempt.request_id, "req-keep");
}
