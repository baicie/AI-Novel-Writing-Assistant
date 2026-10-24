use std::sync::Arc;
use std::time::Instant;

use novel_application::{
    KnowledgeRepository, NewChapter, NewDocument, NewNovel, NovelRepository,
    assemble_director_context,
};
use novel_context::{Lexicon, fts_query, tokenize};
use novel_domain::{ArtifactStatus, ArtifactView};
use novel_storage_sqlite::SqliteKernel;
use novel_workflow::book_scope;

fn pct(samples: &mut [u128], p: usize) -> u128 {
    samples.sort_unstable();
    samples[((samples.len() - 1) * p) / 100]
}

fn artifact(ty: &str, payload: serde_json::Value) -> ArtifactView {
    ArtifactView {
        artifact_type: ty.into(),
        scope: book_scope().to_string(),
        version: 1,
        status: ArtifactStatus::Confirmed,
        protected_user_content: false,
        payload_json: payload.to_string(),
    }
}

#[tokio::test]
async fn thousand_chapter_context_and_index_baseline() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = Arc::new(
        SqliteKernel::open(&dir.path().join("perf.sqlite"))
            .await
            .unwrap(),
    );
    NovelRepository::create_novel(
        kernel.as_ref(),
        NewNovel {
            id: "n".into(),
            title: "千章性能".into(),
            description: String::new(),
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    let create_start = Instant::now();
    let mut chapter_lengths = Vec::new();
    for order in 1..=1000 {
        let id = format!("ch{order}");
        NovelRepository::create_chapter(
            kernel.as_ref(),
            NewChapter {
                id: id.clone(),
                novel_id: "n".into(),
                title: format!("第{order}章"),
                sort_order: order,
                volume_id: None,
                now_ms: order as i64,
            },
        )
        .await
        .unwrap();
        if order < 1000 {
            let content = format!(
                "第{order}章正文。{}事实节点{order}。",
                "林动在宗门中推进主线，人物关系、地点与长期伏笔持续变化。".repeat(50)
            );
            chapter_lengths.push(content.chars().count());
            NovelRepository::save_user_chapter(
                kernel.as_ref(),
                novel_application::SaveChapterRecord {
                    novel_id: "n".into(),
                    chapter_id: id,
                    expected_version: 0,
                    title: format!("第{order}章"),
                    content,
                    revision_id: format!("r{order}"),
                    artifact_id: format!("a{order}"),
                    content_sha256: format!("h{order}"),
                    now_ms: 1_000 + order as i64,
                },
            )
            .await
            .unwrap();
        }
    }
    let chapters = (1..=1000)
        .map(|order| {
            serde_json::json!({
                "chapterOrder": order, "title": format!("第{order}章"), "volumeTitle": "卷一",
                "summary": format!("摘要 {order}"), "characterNames": ["林动"]
            })
        })
        .collect::<Vec<_>>();
    let artifacts = vec![
        artifact(
            "story_macro",
            serde_json::json!({"expansion":{"expanded_premise":"x"},"decomposition":{},"constraints":[],"issues":[]}),
        ),
        artifact(
            "character_cast",
            serde_json::json!({"members":[{"name":"林动","role":"主角","castRole":"protagonist","storyFunction":"推进"}],"relations":[]}),
        ),
        artifact(
            "volume_strategy",
            serde_json::json!({"volumes":[{"volumeOrder":1,"title":"卷一","roleLabel":"开局","coreReward":"入局","planningMode":"hard"}]}),
        ),
        artifact("chapter_list", serde_json::json!({"chapters": chapters})),
    ];
    let mut context_samples = Vec::new();
    for _ in 0..10 {
        let start = Instant::now();
        let assembly =
            assemble_director_context(kernel.as_ref(), &artifacts, "n", "ch1000", 24_000, 8_000)
                .await
                .unwrap();
        assert!(assembly.prompt_text.chars().count() <= 24_000);
        context_samples.push(start.elapsed().as_micros());
    }
    let update_start = Instant::now();
    NovelRepository::save_user_chapter(
        kernel.as_ref(),
        novel_application::SaveChapterRecord {
            novel_id: "n".into(),
            chapter_id: "ch500".into(),
            expected_version: 1,
            title: "第500章".into(),
            content: "第500章新正文 专属收敛词".into(),
            revision_id: "incremental-rev".into(),
            artifact_id: "incremental-art".into(),
            content_sha256: "incremental-hash".into(),
            now_ms: 5_000,
        },
    )
    .await
    .unwrap();
    let incremental_ms = update_start.elapsed().as_millis();
    let lexicon = Lexicon::default();
    let converged = KnowledgeRepository::search_keyword(
        kernel.as_ref(),
        "n",
        &fts_query(&tokenize("专属收敛词", &lexicon)),
        5,
    )
    .await
    .unwrap();
    assert!(
        !converged.is_empty(),
        "chapter revision must converge into index synchronously"
    );
    assert_eq!(converged[0].chunk_id, "chapter:ch500:0");
    let index_start = Instant::now();
    for i in 0..200 {
        let body = format!("知识条目 {i}：林动在宗门记录事实{i}。无关干扰段落{i}。");
        KnowledgeRepository::index_document(
            kernel.as_ref(),
            NewDocument {
                id: format!("doc{i}"),
                novel_id: "n".into(),
                owner_type: "world".into(),
                title: format!("doc{i}"),
                body: body.clone(),
                terms: fts_query(&tokenize(&body, &lexicon)),
                created_at: i as i64,
                source_revision: Some(format!("v{i}")),
                source_content_hash: None,
                embedding: None,
            },
        )
        .await
        .unwrap();
    }
    let index_ms = index_start.elapsed().as_millis();
    let mut search_samples = Vec::new();
    for _ in 0..20 {
        let start = Instant::now();
        let hits = KnowledgeRepository::search_keyword(
            kernel.as_ref(),
            "n",
            &fts_query(&tokenize("事实42", &lexicon)),
            8,
        )
        .await
        .unwrap();
        assert!(!hits.is_empty());
        search_samples.push(start.elapsed().as_micros());
    }
    let page_count: i64 = sqlx::query_scalar("PRAGMA page_count")
        .fetch_one(kernel.pool())
        .await
        .unwrap();
    let page_size: i64 = sqlx::query_scalar("PRAGMA page_size")
        .fetch_one(kernel.pool())
        .await
        .unwrap();
    let db_mb = (page_count * page_size) as f64 / 1_048_576.0;
    let build_ms = create_start.elapsed().as_millis();
    let total_content_bytes: i64 =
        sqlx::query_scalar("SELECT COALESCE(SUM(LENGTH(content)),0) FROM chapter_revision")
            .fetch_one(kernel.pool())
            .await
            .unwrap();
    let revisions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM chapter_revision")
        .fetch_one(kernel.pool())
        .await
        .unwrap();
    let chunks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM knowledge_chunk")
        .fetch_one(kernel.pool())
        .await
        .unwrap();
    let fts_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM knowledge_fts")
        .fetch_one(kernel.pool())
        .await
        .unwrap();
    let vectors: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM embedding")
        .fetch_one(kernel.pool())
        .await
        .unwrap();
    let min_chars = chapter_lengths.iter().min().copied().unwrap_or(0);
    let max_chars = chapter_lengths.iter().max().copied().unwrap_or(0);
    let avg_chars = chapter_lengths.iter().sum::<usize>() / chapter_lengths.len().max(1);
    let context_p50 = pct(&mut context_samples, 50);
    let context_p95 = pct(&mut context_samples, 95);
    let search_p50 = pct(&mut search_samples, 50);
    let search_p95 = pct(&mut search_samples, 95);
    assert!(context_p95 < 2_000_000, "context p95 too slow");
    assert!(search_p95 < 500_000, "search p95 too slow");
    println!(
        "P13 performance: chapters=1000 chars_min={min_chars} chars_avg={avg_chars} chars_max={max_chars} content_bytes={total_content_bytes} revisions={revisions} chunks={chunks} fts_rows={fts_rows} vectors={vectors} build_ms={build_ms} context_p50_us={context_p50} context_p95_us={context_p95} incremental_update_ms={incremental_ms} index_200_ms={index_ms} search_p50_us={search_p50} search_p95_us={search_p95} db_mb={db_mb:.2} parallelism={}",
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(0)
    );
}
