use std::fs;
use std::path::Path;

use novel_application::{NovelRepository, WorkflowRepository};
use novel_storage_sqlite::{SqliteKernel, import_legacy};
use sqlx::Row;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

async fn connect(path: &Path, create: bool) -> sqlx::SqlitePool {
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(create),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn chapter_round_trip_keeps_one_hundred_thousand_characters() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = SqliteKernel::open(&dir.path().join("novel.sqlite"))
        .await
        .unwrap();
    let novel = NovelRepository::create_novel(
        &kernel,
        novel_application::NewNovel {
            id: "n".into(),
            title: "长章".into(),
            description: String::new(),
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    let chapter = NovelRepository::create_chapter(
        &kernel,
        novel_application::NewChapter {
            id: "c".into(),
            novel_id: novel.id.clone(),
            title: "第一章".into(),
            sort_order: 1,
            volume_id: None,
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    let content = "字".repeat(100_000);
    let saved = NovelRepository::save_user_chapter(
        &kernel,
        novel_application::SaveChapterRecord {
            novel_id: novel.id.clone(),
            chapter_id: chapter.id.clone(),
            expected_version: 0,
            title: "第一章".into(),
            content: content.clone(),
            revision_id: "rev".into(),
            artifact_id: "art".into(),
            content_sha256: "hash".into(),
            now_ms: 2,
        },
    )
    .await
    .unwrap();
    assert_eq!(saved.content.chars().count(), 100_000);
    assert_eq!(saved.content, content);
    let conflict = NovelRepository::save_user_chapter(
        &kernel,
        novel_application::SaveChapterRecord {
            novel_id: novel.id,
            chapter_id: chapter.id,
            expected_version: 0,
            title: "第一章".into(),
            content: "新的".into(),
            revision_id: "rev-2".into(),
            artifact_id: "art-2".into(),
            content_sha256: "hash-2".into(),
            now_ms: 3,
        },
    )
    .await;
    assert!(conflict.is_err());
}

#[tokio::test]
async fn legacy_import_pauses_running_work_and_leaves_the_source_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("legacy.sqlite");
    let target = dir.path().join("next.sqlite");
    let pool = connect(&source, true).await;
    sqlx::query("CREATE TABLE Novel (id TEXT PRIMARY KEY, title TEXT, description TEXT, status TEXT, secretNote TEXT)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO Novel (id, title, description, status, secretNote) VALUES ('n1','旧书','简介','draft','不要丢')")
        .execute(&pool).await.unwrap();
    sqlx::query("CREATE TABLE VolumePlan (id TEXT PRIMARY KEY, novelId TEXT, sortOrder INTEGER, title TEXT, summary TEXT)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO VolumePlan (id, novelId, sortOrder, title, summary) VALUES ('v1','n1',1,'第一卷','旧卷摘要')")
        .execute(&pool).await.unwrap();
    sqlx::query("CREATE TABLE VolumeChapterPlan (id TEXT PRIMARY KEY, volumeId TEXT, chapterId TEXT, chapterOrder INTEGER)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO VolumeChapterPlan (id, volumeId, chapterId, chapterOrder) VALUES ('vp1','v1','c1',1)")
        .execute(&pool).await.unwrap();
    sqlx::query("CREATE TABLE Chapter (id TEXT PRIMARY KEY, novelId TEXT, title TEXT, content TEXT, \"order\" INTEGER)")
        .execute(&pool).await.unwrap();
    let legacy_content = "旧正文长段落。".repeat(80_000);
    sqlx::query("INSERT INTO Chapter (id, novelId, title, content, \"order\") VALUES ('c1','n1','第一章',?1,1)")
        .bind(&legacy_content)
        .execute(&pool).await.unwrap();
    sqlx::query("CREATE TABLE DirectorNextRun (id TEXT PRIMARY KEY, novelId TEXT, planVersion TEXT, contractJson TEXT)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO DirectorNextRun (id, novelId, planVersion, contractJson) VALUES ('r1','n1','old-plan','{}')")
        .execute(&pool).await.unwrap();
    sqlx::query("CREATE TABLE DirectorNextRunControl (runId TEXT PRIMARY KEY, novelId TEXT, version INTEGER, status TEXT, pauseJson TEXT, failureReason TEXT)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO DirectorNextRunControl (runId, novelId, version, status, pauseJson, failureReason) VALUES ('r1','n1',4,'running',NULL,NULL)")
        .execute(&pool).await.unwrap();
    sqlx::query("CREATE TABLE ImageJob (id TEXT)")
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
    let before = fs::read(&source).unwrap();
    let report = import_legacy(&source, &target, 10).await.unwrap();
    let after = fs::read(&source).unwrap();
    assert_eq!(before, after);
    assert!(
        report
            .unsupported
            .iter()
            .any(|item| item.column.as_deref() == Some("secretNote"))
    );
    assert!(
        report
            .unsupported
            .iter()
            .any(|item| item.table == "ImageJob")
    );
    assert_eq!(report.runs_paused_for_review, 1);
    assert_eq!(report.volumes, 1);
    assert!(report.content_verified);
    assert!(report.relationships_verified);
    assert!(Path::new(&report.source_backup).is_file());
    assert!(fs::metadata(&report.source_backup).unwrap().len() > 0);
    let kernel = SqliteKernel::open(&target).await.unwrap();
    let chapter = NovelRepository::get_chapter(&kernel, "n1", "c1")
        .await
        .unwrap();
    assert_eq!(chapter.content.len(), legacy_content.len());
    assert_eq!(chapter.content, legacy_content);
    assert_eq!(chapter.protection, "user_edited");
    let volume_id: Option<String> =
        sqlx::query_scalar("SELECT volume_id FROM chapter WHERE id = 'c1'")
            .fetch_one(kernel.pool())
            .await
            .unwrap();
    assert_eq!(volume_id.as_deref(), Some("v1"));
    let status: String = sqlx::query("SELECT status FROM workflow_control WHERE run_id = 'r1'")
        .fetch_one(kernel.pool())
        .await
        .unwrap()
        .try_get("status")
        .unwrap();
    assert_eq!(status, "paused");
    let again = import_legacy(&source, &target, 11).await.unwrap();
    assert_eq!(again.novels, 1);
    assert_eq!(again.chapters, 1);
    assert!(
        again
            .target_backup
            .as_ref()
            .is_some_and(|path| Path::new(path).is_file())
    );
    assert!(again.content_verified);
    let _ = WorkflowRepository::remote_call_count(&kernel, "r1", "draft")
        .await
        .unwrap();
}

#[tokio::test]
async fn list_chapters_returns_them_in_sort_order_with_the_official_content() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = SqliteKernel::open(&dir.path().join("novel.sqlite"))
        .await
        .unwrap();
    let novel = NovelRepository::create_novel(
        &kernel,
        novel_application::NewNovel {
            id: "n".into(),
            title: "次序".into(),
            description: String::new(),
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    let _second = NovelRepository::create_chapter(
        &kernel,
        novel_application::NewChapter {
            id: "c-2".into(),
            novel_id: novel.id.clone(),
            title: "第二章".into(),
            sort_order: 2,
            volume_id: None,
            now_ms: 2,
        },
    )
    .await
    .unwrap();
    let first = NovelRepository::create_chapter(
        &kernel,
        novel_application::NewChapter {
            id: "c-1".into(),
            novel_id: novel.id.clone(),
            title: "第一章".into(),
            sort_order: 1,
            volume_id: None,
            now_ms: 3,
        },
    )
    .await
    .unwrap();
    NovelRepository::save_user_chapter(
        &kernel,
        novel_application::SaveChapterRecord {
            novel_id: novel.id.clone(),
            chapter_id: first.id.clone(),
            expected_version: 0,
            title: "第一章".into(),
            content: "初始正文".into(),
            revision_id: "rev-1".into(),
            artifact_id: "art-1".into(),
            content_sha256: "hash-1".into(),
            now_ms: 4,
        },
    )
    .await
    .unwrap();
    let chapters = NovelRepository::list_chapters(&kernel, &novel.id)
        .await
        .unwrap();
    assert_eq!(chapters.len(), 2);
    assert_eq!(chapters[0].id, first.id);
    assert_eq!(chapters[0].sort_order, 1);
    assert_eq!(chapters[0].content, "初始正文");
    assert_eq!(chapters[0].official_version, 1);
    assert_eq!(chapters[1].id, "c-2");
    assert_eq!(chapters[1].content, "");
    assert_eq!(chapters[1].official_version, 0);
}

#[tokio::test]
async fn model_setup_round_trip_keeps_endpoint_model_and_local_flag() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = SqliteKernel::open(&dir.path().join("novel.sqlite"))
        .await
        .unwrap();
    kernel
        .save_model_setup(
            "https://example.com/v1/chat/completions",
            "gpt-test",
            false,
            1,
        )
        .await
        .unwrap();
    let loaded = kernel.load_model_setup().await.unwrap().unwrap();
    assert_eq!(loaded.endpoint, "https://example.com/v1/chat/completions");
    assert_eq!(loaded.model, "gpt-test");
    assert!(!loaded.allow_localhost_http);
    assert_eq!(loaded.secret_ref, novel_storage_sqlite::MODEL_SECRET_REF);
    kernel
        .save_model_setup(
            "http://127.0.0.1:8080/v1/chat/completions",
            "gpt-test",
            true,
            2,
        )
        .await
        .unwrap();
    let updated = kernel.load_model_setup().await.unwrap().unwrap();
    assert_eq!(
        updated.endpoint,
        "http://127.0.0.1:8080/v1/chat/completions"
    );
    assert!(updated.allow_localhost_http);
}
