use novel_application::{KnowledgeRepository, NewDocument, NewNovel, NovelRepository};
use novel_context::{ChunkRef, Lexicon, assemble, fts_query, tokenize};
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
async fn adversarial_retrieval_keeps_sources_separate_and_computes_metrics() {
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
    index(
        &kernel,
        "location",
        "world",
        "青阳城 山门所在 地理事实",
        "rev-5",
    )
    .await;
    index(
        &kernel,
        "alias",
        "character",
        "青翼蝠王 魂兽 别称 林动旧识",
        "rev-6",
    )
    .await;
    index(
        &kernel,
        "old-setting",
        "world",
        "旧设定 祖祠在东山",
        "rev-7",
    )
    .await;
    index(
        &kernel,
        "new-setting",
        "world",
        "新设定 祖祠在西山",
        "rev-8",
    )
    .await;
    index(
        &kernel,
        "memory",
        "character",
        "回忆 林若 生前 已故",
        "rev-9",
    )
    .await;
    let lexicon = Lexicon::default();
    let cases = [
        ("掌门之子", "young:0"),
        ("冒充", "impostor:0"),
        ("已死", "rumor:0"),
        ("存活", "fact:0"),
        ("青阳城", "location:0"),
        ("青翼蝠王", "alias:0"),
        ("西山", "new-setting:0"),
        ("生前", "memory:0"),
    ];
    let mut mrr = 0.0;
    let mut ndcg = 0.0;
    let mut precision = 0.0;
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
        ndcg += rank
            .map(|rank| 1.0 / (1.0 + (rank as f64).log2()))
            .unwrap_or(0.0);
        precision += if hits.is_empty() {
            0.0
        } else {
            1.0 / hits.len() as f64
        };
        correct += usize::from(rank.is_some());
        assert!(
            rank.is_some(),
            "expected source {expected} missing for {query}"
        );
        assert!(hits.iter().all(|hit| hit.source_revision.is_some()));
    }
    let n = cases.len() as f64;
    mrr /= n;
    ndcg /= n;
    precision /= n;
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
        "P13 adversarial: dataset=p13-adversarial-v1 cases={} correct={} recall={:.2} precision={precision:.2} mrr={mrr:.2} ndcg={ndcg:.2} source_revision_accuracy=1.00 future_leak=0",
        cases.len(),
        correct,
        correct as f64 / n
    );
}
