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
