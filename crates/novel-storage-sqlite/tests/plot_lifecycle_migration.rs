#![cfg(feature = "test-harness")]

use novel_application::{NewChapter, NewNovel, NovelRepository};
use novel_storage_sqlite::SqliteKernel;
use sqlx::Row;

#[tokio::test]
async fn upgrade_from_0025_preserves_plot_threads_and_adds_lifecycle() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("plot-upgrade.sqlite");
    {
        let kernel = SqliteKernel::open_at_migration(&path, 25).await.unwrap();
        NovelRepository::create_novel(
            &kernel,
            NewNovel {
                id: "n1".into(),
                title: "升级".into(),
                description: String::new(),
                now_ms: 1,
            },
        )
        .await
        .unwrap();
        NovelRepository::create_chapter(
            &kernel,
            NewChapter {
                id: "c1".into(),
                novel_id: "n1".into(),
                title: "第一章".into(),
                sort_order: 1,
                volume_id: None,
                now_ms: 1,
            },
        )
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO plot_thread
               (id, novel_id, title, body, opened_chapter_order, resolved_chapter_order, status,
                source_revision, evidence_json, created_at, updated_at, source_chapter_id)
             VALUES ('legacy-plot','n1','旧伏笔','旧伏笔正文',1,NULL,'open','memory:1','旧证据',1,1,'c1')",
        )
        .execute(kernel.pool())
        .await
        .unwrap();
    }

    let kernel = SqliteKernel::open(&path).await.unwrap();
    let row = sqlx::query(
        "SELECT title, body, status, version, last_development_chapter_order FROM plot_thread WHERE id = 'legacy-plot'",
    )
    .fetch_one(kernel.pool())
    .await
    .unwrap();
    assert_eq!(row.try_get::<String, _>("title").unwrap(), "旧伏笔");
    assert_eq!(row.try_get::<String, _>("body").unwrap(), "旧伏笔正文");
    assert_eq!(row.try_get::<String, _>("status").unwrap(), "open");
    assert_eq!(row.try_get::<i64, _>("version").unwrap(), 0);
    let last_dev: Option<i32> = row.try_get("last_development_chapter_order").unwrap();
    assert_eq!(last_dev, None);
    for table in ["plot_thread_event", "plot_causality_edge"] {
        let exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?",
        )
        .bind(table)
        .fetch_one(kernel.pool())
        .await
        .unwrap();
        assert_eq!(exists, 1, "{table} must exist after upgrade");
    }
    drop(kernel);

    // 再次打开不重复执行迁移，也不丢历史行。
    let kernel = SqliteKernel::open(&path).await.unwrap();
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM plot_thread WHERE id = 'legacy-plot'")
            .fetch_one(kernel.pool())
            .await
            .unwrap();
    assert_eq!(count, 1);
    let version: i64 =
        sqlx::query_scalar("SELECT version FROM plot_thread WHERE id = 'legacy-plot'")
            .fetch_one(kernel.pool())
            .await
            .unwrap();
    assert_eq!(version, 0);
}
