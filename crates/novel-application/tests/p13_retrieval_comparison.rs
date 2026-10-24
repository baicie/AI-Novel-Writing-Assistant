use std::collections::HashMap;

use novel_application::{
    KnowledgeRepository, NewDocument, NewEmbedding, NewNovel, NovelRepository, SearchHit,
};
use novel_context::{Lexicon, fts_query, rrf, tokenize};
use novel_storage_sqlite::SqliteKernel;
use sha2::Digest;

fn tag(order: usize) -> String {
    let first = char::from_u32(0x4e00 + (order * 2) as u32).unwrap_or('一');
    let second = char::from_u32(0x4e00 + (order * 2 + 1) as u32).unwrap_or('二');
    format!("{first}{second}")
}

fn embedding(text: &str, lexicon: &Lexicon) -> Vec<f32> {
    let mut vector = vec![0.0f32; 64];
    for token in tokenize(text, lexicon) {
        let digest = sha2::Sha256::digest(token.as_bytes());
        let index = usize::from(digest[0]) % vector.len();
        let sign = if digest[1] % 2 == 0 { 1.0 } else { -1.0 };
        vector[index] += sign;
    }
    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in &mut vector {
            *value /= norm;
        }
    }
    vector
}

fn metrics(ranks: &[Option<usize>], positives: usize) -> (f64, f64, f64) {
    let recall =
        ranks.iter().filter(|rank| rank.is_some()).count() as f64 / positives.max(1) as f64;
    let mrr = ranks
        .iter()
        .filter_map(|rank| *rank)
        .map(|rank| 1.0 / rank as f64)
        .sum::<f64>()
        / ranks.len().max(1) as f64;
    let ndcg = ranks
        .iter()
        .filter_map(|rank| *rank)
        .map(|rank| 1.0 / (1.0 + (rank as f64).log2()))
        .sum::<f64>()
        / ranks.len().max(1) as f64;
    (recall, mrr, ndcg)
}

fn rank_of(hits: &[SearchHit], expected: &str) -> Option<usize> {
    hits.iter()
        .position(|hit| hit.chunk_id == expected)
        .map(|index| index + 1)
}

#[tokio::test]
async fn fts_vector_hybrid_and_context_policy_on_frozen_fixture() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = SqliteKernel::open(&dir.path().join("comparison.sqlite"))
        .await
        .unwrap();
    NovelRepository::create_novel(
        &kernel,
        NewNovel {
            id: "n".into(),
            title: "检索对照".into(),
            description: String::new(),
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    let lexicon = Lexicon::default();
    let characters = ["林动", "苏晚", "陆沉", "林若", "青翼蝠王"];
    let locations = ["青阳城", "祖祠", "山门", "黑风寨", "北境"];
    for order in 1..=50usize {
        let body = format!(
            "第{order}章。{}在{}行动，{}指向宗门旧事。",
            characters[order % 5],
            locations[order % 5],
            tag(order)
        );
        let terms = fts_query(&tokenize(&body, &lexicon));
        KnowledgeRepository::index_document(
            &kernel,
            NewDocument {
                id: format!("chapter:{order}"),
                novel_id: "n".into(),
                owner_type: "chapter".into(),
                title: format!("第{order}章"),
                body: body.clone(),
                terms,
                created_at: order as i64,
                source_revision: Some(format!("rev-{order}")),
                source_content_hash: None,
                embedding: Some(NewEmbedding {
                    model: "mock-64".into(),
                    normalized: true,
                    vector: embedding(&body, &lexicon),
                }),
            },
        )
        .await
        .unwrap();
    }
    for plan in 1..=5 {
        let body = format!("未来计划 林动 成为掌门 计划{plan}");
        KnowledgeRepository::index_document(
            &kernel,
            NewDocument {
                id: format!("future-{plan}"),
                novel_id: "n".into(),
                owner_type: "future_plan".into(),
                title: "未来计划".into(),
                body: body.clone(),
                terms: fts_query(&tokenize(&body, &lexicon)),
                created_at: 100 + plan,
                source_revision: Some(format!("plan-{plan}")),
                source_content_hash: None,
                embedding: Some(NewEmbedding {
                    model: "mock-64".into(),
                    normalized: true,
                    vector: embedding(&body, &lexicon),
                }),
            },
        )
        .await
        .unwrap();
    }
    let mut questions = Vec::new();
    for i in 1..=20 {
        questions.push(("clue", tag(i), Some(format!("chapter:{i}:0")), i));
    }
    for i in 1..=10 {
        questions.push((
            "location",
            format!("{} {}", tag(i), locations[i % 5]),
            Some(format!("chapter:{i}:0")),
            i,
        ));
    }
    for i in 1..=10 {
        questions.push((
            "alias",
            format!("{} 青翼蝠王", tag(i)),
            Some(format!("chapter:{i}:0")),
            i,
        ));
    }
    for _ in 1..=10 {
        questions.push(("no_answer", "齉齉".into(), None, 50));
    }
    assert_eq!(questions.len(), 50);

    let mut fts = Vec::new();
    let mut vector = Vec::new();
    let mut hybrid = Vec::new();
    let mut policy = Vec::new();
    let mut fts_none = 0;
    for (_, query, expected, _) in &questions {
        let q = fts_query(&tokenize(query, &lexicon));
        let keyword_hits = KnowledgeRepository::search_keyword(&kernel, "n", &q, 10)
            .await
            .unwrap();
        let query_vector = embedding(query, &lexicon);
        let vector_hits =
            KnowledgeRepository::search_vector(&kernel, "n", "mock-64", &query_vector, true, 10)
                .await
                .unwrap();
        if expected.is_none() {
            fts_none += usize::from(keyword_hits.is_empty());
            continue;
        }
        let expected = expected.as_ref().unwrap();
        fts.push(rank_of(&keyword_hits, expected));
        vector.push(rank_of(&vector_hits, expected));
        let keyword_ids = keyword_hits
            .iter()
            .map(|hit| hit.chunk_id.clone())
            .collect::<Vec<_>>();
        let vector_ids = vector_hits
            .iter()
            .map(|hit| hit.chunk_id.clone())
            .collect::<Vec<_>>();
        let fused = rrf(&[keyword_ids.clone(), vector_ids.clone()], 60.0);
        let by_id: HashMap<String, SearchHit> = keyword_hits
            .iter()
            .chain(vector_hits.iter())
            .map(|hit| (hit.chunk_id.clone(), hit.clone()))
            .collect();
        let mut fused_hits = fused
            .into_iter()
            .filter_map(|(id, _)| by_id.get(&id).cloned())
            .collect::<Vec<_>>();
        fused_hits.truncate(10);
        hybrid.push(rank_of(&fused_hits, expected));
        let policy_hits = fused_hits
            .into_iter()
            .filter(|hit| hit.owner_type != "future_plan")
            .collect::<Vec<_>>();
        policy.push(rank_of(&policy_hits, expected));
    }
    let positives = fts.len();
    let (fts_recall, fts_mrr, fts_ndcg) = metrics(&fts, positives);
    let (vector_recall, vector_mrr, vector_ndcg) = metrics(&vector, positives);
    let (hybrid_recall, hybrid_mrr, hybrid_ndcg) = metrics(&hybrid, positives);
    let (policy_recall, policy_mrr, policy_ndcg) = metrics(&policy, positives);
    assert!(fts_recall > 0.0);
    assert!(vector_recall > 0.0);
    println!(
        "P13 retrieval comparison: dataset=p13-large-v1 questions={} positives={positives} no_answer=10 embedding=mock-64 deterministic",
        questions.len()
    );
    println!("FTS recall@10={fts_recall:.2} mrr={fts_mrr:.2} ndcg@10={fts_ndcg:.2}");
    println!("VECTOR recall@10={vector_recall:.2} mrr={vector_mrr:.2} ndcg@10={vector_ndcg:.2}");
    println!(
        "HYBRID RRF recall@10={hybrid_recall:.2} mrr={hybrid_mrr:.2} ndcg@10={hybrid_ndcg:.2}"
    );
    println!(
        "HYBRID+POLICY recall@10={policy_recall:.2} mrr={policy_mrr:.2} ndcg@10={policy_ndcg:.2}"
    );
    println!(
        "no_answer_fts_empty={fts_none}/10; vector no-answer N/A (no score threshold in SearchHit API)"
    );
}
