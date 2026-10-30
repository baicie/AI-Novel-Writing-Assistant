#![cfg(feature = "test-harness")]
use novel_storage_sqlite::SqliteKernel;

#[tokio::test]
async fn database_created_before_structured_facts_upgrades_without_data_loss() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("historical.sqlite");
    let old = SqliteKernel::open_at_migration(&path, 19).await.unwrap();
    sqlx::query("INSERT INTO novel(id,title,description,status,created_at,updated_at) VALUES ('n1','旧书','旧描述','draft',1,1)")
        .execute(old.pool()).await.unwrap();
    sqlx::query("INSERT INTO chapter(id,novel_id,title,sort_order,official_revision_id,protection,created_at,updated_at) VALUES ('c1','n1','第一章',1,'r1','user_edited',1,1)")
        .execute(old.pool()).await.unwrap();
    sqlx::query("INSERT INTO chapter_revision(id,chapter_id,version,origin,content,content_sha256,created_at) VALUES ('r1','c1',1,'user','旧版正式正文','hash-old',1)")
        .execute(old.pool()).await.unwrap();
    sqlx::query("INSERT INTO chapter_local_draft(novel_id,chapter_id,client_id,content,base_official_version,base_updated_at,local_revision,updated_at) VALUES ('n1','c1','writer-a','旧窗口草稿',1,'t1',1,2)")
        .execute(old.pool()).await.unwrap();
    old.pool().close().await;

    let original = std::fs::read(&path).unwrap();
    let upgraded = SqliteKernel::open(&path).await.unwrap();
    let versions: Vec<String> =
        sqlx::query_scalar("SELECT version FROM schema_migration ORDER BY version")
            .fetch_all(upgraded.pool())
            .await
            .unwrap();
    assert!(versions.contains(&"0020".to_string()));
    assert!(versions.contains(&"0021".to_string()));
    let integrity: String = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(upgraded.pool())
        .await
        .unwrap();
    assert_eq!(integrity, "ok");
    let fk: Vec<String> = sqlx::query_scalar("PRAGMA foreign_key_check")
        .fetch_all(upgraded.pool())
        .await
        .unwrap();
    assert!(fk.is_empty());
    let content: String = sqlx::query_scalar("SELECT content FROM chapter_revision WHERE id='r1'")
        .fetch_one(upgraded.pool())
        .await
        .unwrap();
    assert_eq!(content, "旧版正式正文");
    let draft: String =
        sqlx::query_scalar("SELECT content FROM chapter_local_draft WHERE client_id='writer-a'")
            .fetch_one(upgraded.pool())
            .await
            .unwrap();
    assert_eq!(draft, "旧窗口草稿");
    let novel_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM novel")
        .fetch_one(upgraded.pool())
        .await
        .unwrap();
    assert_eq!(novel_count, 1);
    upgraded.pool().close().await;
    // Reopening the upgraded database must be idempotent.
    let reopened = SqliteKernel::open(&path).await.unwrap();
    let version_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM schema_migration")
        .fetch_one(reopened.pool())
        .await
        .unwrap();
    assert_eq!(
        version_count as usize,
        novel_storage_sqlite::MIGRATION_COUNT
    );
    drop(reopened);
    assert!(!original.is_empty());
}
