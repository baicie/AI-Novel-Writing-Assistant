use novel_application::{NewChapter, NewNovel, NovelRepository};
use novel_storage_sqlite::SqliteKernel;

#[tokio::test]
async fn vacuum_into_backup_is_consistent_and_restorable() {
    let dir = tempfile::tempdir().unwrap();
    let main = dir.path().join("main.sqlite");
    let backup = dir.path().join("backup.sqlite");
    let kernel = SqliteKernel::open(&main).await.unwrap();
    NovelRepository::create_novel(
        &kernel,
        NewNovel {
            id: "n1".into(),
            title: "备份".into(),
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
    sqlx::query("VACUUM INTO ?")
        .bind(backup.to_string_lossy().to_string())
        .execute(kernel.pool())
        .await
        .unwrap();
    assert!(backup.is_file());
    kernel.pool().close().await;

    let backup_kernel = SqliteKernel::open(&backup).await.unwrap();
    let chapters = NovelRepository::list_chapters(&backup_kernel, "n1")
        .await
        .unwrap();
    assert_eq!(chapters.len(), 1);
    assert_eq!(chapters[0].title, "第一章");
    backup_kernel.pool().close().await;

    // Restore into a fresh working copy, leaving the backup intact.
    let restored = dir.path().join("restored.sqlite");
    std::fs::copy(&backup, &restored).unwrap();
    let restored_kernel = SqliteKernel::open(&restored).await.unwrap();
    let novels = NovelRepository::list_novels(&restored_kernel)
        .await
        .unwrap();
    assert_eq!(novels.len(), 1);
    assert_eq!(novels[0].title, "备份");
    // Opening the same database again must be migration-idempotent.
    drop(restored_kernel);
    let reopened = SqliteKernel::open(&restored).await.unwrap();
    assert_eq!(
        NovelRepository::list_novels(&reopened).await.unwrap().len(),
        1
    );
}
