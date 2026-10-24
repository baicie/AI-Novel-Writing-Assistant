use novel_application::{
    KnowledgeRepository, NewDocument, NewEmbedding, NewNovel, NovelRepository,
};
use novel_context::{Lexicon, fts_query, tokenize};
use novel_storage_sqlite::SqliteKernel;

fn doc(id: &str, body: &str, revision: &str, hash: &str) -> NewDocument {
    let lexicon = Lexicon {
        aliases: vec!["魂兽".into()],
    };
    NewDocument {
        id: id.into(),
        novel_id: "n".into(),
        owner_type: "character".into(),
        title: id.into(),
        body: body.into(),
        terms: fts_query(&tokenize(body, &lexicon)),
        created_at: 2,
        source_revision: Some(revision.into()),
        source_content_hash: Some(hash.into()),
        embedding: Some(NewEmbedding {
            model: "local".into(),
            normalized: true,
            vector: vec![1.0, 0.0],
        }),
    }
}

#[tokio::test]
async fn reindex_replaces_old_chunks_and_failed_rebuild_is_not_queryable() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = SqliteKernel::open(&dir.path().join("knowledge.sqlite"))
        .await
        .unwrap();
    NovelRepository::create_novel(
        &kernel,
        NewNovel {
            id: "n".into(),
            title: "索引书".into(),
            description: String::new(),
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    KnowledgeRepository::index_document(
        &kernel,
        doc("char", "旧正文 林动遇见魂兽", "rev-1", "hash-1"),
    )
    .await
    .unwrap();
    let old = KnowledgeRepository::search_keyword(
        &kernel,
        "n",
        &fts_query(&tokenize("魂兽", &Lexicon::default())),
        5,
    )
    .await
    .unwrap();
    assert_eq!(old.len(), 1);

    KnowledgeRepository::index_document(
        &kernel,
        doc("char", "新正文 林动进入祖祠", "rev-2", "hash-2"),
    )
    .await
    .unwrap();
    let stale = KnowledgeRepository::search_keyword(
        &kernel,
        "n",
        &fts_query(&tokenize("魂兽", &Lexicon::default())),
        5,
    )
    .await
    .unwrap();
    assert!(stale.is_empty(), "old revision must not be searchable");
    let fresh = KnowledgeRepository::search_keyword(
        &kernel,
        "n",
        &fts_query(&tokenize("祖祠", &Lexicon::default())),
        5,
    )
    .await
    .unwrap();
    assert_eq!(fresh.len(), 1);
    assert_eq!(fresh[0].source_revision.as_deref(), Some("rev-2"));
    assert_eq!(fresh[0].index_version, 2);
    assert!(fresh[0].chunk_hash.is_some());

    // Simulate a crash after the "building" marker: stale rows must not be served.
    sqlx::query("UPDATE knowledge_document SET index_status='building' WHERE id='char'")
        .execute(kernel.pool())
        .await
        .unwrap();
    let during = KnowledgeRepository::search_keyword(
        &kernel,
        "n",
        &fts_query(&tokenize("祖祠", &Lexicon::default())),
        5,
    )
    .await
    .unwrap();
    assert!(during.is_empty(), "building index must not be queried");
    KnowledgeRepository::index_document(
        &kernel,
        doc("char", "重建正文 苏晚在祖祠", "rev-3", "hash-3"),
    )
    .await
    .unwrap();
    let rebuilt = KnowledgeRepository::search_keyword(
        &kernel,
        "n",
        &fts_query(&tokenize("苏晚", &Lexicon::default())),
        5,
    )
    .await
    .unwrap();
    assert_eq!(rebuilt.len(), 1);

    let source: (String, String, i64) = sqlx::query_as(
        "SELECT source_revision, source_content_hash, index_version FROM knowledge_document WHERE id='char'",
    ).fetch_one(kernel.pool()).await.unwrap();
    assert_eq!(source, ("rev-3".into(), "hash-3".into(), 3));

    // Wrong embedding model is not silently mixed in.
    let wrong =
        KnowledgeRepository::search_vector(&kernel, "n", "other-model", &[1.0, 0.0], true, 5)
            .await
            .unwrap();
    assert!(wrong.is_empty());

    assert!(
        KnowledgeRepository::delete_document(&kernel, "char")
            .await
            .unwrap()
    );
    let deleted = KnowledgeRepository::search_keyword(
        &kernel,
        "n",
        &fts_query(&tokenize("苏晚", &Lexicon::default())),
        5,
    )
    .await
    .unwrap();
    assert!(deleted.is_empty(), "deleted source must not be searchable");
}

#[tokio::test]
async fn confirmed_chapter_revision_updates_and_deletes_chapter_index() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = SqliteKernel::open(&dir.path().join("chapter-index.sqlite"))
        .await
        .unwrap();
    NovelRepository::create_novel(
        &kernel,
        NewNovel {
            id: "n".into(),
            title: "章节索引".into(),
            description: String::new(),
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    NovelRepository::create_chapter(
        &kernel,
        novel_application::NewChapter {
            id: "ch1".into(),
            novel_id: "n".into(),
            title: "第一章".into(),
            sort_order: 1,
            volume_id: None,
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    NovelRepository::save_user_chapter(
        &kernel,
        novel_application::SaveChapterRecord {
            novel_id: "n".into(),
            chapter_id: "ch1".into(),
            expected_version: 0,
            title: "第一章".into(),
            content: "林动进入山门。".into(),
            revision_id: "rev1".into(),
            artifact_id: "art1".into(),
            content_sha256: "hash1".into(),
            now_ms: 2,
        },
    )
    .await
    .unwrap();
    let first = KnowledgeRepository::search_keyword(
        &kernel,
        "n",
        &fts_query(&tokenize("山门", &Lexicon::default())),
        5,
    )
    .await
    .unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].chunk_id, "chapter:ch1:0");
    assert!(
        first[0]
            .source_revision
            .as_deref()
            .unwrap_or("")
            .contains("rev1")
    );

    NovelRepository::save_user_chapter(
        &kernel,
        novel_application::SaveChapterRecord {
            novel_id: "n".into(),
            chapter_id: "ch1".into(),
            expected_version: 1,
            title: "第一章".into(),
            content: "林动进入祖祠。".into(),
            revision_id: "rev2".into(),
            artifact_id: "art2".into(),
            content_sha256: "hash2".into(),
            now_ms: 3,
        },
    )
    .await
    .unwrap();
    let old = KnowledgeRepository::search_keyword(
        &kernel,
        "n",
        &fts_query(&tokenize("山门", &Lexicon::default())),
        5,
    )
    .await
    .unwrap();
    assert!(old.is_empty(), "old chapter revision must leave the index");
    let new = KnowledgeRepository::search_keyword(
        &kernel,
        "n",
        &fts_query(&tokenize("祖祠", &Lexicon::default())),
        5,
    )
    .await
    .unwrap();
    assert_eq!(new.len(), 1);
    assert!(
        new[0]
            .source_revision
            .as_deref()
            .unwrap_or("")
            .contains("rev2")
    );

    NovelRepository::delete_chapter(
        &kernel,
        novel_application::DeleteChapterRecord {
            novel_id: "n".into(),
            chapter_id: "ch1".into(),
            expected_version: 2,
        },
    )
    .await
    .unwrap();
    let deleted = KnowledgeRepository::search_keyword(
        &kernel,
        "n",
        &fts_query(&tokenize("祖祠", &Lexicon::default())),
        5,
    )
    .await
    .unwrap();
    assert!(deleted.is_empty(), "deleted chapter must leave the index");
}
