use novel_application::{KnowledgeRepository, NewDocument, NewNovel, NovelRepository};
use novel_storage_sqlite::SqliteKernel;

async fn kernel() -> (tempfile::TempDir, SqliteKernel) {
    let dir = tempfile::tempdir().unwrap();
    let kernel = SqliteKernel::open(&dir.path().join("source.sqlite"))
        .await
        .unwrap();
    for (id, title) in [("n1", "书一"), ("n2", "书二")] {
        NovelRepository::create_novel(
            &kernel,
            NewNovel {
                id: id.into(),
                title: title.into(),
                description: String::new(),
                now_ms: 1,
            },
        )
        .await
        .unwrap();
    }
    (dir, kernel)
}

fn doc(id: &str, novel: &str, owner: &str, revision: &str) -> NewDocument {
    let body = "林动在祖祠发现封印。".to_string();
    NewDocument {
        id: id.into(),
        novel_id: novel.into(),
        owner_type: owner.into(),
        title: id.into(),
        body: body.clone(),
        terms: novel_context::fts_query(&novel_context::tokenize(
            &body,
            &novel_context::Lexicon::default(),
        )),
        created_at: 1,
        source_revision: Some(revision.into()),
        source_content_hash: Some(format!("hash-{revision}")),
        embedding: None,
    }
}

async fn revision(kernel: &SqliteKernel, id: &str) -> String {
    sqlx::query_scalar("SELECT source_revision FROM knowledge_document WHERE id = ?")
        .bind(id)
        .fetch_one(kernel.pool())
        .await
        .unwrap()
}

#[tokio::test]
async fn old_revision_cannot_overwrite_newer_and_cross_novel_is_rejected() {
    let (_dir, kernel) = kernel().await;
    KnowledgeRepository::index_document(&kernel, doc("d1", "n1", "document", "rev@v2"))
        .await
        .unwrap();
    assert_eq!(revision(&kernel, "d1").await, "rev@v2");
    let stale =
        KnowledgeRepository::index_document(&kernel, doc("d1", "n1", "document", "rev@v1")).await;
    assert!(
        stale.is_err(),
        "older source revision must not overwrite newer index"
    );
    assert_eq!(revision(&kernel, "d1").await, "rev@v2");
    let newer = doc("d1", "n1", "document", "rev@v3");
    KnowledgeRepository::index_document(&kernel, newer)
        .await
        .unwrap();
    assert_eq!(revision(&kernel, "d1").await, "rev@v3");
    let cross =
        KnowledgeRepository::index_document(&kernel, doc("d1", "n2", "document", "rev@v4")).await;
    assert!(
        cross.is_err(),
        "same document id must not move across novels"
    );
}

#[tokio::test]
async fn generic_delete_cannot_remove_system_chapter_index() {
    let (_dir, kernel) = kernel().await;
    KnowledgeRepository::index_document(&kernel, doc("chapter:ch1", "n1", "chapter", "rev@v1"))
        .await
        .unwrap();
    let deleted = KnowledgeRepository::delete_document(&kernel, "chapter:ch1").await;
    assert!(
        deleted.is_err(),
        "chapter index deletion must use the controlled chapter path"
    );
    assert_eq!(revision(&kernel, "chapter:ch1").await, "rev@v1");
}

#[tokio::test]
async fn concurrent_old_and_new_revision_write_keeps_new_chunk() {
    let (_dir, kernel) = kernel().await;
    let kernel = std::sync::Arc::new(kernel);
    let old =
        KnowledgeRepository::index_document(kernel.as_ref(), doc("d1", "n1", "document", "rev@v2"));
    let new =
        KnowledgeRepository::index_document(kernel.as_ref(), doc("d1", "n1", "document", "rev@v3"));
    let (old_result, new_result) = tokio::join!(old, new);
    assert!(old_result.is_ok() || new_result.is_ok());
    assert_eq!(revision(&kernel, "d1").await, "rev@v3");
    let chunk_revision: String = sqlx::query_scalar(
        "SELECT c.source_revision FROM knowledge_chunk c JOIN knowledge_document d ON d.id = c.document_id WHERE d.id = 'd1'",
    ).fetch_one(kernel.pool()).await.unwrap();
    assert_eq!(chunk_revision, "rev@v3");
}

#[tokio::test]
async fn second_process_cannot_overwrite_newer_revision() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cross.sqlite");
    let first = SqliteKernel::open(&path).await.unwrap();
    let second = SqliteKernel::open(&path).await.unwrap();
    NovelRepository::create_novel(
        &first,
        novel_application::NewNovel {
            id: "n1".into(),
            title: "跨进程".into(),
            description: String::new(),
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    KnowledgeRepository::index_document(&first, doc("d1", "n1", "document", "rev@v2"))
        .await
        .unwrap();
    KnowledgeRepository::index_document(&second, doc("d1", "n1", "document", "rev@v3"))
        .await
        .unwrap();
    let stale =
        KnowledgeRepository::index_document(&first, doc("d1", "n1", "document", "rev@v2")).await;
    assert!(
        stale.is_err(),
        "older process must not overwrite newer revision"
    );
    assert_eq!(revision(&second, "d1").await, "rev@v3");
    let chunk_revision: String = sqlx::query_scalar(
        "SELECT c.source_revision FROM knowledge_chunk c WHERE c.document_id = 'd1'",
    )
    .fetch_one(second.pool())
    .await
    .unwrap();
    assert_eq!(chunk_revision, "rev@v3");
}

#[tokio::test]
async fn opaque_revisions_cannot_overwrite_without_ordering() {
    let (_dir, kernel) = kernel().await;
    KnowledgeRepository::index_document(&kernel, doc("d1", "n1", "document", "alpha"))
        .await
        .unwrap();
    let replaced =
        KnowledgeRepository::index_document(&kernel, doc("d1", "n1", "document", "beta")).await;
    assert!(
        replaced.is_err(),
        "unversioned revisions must not silently replace each other"
    );
    assert_eq!(revision(&kernel, "d1").await, "alpha");
}
