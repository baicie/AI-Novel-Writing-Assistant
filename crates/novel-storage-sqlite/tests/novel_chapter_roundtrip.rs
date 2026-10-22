use std::path::PathBuf;

use novel_application::{
    DeleteChapterRecord, NewChapter, NewNovel, NewVolume, NovelRepository, PromoteOutcome,
    SaveChapterRecord, UpdateChapterMetaRecord, UpdateVolumeRecord,
};
use novel_storage_sqlite::SqliteKernel;
use tempfile::TempDir;

fn db_path(dir: &TempDir) -> PathBuf {
    dir.path().join("novel.sqlite")
}

fn chapter_record(
    novel_id: &str,
    chapter_id: &str,
    expected_version: i64,
    content: &str,
) -> SaveChapterRecord {
    SaveChapterRecord {
        novel_id: novel_id.into(),
        chapter_id: chapter_id.into(),
        expected_version,
        title: "第一章 长夜将明".into(),
        content: content.into(),
        revision_id: format!("rev-{expected_version}-{}", content.len()),
        artifact_id: format!("artifact-{expected_version}-{}", content.len()),
        content_sha256: novel_application::sha256_hex(content.as_bytes()),
        now_ms: 1_760_000_000_000 + expected_version,
    }
}

#[tokio::test]
async fn long_chapter_roundtrip_survives_reopen_and_preserves_user_edit_guard() {
    let dir = TempDir::new().unwrap();
    let path = db_path(&dir);

    let novel = {
        let kernel = SqliteKernel::open(&path).await.unwrap();
        let novel = kernel
            .create_novel(NewNovel {
                id: "novel-1".into(),
                title: "迁移验证小说".into(),
                description: "用于验证 SQLite 长正文和重启恢复。".into(),
                now_ms: 1_760_000_000_000,
            })
            .await
            .unwrap();
        let volume = kernel
            .create_volume(NewVolume {
                id: "volume-1".into(),
                novel_id: novel.id.clone(),
                title: "第一卷".into(),
                sort_order: 1,
                summary: "开篇".into(),
            })
            .await
            .unwrap();
        assert_eq!(volume.title, "第一卷");
        let chapter = kernel
            .create_chapter(NewChapter {
                id: "chapter-1".into(),
                novel_id: novel.id.clone(),
                title: "第一章".into(),
                sort_order: 1,
                volume_id: Some("volume-1".into()),
                now_ms: 1_760_000_000_001,
            })
            .await
            .unwrap();
        assert_eq!(chapter.official_version, 0);

        let long_content = "长夜将明，风雪压城。".repeat(100_000);
        assert!(long_content.len() > 1_000_000);
        let saved = kernel
            .save_user_chapter(chapter_record(&novel.id, &chapter.id, 0, &long_content))
            .await
            .unwrap();
        assert_eq!(saved.official_version, 1);
        assert_eq!(saved.protection, "user_edited");
        assert_eq!(saved.content.len(), long_content.len());

        let conflict = kernel
            .save_user_chapter(chapter_record(&novel.id, &chapter.id, 0, "旧版本覆盖"))
            .await;
        assert!(matches!(
            conflict,
            Err(novel_application::PortError::Conflict(_))
        ));

        let revisions = kernel
            .list_chapter_revisions(&novel.id, &chapter.id)
            .await
            .unwrap();
        assert_eq!(revisions.len(), 1);
        assert_eq!(revisions[0].origin, "user");
        assert_eq!(revisions[0].content.len(), long_content.len());

        let blocked = kernel
            .promote_candidate(&chapter.id, "candidate-ai", "rev-ai", 1_760_000_000_999)
            .await
            .unwrap();
        assert_eq!(blocked, PromoteOutcome::Blocked);

        let renamed = kernel
            .update_chapter_meta(UpdateChapterMetaRecord {
                novel_id: novel.id.clone(),
                chapter_id: chapter.id.clone(),
                expected_version: 1,
                title: "第一章：雪夜".into(),
                sort_order: 2,
                volume_id: Some("volume-1".into()),
            })
            .await
            .unwrap();
        assert_eq!(renamed.title, "第一章：雪夜");
        assert_eq!(renamed.sort_order, 2);
        assert_eq!(renamed.official_version, 1);

        let volume_busy = kernel.delete_volume(&novel.id, &volume.id).await;
        assert!(matches!(
            volume_busy,
            Err(novel_application::PortError::Conflict(_))
        ));

        let updated_volume = kernel
            .update_volume(UpdateVolumeRecord {
                novel_id: novel.id.clone(),
                volume_id: volume.id.clone(),
                title: "第一卷：风雪".into(),
                sort_order: 1,
                summary: "开篇修订".into(),
            })
            .await
            .unwrap();
        assert_eq!(updated_volume.title, "第一卷：风雪");
        novel
    };

    let reopened = SqliteKernel::open(&path).await.unwrap();
    let chapters = reopened.list_chapters(&novel.id).await.unwrap();
    assert_eq!(chapters.len(), 1);
    assert_eq!(chapters[0].official_version, 1);
    assert_eq!(
        chapters[0].content.len(),
        "长夜将明，风雪压城。".len() * 100_000
    );
    assert_eq!(chapters[0].protection, "user_edited");
    assert_eq!(chapters[0].title, "第一章：雪夜");
    assert_eq!(chapters[0].sort_order, 2);

    let volumes = reopened.list_volumes(&novel.id).await.unwrap();
    assert_eq!(volumes.len(), 1);
    assert_eq!(volumes[0].title, "第一卷：风雪");
    assert!(matches!(
        reopened.delete_novel(&novel.id).await,
        Err(novel_application::PortError::Conflict(_))
    ));
}

#[tokio::test]
async fn chapter_delete_requires_the_current_version_and_removes_revisions() {
    let dir = TempDir::new().unwrap();
    let path = db_path(&dir);
    let kernel = SqliteKernel::open(&path).await.unwrap();
    let novel = kernel
        .create_novel(NewNovel {
            id: "n-delete".into(),
            title: "删除验证".into(),
            description: String::new(),
            now_ms: 1_760_000_000_100,
        })
        .await
        .unwrap();
    let chapter = kernel
        .create_chapter(NewChapter {
            id: "c-delete".into(),
            novel_id: novel.id.clone(),
            title: "待删除".into(),
            sort_order: 1,
            volume_id: None,
            now_ms: 1_760_000_000_101,
        })
        .await
        .unwrap();
    kernel
        .save_user_chapter(chapter_record(&novel.id, &chapter.id, 0, "正文"))
        .await
        .unwrap();
    let stale = kernel
        .delete_chapter(DeleteChapterRecord {
            novel_id: novel.id.clone(),
            chapter_id: chapter.id.clone(),
            expected_version: 0,
        })
        .await;
    assert!(matches!(
        stale,
        Err(novel_application::PortError::Conflict(_))
    ));
    kernel
        .delete_chapter(DeleteChapterRecord {
            novel_id: novel.id.clone(),
            chapter_id: chapter.id.clone(),
            expected_version: 1,
        })
        .await
        .unwrap();
    assert!(kernel.list_chapters(&novel.id).await.unwrap().is_empty());
    assert!(
        kernel
            .list_chapter_revisions(&novel.id, &chapter.id)
            .await
            .is_err()
    );
}
