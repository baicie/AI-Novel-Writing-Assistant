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
    let index: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'plot_thread_event_plot'",
    )
    .fetch_one(kernel.pool())
    .await
    .unwrap();
    assert_eq!(index, 1, "As-Of 折叠需要按 plot_id 的覆盖索引");
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

#[tokio::test]
async fn upgrade_from_0026_adds_lookup_index_and_preserves_events() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("plot-0026-upgrade.sqlite");
    {
        let kernel = SqliteKernel::open_at_migration(&path, 26).await.unwrap();
        NovelRepository::create_novel(
            &kernel,
            NewNovel {
                id: "n1".into(),
                title: "0026 升级".into(),
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
            "INSERT INTO chapter_revision
               (id, chapter_id, version, origin, content, content_sha256, created_at)
             VALUES ('rev-c1', 'c1', 1, 'user', '第一章埋下玉佩。', 'hash', 1)",
        )
        .execute(kernel.pool())
        .await
        .unwrap();
        sqlx::query("UPDATE chapter SET official_revision_id = 'rev-c1' WHERE id = 'c1'")
            .execute(kernel.pool())
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO plot_thread
               (id, novel_id, title, body, opened_chapter_order, resolved_chapter_order, status,
                source_revision, evidence_json, created_at, updated_at, source_chapter_id, version)
             VALUES ('legacy-plot','n1','旧伏笔','旧伏笔正文',1,NULL,'open','1','第一章埋下玉佩。',
                     1,1,'c1',1)",
        )
        .execute(kernel.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO plot_thread_event
               (id, plot_id, novel_id, kind, chapter_id, chapter_order, source_revision, evidence,
                from_status, to_status, command_id, request_hash, created_at)
             VALUES ('setup-legacy-plot','legacy-plot','n1','setup','c1',1,1,'第一章埋下玉佩。',
                     NULL,'open','legacy-create','legacy-hash',1)",
        )
        .execute(kernel.pool())
        .await
        .unwrap();
    }

    let kernel = SqliteKernel::open(&path).await.unwrap();
    let index: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'plot_thread_event_plot'",
    )
    .fetch_one(kernel.pool())
    .await
    .unwrap();
    assert_eq!(index, 1);
    let events: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM plot_thread_event")
        .fetch_one(kernel.pool())
        .await
        .unwrap();
    assert_eq!(events, 1, "升级不能丢事件历史");
    let threads = novel_application::PlotRepository::list_plot_threads(&kernel, "n1", 5)
        .await
        .unwrap();
    assert_eq!(threads.len(), 1);
    assert_eq!(threads[0].status, "open");

    let plan: Vec<String> = sqlx::query(
        "EXPLAIN QUERY PLAN
         SELECT e2.id FROM plot_thread_event e2
         LEFT JOIN chapter c2 ON c2.id = e2.chapter_id
         WHERE e2.plot_id = 'legacy-plot'
           AND COALESCE(c2.sort_order, e2.chapter_order) <= 5
         ORDER BY COALESCE(c2.sort_order, e2.chapter_order) DESC,
                  e2.created_at DESC, e2.rowid DESC
         LIMIT 1",
    )
    .fetch_all(kernel.pool())
    .await
    .unwrap()
    .iter()
    .map(|row| sqlx::Row::try_get::<String, _>(row, "detail").unwrap())
    .collect();
    assert!(
        plan.iter()
            .any(|detail| detail.contains("plot_thread_event_plot")),
        "As-Of 折叠应使用 plot_id 覆盖索引，实际计划：{plan:?}"
    );

    drop(kernel);
    // 数据库重启后索引与查询行为保持一致，且 0027 只执行一次。
    let kernel = SqliteKernel::open(&path).await.unwrap();
    let applied: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM schema_migration WHERE version = '0027'")
            .fetch_one(kernel.pool())
            .await
            .unwrap();
    assert_eq!(applied, 1);
    let threads = novel_application::PlotRepository::list_plot_threads(&kernel, "n1", 5)
        .await
        .unwrap();
    assert_eq!(threads[0].status, "open");
}
