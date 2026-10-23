use std::sync::Arc;

use novel_application::{
    KnowledgeRepository, NewDocument, NewEmbedding, NewNovel, NovelRepository,
};
use novel_context::{Lexicon, fts_query, tokenize};
use novel_storage_sqlite::SqliteKernel;

async fn index(kernel: &SqliteKernel, id: &str, owner: &str, body: &str, vector: Option<Vec<f32>>) {
    let lexicon = Lexicon {
        aliases: vec!["魂兽".into(), "祖祠".into()],
    };
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
            embedding: vector.map(|vector| NewEmbedding {
                model: "local".into(),
                normalized: true,
                vector,
            }),
        },
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn chinese_local_rag_recall_and_precision_benchmark() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = SqliteKernel::open(&dir.path().join("rag.sqlite"))
        .await
        .unwrap();
    NovelRepository::create_novel(
        &kernel,
        NewNovel {
            id: "n".into(),
            title: "检索书".into(),
            description: String::new(),
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    index(
        &kernel,
        "lin",
        "character",
        "林动在祖祠遇见魂兽。",
        Some(vec![1.0, 0.0, 0.0]),
    )
    .await;
    index(
        &kernel,
        "su",
        "character",
        "苏晚在山门等候林动。",
        Some(vec![0.0, 1.0, 0.0]),
    )
    .await;
    index(
        &kernel,
        "world",
        "world",
        "祖祠是宗门禁地，魂兽出没。",
        Some(vec![0.5, 0.5, 0.0]),
    )
    .await;
    index(
        &kernel,
        "weather",
        "document",
        "今天天气晴朗。",
        Some(vec![0.0, 0.0, 1.0]),
    )
    .await;
    let lexicon = Lexicon {
        aliases: vec!["魂兽".into(), "祖祠".into()],
    };
    let cases = [
        ("魂兽", vec!["lin:0", "world:0"]),
        ("祖祠", vec!["lin:0", "world:0"]),
        ("林动", vec!["lin:0", "su:0"]),
    ];
    let mut total_recall = 0.0;
    let mut total_precision = 0.0;
    for (query, expected) in &cases {
        let hits = KnowledgeRepository::search_keyword(
            &kernel,
            "n",
            &fts_query(&tokenize(query, &lexicon)),
            5,
        )
        .await
        .unwrap();
        let ids: Vec<&str> = hits.iter().map(|hit| hit.chunk_id.as_str()).collect();
        let found = expected.iter().filter(|id| ids.contains(id)).count();
        let recall = found as f32 / expected.len() as f32;
        let precision = if ids.is_empty() {
            0.0
        } else {
            found as f32 / ids.len() as f32
        };
        assert!(recall >= 0.5, "query {query} recall {recall}");
        assert!(precision >= 0.4, "query {query} precision {precision}");
        total_recall += recall;
        total_precision += precision;
    }
    let lexical_recall = total_recall / cases.len() as f32;
    let lexical_precision = total_precision / cases.len() as f32;
    let vectors =
        KnowledgeRepository::search_vector(&kernel, "n", "local", &[1.0, 0.0, 0.0], true, 3)
            .await
            .unwrap();
    assert_eq!(
        vectors.first().map(|hit| hit.chunk_id.as_str()),
        Some("lin:0")
    );
    println!(
        "P12 Chinese RAG benchmark: lexical_recall={lexical_recall:.2} lexical_precision={lexical_precision:.2} vector_top1={:?}",
        vectors.first().map(|hit| hit.chunk_id.clone())
    );
}
