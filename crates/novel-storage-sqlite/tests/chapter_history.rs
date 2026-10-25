use novel_application::{
    NewChapter, NewNovel, NovelRepository, RestoreChapterRecord, SaveChapterRecord,
};
use novel_storage_sqlite::SqliteKernel;

async fn kernel() -> (tempfile::TempDir, SqliteKernel) {
    let dir = tempfile::tempdir().unwrap();
    let kernel = SqliteKernel::open(&dir.path().join("history.sqlite"))
        .await
        .unwrap();
    (dir, kernel)
}

async fn save(
    kernel: &SqliteKernel,
    revision: &str,
    expected: i64,
    content: &str,
) -> novel_application::ChapterSnapshot {
    NovelRepository::save_user_chapter(
        kernel,
        SaveChapterRecord {
            novel_id: "n1".into(),
            chapter_id: "ch1".into(),
            expected_version: expected,
            title: "第一章".into(),
            content: content.into(),
            revision_id: revision.into(),
            artifact_id: format!("artifact-{revision}"),
            content_sha256: format!("hash-{revision}"),
            now_ms: 100 + expected,
        },
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn restore_creates_new_revision_and_rejects_stale_expected_version() {
    let (_dir, kernel) = kernel().await;
    NovelRepository::create_novel(
        &kernel,
        NewNovel {
            id: "n1".into(),
            title: "历史".into(),
            description: String::new(),
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    NovelRepository::create_chapter(
        &kernel,
        NewChapter {
            id: "ch1".into(),
            novel_id: "n1".into(),
            title: "第一章".into(),
            sort_order: 1,
            volume_id: None,
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    save(&kernel, "r1", 0, "第一版正文").await;
    save(&kernel, "r2", 1, "第二版正文").await;
    let revisions = NovelRepository::list_chapter_revisions(&kernel, "n1", "ch1")
        .await
        .unwrap();
    assert_eq!(revisions.len(), 2);
    let v1 = revisions
        .iter()
        .find(|revision| revision.version == 1)
        .unwrap();

    let restored = NovelRepository::restore_chapter_revision(
        &kernel,
        RestoreChapterRecord {
            novel_id: "n1".into(),
            chapter_id: "ch1".into(),
            source_revision_id: v1.id.clone(),
            expected_version: 2,
            revision_id: "r3".into(),
            artifact_id: "artifact-r3".into(),
            now_ms: 300,
        },
    )
    .await
    .unwrap();
    assert_eq!(restored.official_version, 3);
    assert_eq!(restored.content, "第一版正文");
    let revisions = NovelRepository::list_chapter_revisions(&kernel, "n1", "ch1")
        .await
        .unwrap();
    assert_eq!(revisions.len(), 3);
    assert_eq!(revisions[0].origin, "user");
    assert_eq!(revisions[0].content, "第一版正文");
    let restored_from: String = sqlx::query_scalar(
        "SELECT json_extract(payload_json, '$.restoredFromRevisionId') FROM workflow_artifact WHERE type = 'chapter_restore'",
    ).fetch_one(kernel.pool()).await.unwrap();
    assert_eq!(restored_from, v1.id);

    let stale = NovelRepository::restore_chapter_revision(
        &kernel,
        RestoreChapterRecord {
            novel_id: "n1".into(),
            chapter_id: "ch1".into(),
            source_revision_id: v1.id.clone(),
            expected_version: 2,
            revision_id: "r4".into(),
            artifact_id: "artifact-r4".into(),
            now_ms: 400,
        },
    )
    .await;
    assert!(stale.is_err(), "stale official version must reject restore");
    let missing = NovelRepository::restore_chapter_revision(
        &kernel,
        RestoreChapterRecord {
            novel_id: "n1".into(),
            chapter_id: "ch1".into(),
            source_revision_id: "not-a-revision".into(),
            expected_version: 3,
            revision_id: "r5".into(),
            artifact_id: "artifact-r5".into(),
            now_ms: 500,
        },
    )
    .await;
    assert!(
        missing.is_err(),
        "foreign/missing revision must be rejected"
    );
}
