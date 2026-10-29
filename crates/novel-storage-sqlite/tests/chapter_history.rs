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
            command_id: "restore-1".into(),
            operator: "tester".into(),
            reason: "test".into(),
            draft_decision: "reject".into(),
            draft_client_id: None,
            expected_draft_revision: None,
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
            command_id: "restore-2".into(),
            operator: "tester".into(),
            reason: "test".into(),
            draft_decision: "reject".into(),
            draft_client_id: None,
            expected_draft_revision: None,
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
            command_id: "restore-3".into(),
            operator: "tester".into(),
            reason: "test".into(),
            draft_decision: "reject".into(),
            draft_client_id: None,
            expected_draft_revision: None,
            now_ms: 500,
        },
    )
    .await;
    assert!(
        missing.is_err(),
        "foreign/missing revision must be rejected"
    );
}

#[tokio::test]
async fn restore_protects_local_drafts_and_records_provenance() {
    let (_dir, kernel) = kernel().await;
    NovelRepository::create_novel(
        &kernel,
        NewNovel {
            id: "n1".into(),
            title: "草稿保护".into(),
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
    save(&kernel, "r1", 0, "历史正文").await;
    NovelRepository::save_chapter_draft(
        &kernel,
        novel_application::SaveChapterDraftRecord {
            novel_id: "n1".into(),
            chapter_id: "ch1".into(),
            client_id: "writer-a".into(),
            content: "未保存草稿".into(),
            base_official_version: 1,
            base_updated_at: "1".into(),
            expected_revision: 0,
            now_ms: 10,
        },
    )
    .await
    .unwrap();
    let base = RestoreChapterRecord {
        novel_id: "n1".into(),
        chapter_id: "ch1".into(),
        source_revision_id: "r1".into(),
        expected_version: 1,
        revision_id: "r2".into(),
        artifact_id: "artifact-r2".into(),
        command_id: "restore-draft".into(),
        operator: "tester".into(),
        reason: "选择历史".into(),
        draft_decision: "reject".into(),
        draft_client_id: None,
        expected_draft_revision: None,
        now_ms: 20,
    };
    assert!(
        NovelRepository::restore_chapter_revision(&kernel, base.clone())
            .await
            .is_err()
    );
    let mut discard = base.clone();
    discard.draft_decision = "discard_own".into();
    discard.draft_client_id = Some("writer-a".into());
    discard.expected_draft_revision = Some(1);
    let restored = NovelRepository::restore_chapter_revision(&kernel, discard)
        .await
        .unwrap();
    assert_eq!(restored.content, "历史正文");
    let drafts = NovelRepository::list_chapter_drafts(&kernel, "n1", "ch1")
        .await
        .unwrap();
    assert!(drafts.is_empty());
    let revisions = NovelRepository::list_chapter_revisions(&kernel, "n1", "ch1")
        .await
        .unwrap();
    assert_eq!(revisions[0].origin_detail, "restored_from_history");
    assert_eq!(
        revisions[0].restored_from_revision_id.as_deref(),
        Some("r1")
    );
    assert_eq!(revisions[0].restored_by.as_deref(), Some("tester"));
    // Same command ID is idempotent and does not append another revision.
    let mut replay = base;
    replay.draft_decision = "keep".into();
    replay.command_id = "restore-draft".into();
    let replayed = NovelRepository::restore_chapter_revision(&kernel, replay)
        .await
        .unwrap();
    assert_eq!(replayed.official_version, 2);
    assert_eq!(
        NovelRepository::list_chapter_revisions(&kernel, "n1", "ch1")
            .await
            .unwrap()
            .len(),
        2
    );
}
