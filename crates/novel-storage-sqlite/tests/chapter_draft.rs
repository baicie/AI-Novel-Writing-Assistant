use novel_application::{
    NewChapter, NewNovel, NovelRepository, SaveChapterDraftRecord, SaveChapterRecord,
};
use novel_storage_sqlite::SqliteKernel;
use sqlx::Row;

#[tokio::test]
async fn local_drafts_are_isolated_and_cleared_only_for_matching_content() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = SqliteKernel::open(&dir.path().join("draft.sqlite"))
        .await
        .unwrap();
    NovelRepository::create_novel(
        &kernel,
        NewNovel {
            id: "novel-draft".into(),
            title: "草稿书".into(),
            description: String::new(),
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    NovelRepository::create_chapter(
        &kernel,
        NewChapter {
            id: "chapter-draft".into(),
            novel_id: "novel-draft".into(),
            title: "第一章".into(),
            sort_order: 1,
            volume_id: None,
            now_ms: 1,
        },
    )
    .await
    .unwrap();

    let first = NovelRepository::save_chapter_draft(
        &kernel,
        SaveChapterDraftRecord {
            novel_id: "novel-draft".into(),
            chapter_id: "chapter-draft".into(),
            client_id: "window-a".into(),
            content: "窗口 A 的未保存正文".into(),
            base_official_version: 0,
            base_updated_at: "t0".into(),
            expected_revision: 0,
            now_ms: 10,
        },
    )
    .await
    .unwrap();
    assert_eq!(first.local_revision, 1);
    assert_eq!(
        NovelRepository::get_chapter_draft(&kernel, "novel-draft", "chapter-draft", "window-a")
            .await
            .unwrap()
            .unwrap()
            .content,
        "窗口 A 的未保存正文"
    );

    NovelRepository::save_chapter_draft(
        &kernel,
        SaveChapterDraftRecord {
            novel_id: "novel-draft".into(),
            chapter_id: "chapter-draft".into(),
            client_id: "window-b".into(),
            content: "窗口 B 的未保存正文".into(),
            base_official_version: 0,
            base_updated_at: "t0".into(),
            expected_revision: 0,
            now_ms: 11,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        NovelRepository::get_chapter_draft(&kernel, "novel-draft", "chapter-draft", "window-a")
            .await
            .unwrap()
            .unwrap()
            .content,
        "窗口 A 的未保存正文"
    );
    assert_eq!(
        NovelRepository::get_chapter_draft(&kernel, "novel-draft", "chapter-draft", "window-b")
            .await
            .unwrap()
            .unwrap()
            .content,
        "窗口 B 的未保存正文"
    );

    // A stale/mismatched official save must not delete a newer draft.
    assert!(
        !NovelRepository::clear_chapter_draft(
            &kernel,
            "novel-draft",
            "chapter-draft",
            "window-a",
            "窗口 A 的未保存正文",
            0
        )
        .await
        .unwrap()
    );
    let second = NovelRepository::save_chapter_draft(
        &kernel,
        SaveChapterDraftRecord {
            novel_id: "novel-draft".into(),
            chapter_id: "chapter-draft".into(),
            client_id: "window-a".into(),
            content: "窗口 A 更新后的正文".into(),
            base_official_version: 0,
            base_updated_at: "t0".into(),
            expected_revision: 1,
            now_ms: 12,
        },
    )
    .await
    .unwrap();
    assert_eq!(second.local_revision, 2);
    assert!(
        !NovelRepository::clear_chapter_draft(
            &kernel,
            "novel-draft",
            "chapter-draft",
            "window-a",
            "窗口 A 更新后的正文",
            1
        )
        .await
        .unwrap()
    );
    assert!(
        NovelRepository::clear_chapter_draft(
            &kernel,
            "novel-draft",
            "chapter-draft",
            "window-a",
            "窗口 A 更新后的正文",
            2
        )
        .await
        .unwrap()
    );
    assert!(
        NovelRepository::get_chapter_draft(&kernel, "novel-draft", "chapter-draft", "window-a")
            .await
            .unwrap()
            .is_none()
    );

    // An official save (including an AI-promoted version) must not delete or overwrite
    // a local draft from another writer.
    let official = NovelRepository::save_user_chapter(
        &kernel,
        SaveChapterRecord {
            novel_id: "novel-draft".into(),
            chapter_id: "chapter-draft".into(),
            expected_version: 0,
            title: "第一章".into(),
            content: "官方保存的正文".into(),
            revision_id: "revision-official".into(),
            artifact_id: "artifact-official".into(),
            content_sha256: "official-hash".into(),
            now_ms: 20,
        },
    )
    .await
    .unwrap();
    assert_eq!(official.content, "官方保存的正文");
    let preserved =
        NovelRepository::get_chapter_draft(&kernel, "novel-draft", "chapter-draft", "window-b")
            .await
            .unwrap()
            .expect("window B local draft must survive an official save from window A");
    assert_eq!(preserved.content, "窗口 B 的未保存正文");

    let revisions: i64 = sqlx::query("SELECT COUNT(*) FROM chapter_revision WHERE chapter_id = ?")
        .bind("chapter-draft")
        .fetch_one(kernel.pool())
        .await
        .unwrap()
        .try_get(0)
        .unwrap();
    assert_eq!(
        revisions, 1,
        "only the explicit official save creates an official revision"
    );
}

#[tokio::test]
async fn concurrent_and_stale_draft_writes_cannot_overwrite_newer_text() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = SqliteKernel::open(&dir.path().join("race.sqlite"))
        .await
        .unwrap();
    NovelRepository::create_novel(
        &kernel,
        NewNovel {
            id: "n".into(),
            title: "书".into(),
            description: String::new(),
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    NovelRepository::create_chapter(
        &kernel,
        NewChapter {
            id: "c".into(),
            novel_id: "n".into(),
            title: "章".into(),
            sort_order: 1,
            volume_id: None,
            now_ms: 1,
        },
    )
    .await
    .unwrap();

    let make = |content: &str, now_ms: i64| SaveChapterDraftRecord {
        novel_id: "n".into(),
        chapter_id: "c".into(),
        client_id: "same-window".into(),
        content: content.into(),
        base_official_version: 0,
        base_updated_at: "t0".into(),
        expected_revision: 0,
        now_ms,
    };
    let (left, right) = tokio::join!(
        NovelRepository::save_chapter_draft(&kernel, make("并发 A", 10)),
        NovelRepository::save_chapter_draft(&kernel, make("并发 B", 11)),
    );
    assert_eq!(usize::from(left.is_ok()) + usize::from(right.is_ok()), 1);
    let first = NovelRepository::get_chapter_draft(&kernel, "n", "c", "same-window")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first.local_revision, 1);

    let second = NovelRepository::save_chapter_draft(
        &kernel,
        SaveChapterDraftRecord {
            novel_id: "n".into(),
            chapter_id: "c".into(),
            client_id: "same-window".into(),
            content: "较新的正文".into(),
            base_official_version: 0,
            base_updated_at: "t0".into(),
            expected_revision: first.local_revision,
            now_ms: 12,
        },
    )
    .await
    .unwrap();
    assert_eq!(second.local_revision, 2);

    let stale = NovelRepository::save_chapter_draft(
        &kernel,
        SaveChapterDraftRecord {
            novel_id: "n".into(),
            chapter_id: "c".into(),
            client_id: "same-window".into(),
            content: "迟到的旧正文".into(),
            base_official_version: 0,
            base_updated_at: "t0".into(),
            expected_revision: first.local_revision,
            now_ms: 13,
        },
    )
    .await;
    assert!(stale.is_err(), "stale local revision must be rejected");
    assert_eq!(
        NovelRepository::get_chapter_draft(&kernel, "n", "c", "same-window")
            .await
            .unwrap()
            .unwrap()
            .content,
        "较新的正文"
    );
}
