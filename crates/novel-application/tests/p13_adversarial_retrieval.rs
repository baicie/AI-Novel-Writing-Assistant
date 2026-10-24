use novel_application::{KnowledgeRepository, NewDocument, NewNovel, NovelRepository};
use novel_context::ChunkRef;
use novel_context::{Lexicon, assemble, fts_query, tokenize};
use novel_domain::ArtifactStatus;
use novel_storage_sqlite::SqliteKernel;

async fn index(kernel: &SqliteKernel, id: &str, owner: &str, body: &str, revision: &str) {
    let lexicon = Lexicon::default();
    KnowledgeRepository::index_document(
        kernel,
        NewDocument {
            id: id.into(),
            novel_id: "n".into(),
            owner_type: owner.into(),
            title: id.into(),
            body: body.into(),
            terms: fts_query(&tokenize(body, &lexicon)),
            created_at: 1,
            source_revision: Some(revision.into()),
            source_content_hash: Some(format!("hash-{revision}")),
            embedding: None,
        },
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn adversarial_retrieval_keeps_same_name_rumor_and_future_separate() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = SqliteKernel::open(&dir.path().join("adversarial.sqlite"))
        .await
        .unwrap();
    NovelRepository::create_novel(
        &kernel,
        NewNovel {
            id: "n".into(),
            title: "对抗检索".into(),
            description: String::new(),
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    index(
        &kernel,
        "young",
        "character",
        "林动 外门弟子 掌门之子 真实身份",
        "rev-1",
    )
    .await;
    index(
        &kernel,
        "impostor",
        "character",
        "林动 黑风寨盗匪 冒充同名",
        "rev-3",
    )
    .await;
    index(
        &kernel,
        "rumor",
        "document",
        "传闻 林动 已死 不可信",
        "rev-2",
    )
    .await;
    index(&kernel, "fact", "character", "林动 存活 在祖祠", "rev-4").await;
    index(
        &kernel,
        "future",
        "future_plan",
        "未来计划 林动 成为掌门",
        "plan-1",
    )
    .await;
    let lexicon = Lexicon::default();
    let cases = [
        ("掌门之子", "young:0"),
        ("冒充", "impostor:0"),
        ("已死", "rumor:0"),
        ("存活", "fact:0"),
    ];
    let mut mrr = 0.0;
    let mut correct = 0;
    for (query, expected) in cases {
        let hits = KnowledgeRepository::search_keyword(
            &kernel,
            "n",
            &fts_query(&tokenize(query, &lexicon)),
            5,
        )
        .await
        .unwrap();
        let rank = hits
            .iter()
            .position(|hit| hit.chunk_id == expected)
            .map(|index| index + 1);
        mrr += rank.map(|rank| 1.0 / rank as f64).unwrap_or(0.0);
        correct += usize::from(rank.is_some());
        assert!(
            rank.is_some(),
            "expected source {expected} missing for {query}"
        );
        assert!(hits.iter().all(|hit| hit.source_revision.is_some()));
    }
    mrr /= cases.len() as f64;
    // Future planning can be a goal but must not be assembled as confirmed character fact.
    let chunks = vec![ChunkRef {
        id: "future:0".into(),
        owner_type: "future_plan".into(),
        body: "未来计划 林动 成为掌门".into(),
    }];
    let assembled = assemble(
        &[],
        &["character".into()],
        &["future:0".into()],
        &chunks,
        false,
        200,
    );
    assert!(
        assembled.is_empty(),
        "future planning leaked into confirmed facts"
    );
    println!(
        "P13 adversarial: cases={} correct={} mrr={mrr:.2} future_leak=0",
        cases.len(),
        correct
    );
}
