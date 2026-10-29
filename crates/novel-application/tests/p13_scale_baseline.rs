use std::sync::Arc;
use std::time::Instant;

use novel_application::{
    KnowledgeRepository, NewChapter, NewNovel, NovelRepository, assemble_director_context,
};
use novel_context::{Lexicon, fts_query, tokenize};
use novel_domain::{ArtifactStatus, ArtifactView};
use novel_storage_sqlite::SqliteKernel;
use novel_workflow::book_scope;

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

fn pct(samples: &mut [u128], p: usize) -> u128 {
    samples.sort_unstable();
    samples[((samples.len() - 1) * p) / 100]
}

struct Scale {
    build_ms: u128,
    context_p50: u128,
    context_p95: u128,
    context_p99: u128,
    search_p50: u128,
    search_p95: u128,
    search_p99: u128,
    concurrent_ms: u128,
    incremental_ms: u128,
    db_bytes: i64,
    revisions: i64,
    chunks: i64,
}

async fn run_scale(size: usize) -> Scale {
    let dir = tempfile::tempdir().unwrap();
    let kernel = Arc::new(
        SqliteKernel::open(&dir.path().join(format!("scale-{size}.sqlite")))
            .await
            .unwrap(),
    );
    let novel = format!("n{size}");
    NovelRepository::create_novel(
        kernel.as_ref(),
        NewNovel {
            id: novel.clone(),
            title: format!("{size}章规模"),
            description: String::new(),
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    let build_start = Instant::now();
    for order in 1..=size {
        let id = format!("ch{order}");
        NovelRepository::create_chapter(
            kernel.as_ref(),
            NewChapter {
                id: id.clone(),
                novel_id: novel.clone(),
                title: format!("第{order}章"),
                sort_order: order as i32,
                volume_id: None,
                now_ms: order as i64,
            },
        )
        .await
        .unwrap();
        if order < size {
            let content = format!(
                "第{order}章正文。{}专属事实{order}。",
                "林动在宗门中推进主线，人物关系、地点与长期伏笔持续变化。".repeat(50)
            );
            NovelRepository::save_user_chapter(
                kernel.as_ref(),
                novel_application::SaveChapterRecord {
                    novel_id: novel.clone(),
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
    let build_ms = build_start.elapsed().as_millis();
    let chapters = (1..=size)
        .map(|order| {
            serde_json::json!({
                "chapterOrder": order, "title": format!("第{order}章"), "volumeTitle": "卷一",
                "summary": format!("摘要{order}"), "characterNames": ["林动"]
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
    let target = format!("ch{size}");
    let mut context_samples = Vec::new();
    for _ in 0..10 {
        let start = Instant::now();
        let assembly =
            assemble_director_context(kernel.as_ref(), &artifacts, &novel, &target, 24_000, 8_000)
                .await
                .unwrap();
        assert!(assembly.prompt_text.chars().count() <= 24_000);
        context_samples.push(start.elapsed().as_micros());
    }
    let lexicon = Lexicon::default();
    let query = fts_query(&tokenize("专属事实42", &lexicon));
    let mut search_samples = Vec::new();
    for _ in 0..20 {
        let start = Instant::now();
        let hits = KnowledgeRepository::search_keyword(kernel.as_ref(), &novel, &query, 8)
            .await
            .unwrap();
        assert!(!hits.is_empty());
        search_samples.push(start.elapsed().as_micros());
    }
    let mut tasks = Vec::new();
    let concurrent_start = Instant::now();
    for _ in 0..8 {
        let kernel = kernel.clone();
        let novel = novel.clone();
        let query = query.clone();
        tasks.push(tokio::spawn(async move {
            for _ in 0..10 {
                let hits = KnowledgeRepository::search_keyword(kernel.as_ref(), &novel, &query, 8)
                    .await
                    .unwrap();
                assert!(!hits.is_empty());
            }
        }));
    }
    for task in tasks {
        task.await.unwrap();
    }
    let concurrent_ms = concurrent_start.elapsed().as_millis();
    let update_start = Instant::now();
    NovelRepository::save_user_chapter(
        kernel.as_ref(),
        novel_application::SaveChapterRecord {
            novel_id: novel.clone(),
            chapter_id: format!("ch{}", size / 2),
            expected_version: 1,
            title: format!("第{}章", size / 2),
            content: "第500章新正文 专属收敛词".into(),
            revision_id: format!("inc-{size}"),
            artifact_id: format!("inc-a-{size}"),
            content_sha256: format!("inc-h-{size}"),
            now_ms: 9_000,
        },
    )
    .await
    .unwrap();
    let incremental_ms = update_start.elapsed().as_millis();
    let converged = KnowledgeRepository::search_keyword(
        kernel.as_ref(),
        &novel,
        &fts_query(&tokenize("专属收敛词", &lexicon)),
        5,
    )
    .await
    .unwrap();
    assert!(!converged.is_empty());
    let db_bytes: i64 = sqlx::query_scalar(
        "SELECT page_count * page_size FROM pragma_page_count(), pragma_page_size()",
    )
    .fetch_one(kernel.pool())
    .await
    .unwrap_or(0);
    let revisions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM chapter_revision")
        .fetch_one(kernel.pool())
        .await
        .unwrap();
    let chunks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM knowledge_chunk")
        .fetch_one(kernel.pool())
        .await
        .unwrap();
    let context_p50 = pct(&mut context_samples, 50);
    let context_p95 = pct(&mut context_samples, 95);
    let context_p99 = pct(&mut context_samples, 99);
    let search_p50 = pct(&mut search_samples, 50);
    let search_p95 = pct(&mut search_samples, 95);
    let search_p99 = pct(&mut search_samples, 99);
    Scale {
        build_ms,
        context_p50,
        context_p95,
        context_p99,
        search_p50,
        search_p95,
        search_p99,
        concurrent_ms,
        incremental_ms,
        db_bytes,
        revisions,
        chunks,
    }
}

#[tokio::test]
async fn hundred_five_hundred_thousand_chapter_realistic_scale_baseline() {
    for size in [100usize, 500, 1000] {
        let result = run_scale(size).await;
        println!(
            "P13 scale: chapters={size} build_ms={} context_p50_us={} context_p95_us={} context_p99_us={} search_p50_us={} search_p95_us={} search_p99_us={} concurrent_8x10_ms={} incremental_ms={} db_bytes={} revisions={} chunks={}",
            result.build_ms,
            result.context_p50,
            result.context_p95,
            result.context_p99,
            result.search_p50,
            result.search_p95,
            result.search_p99,
            result.concurrent_ms,
            result.incremental_ms,
            result.db_bytes,
            result.revisions,
            result.chunks
        );
    }
}
