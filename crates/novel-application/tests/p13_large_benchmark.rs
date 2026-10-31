use novel_application::{KnowledgeRepository, NewDocument, NewNovel, NovelRepository};
use novel_context::{Lexicon, fts_query, tokenize};
use novel_storage_sqlite::SqliteKernel;

fn tag(order: usize) -> String {
    let first = char::from_u32(0x4e00 + (order * 2) as u32).unwrap_or('一');
    let second = char::from_u32(0x4e00 + (order * 2 + 1) as u32).unwrap_or('二');
    format!("{first}{second}")
}

struct Question {
    category: &'static str,
    query: String,
    expected: Option<String>,
    target_chapter: usize,
    forbidden: Vec<String>,
}

fn metrics(ranks: &[Option<usize>], answered: usize) -> (f64, f64, f64, f64) {
    if ranks.is_empty() {
        return (0.0, 0.0, 0.0, 0.0);
    }
    let recall = ranks.iter().filter(|rank| rank.is_some()).count() as f64 / answered.max(1) as f64;
    let precision = ranks
        .iter()
        .filter_map(|rank| *rank)
        .map(|_| 1.0 / 5.0)
        .sum::<f64>()
        / ranks.len() as f64;
    let mrr = ranks
        .iter()
        .filter_map(|rank| *rank)
        .map(|rank| 1.0 / rank as f64)
        .sum::<f64>()
        / ranks.len() as f64;
    let ndcg = ranks
        .iter()
        .filter_map(|rank| *rank)
        .map(|rank| 1.0 / (1.0 + (rank as f64).log2()))
        .sum::<f64>()
        / ranks.len() as f64;
    (recall, precision, mrr, ndcg)
}

#[tokio::test]
async fn fifty_chapter_fifty_question_annotated_retrieval_benchmark() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = SqliteKernel::open(&dir.path().join("benchmark.sqlite"))
        .await
        .unwrap();
    NovelRepository::create_novel(
        &kernel,
        NewNovel {
            id: "n".into(),
            title: "五十章标注集".into(),
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
        let character = characters[order % characters.len()];
        let location = locations[order % locations.len()];
        let alias = if order <= 10 {
            "青翼蝠王"
        } else {
            "无名客"
        };
        let body = format!(
            "第{order}章 标注正文。{character}在{location}行动。线索{order}指向宗门旧事。身份别称{alias}。\
             第{order}章的事件与此前章节保持连续，并留下需要后续确认的伏笔。{tag}。",
            tag = tag(order)
        );
        let terms = fts_query(&tokenize(&body, &lexicon));
        KnowledgeRepository::index_document(
            &kernel,
            NewDocument {
                id: format!("chapter:{order}"),
                novel_id: "n".into(),
                owner_type: "chapter".into(),
                title: format!("第{order}章"),
                body,
                terms,
                created_at: order as i64,
                source_revision: Some(format!("rev-{order}")),
                source_content_hash: Some(format!("hash-{order}")),
                embedding: None,
            },
        )
        .await
        .unwrap();
    }
    let mut forbidden = Vec::new();
    for plan in 1..=5 {
        let id = format!("future-{plan}");
        let body = format!("未来计划 林动 成为掌门 计划{plan}");
        KnowledgeRepository::index_document(
            &kernel,
            NewDocument {
                id: id.clone(),
                novel_id: "n".into(),
                owner_type: "future_plan".into(),
                title: "未来计划".into(),
                body: body.clone(),
                terms: fts_query(&tokenize(&body, &lexicon)),
                created_at: 1000 + plan,
                source_revision: Some(format!("plan-{plan}")),
                source_content_hash: None,
                embedding: None,
            },
        )
        .await
        .unwrap();
        forbidden.push(id);
    }
    let mut questions = Vec::new();
    for i in 1..=20usize {
        questions.push(Question {
            category: "clue",
            query: tag(i),
            expected: Some(format!("chapter:{i}:0")),
            target_chapter: i,
            forbidden: forbidden.clone(),
        });
    }
    for i in 1..=10usize {
        let location = locations[i % locations.len()];
        questions.push(Question {
            category: "location",
            query: format!("{} {location}", tag(i)),
            expected: Some(format!("chapter:{i}:0")),
            target_chapter: i,
            forbidden: forbidden.clone(),
        });
    }
    for i in 1..=10usize {
        questions.push(Question {
            category: "alias",
            query: format!("{} 青翼蝠王", tag(i)),
            expected: Some(format!("chapter:{i}:0")),
            target_chapter: i,
            forbidden: forbidden.clone(),
        });
    }
    for _i in 1..=10usize {
        questions.push(Question {
            category: "no_answer",
            query: "齉齉".into(),
            expected: None,
            target_chapter: 50,
            forbidden: forbidden.clone(),
        });
    }
    assert_eq!(questions.len(), 50);
    let mut ranks = Vec::new();
    let mut answered = 0;
    let mut revision_ok = 0;
    let mut revision_checked = 0;
    let mut future_leaks = 0;
    let mut no_answer_ok = 0;
    let mut category =
        std::collections::BTreeMap::<&str, (usize, usize, Vec<Option<usize>>)>::new();
    for question in &questions {
        let hits = KnowledgeRepository::search_keyword(
            &kernel,
            "n",
            &fts_query(&tokenize(&question.query, &lexicon)),
            10,
        )
        .await
        .unwrap();
        let rank = question.expected.as_ref().and_then(|expected| {
            hits.iter()
                .position(|hit| &hit.chunk_id == expected)
                .map(|index| index + 1)
        });
        if question.expected.is_some() {
            answered += 1;
        } else {
            no_answer_ok += usize::from(hits.is_empty());
        }
        if let Some(expected) = &question.expected
            && let Some(hit) = hits.iter().find(|hit| &hit.chunk_id == expected)
        {
            revision_checked += 1;
            revision_ok += usize::from(
                hit.source_revision.as_deref()
                    == Some(format!("rev-{}", question.target_chapter).as_str()),
            );
        }
        if hits
            .iter()
            .any(|hit| question.forbidden.contains(&hit.chunk_id))
        {
            future_leaks += 1;
        }
        ranks.push(rank);
        let entry = category
            .entry(question.category)
            .or_insert((0, 0, Vec::new()));
        entry.0 += 1;
        entry.1 += usize::from(rank.is_some());
        entry.2.push(rank);
    }
    let (recall, precision, mrr, ndcg) = metrics(&ranks[..answered], answered);
    let no_answer_accuracy = no_answer_ok as f64 / 10.0;
    assert_eq!(revision_ok, revision_checked);
    assert_eq!(
        future_leaks, 0,
        "future planning sources must not leak into retrieval"
    );
    println!(
        "P13 large benchmark: dataset=p13-large-v1 chapters=50 questions=50 answered={answered} recall@10={recall:.2} precision@5={precision:.2} mrr={mrr:.2} ndcg={ndcg:.2} revision_accuracy={:.2} future_leak={future_leaks} no_answer_accuracy={no_answer_accuracy:.2}",
        revision_ok as f64 / revision_checked.max(1) as f64
    );
    for (name, (total, hits, ranks)) in category {
        let (category_recall, _, category_mrr, _) = metrics(&ranks, hits.max(1));
        println!(
            "P13 large category: {name} cases={total} hit={hits} recall={category_recall:.2} mrr={category_mrr:.2}"
        );
    }
    assert!(recall >= 0.9, "large fixture recall regressed: {recall}");
}
