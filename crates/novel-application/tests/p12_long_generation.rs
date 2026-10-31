use std::sync::Arc;

use novel_application::{OpenRunInput, Runtime, RuntimeDeps, TickOutcome, WorkflowRepository};
use novel_storage_sqlite::SqliteKernel;
use novel_testkit::{FakeClock, ScriptedLlm, SequentialIds};

fn story() -> String {
    serde_json::json!({"expansion":{"expanded_premise":"少年入山。","protagonist_core":"被逐少年","conflict_engine":"宗门秘密","conflict_layers":{"external":"外","internal":"内"},"mystery_box":"祖祠","emotional_line":"成长","setpiece_seeds":["夜战"],"tone_reference":"冷峻"},"decomposition":{"selling_point":"逆袭","core_conflict":"真相","main_hook":"血脉","progression_loop":"发现","growth_path":"承担","major_payoffs":["真相"],"ending_flavor":"新生"},"constraints":["不揭露终极秘密"],"issues":[]}).to_string()
}
fn cast() -> String {
    serde_json::json!({"members":[{"name":"林动","role":"主角","castRole":"protagonist","storyFunction":"承载真相"},{"name":"苏晚","role":"同伴","castRole":"ally","storyFunction":"协助"},{"name":"陆沉","role":"对手","castRole":"antagonist","storyFunction":"施压"}],"relations":[{"sourceName":"林动","targetName":"苏晚","surfaceRelation":"同门"},{"sourceName":"林动","targetName":"陆沉","surfaceRelation":"敌对"}]}).to_string()
}
fn volume() -> String {
    serde_json::json!({"volumes":[{"volumeOrder":1,"title":"山门卷","roleLabel":"开局","coreReward":"入门","planningMode":"hard"}]}).to_string()
}
fn chapter_plan() -> String {
    let chapters: Vec<_> = (1..=100)
        .map(|order| {
            serde_json::json!({
                "chapterOrder": order, "title": format!("第{order}章"), "volumeTitle": "山门卷",
                "summary": format!("章节摘要 {order}：林动推进主线。"), "characterNames": ["林动"]
            })
        })
        .collect();
    serde_json::json!({"chapters": chapters}).to_string()
}
fn clean_review() -> String {
    serde_json::json!({"issues":[]}).to_string()
}

#[tokio::test]
async fn hundred_chapter_mock_generation_benchmark() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = Arc::new(
        SqliteKernel::open(&dir.path().join("long-gen.sqlite"))
            .await
            .unwrap(),
    );
    let mut outputs = vec![
        ("STEP:story_macro", story()),
        ("STEP:character_cast", cast()),
        ("STEP:volume_strategy", volume()),
        ("STEP:chapter_list", chapter_plan()),
    ];
    for order in 1..=100 {
        outputs.push((
            "STEP:chapter_draft",
            format!("第{order}章正文：林动推进主线，事实节点 {order}。"),
        ));
        outputs.push(("STEP:consistency_review", clean_review()));
    }
    let llm = Arc::new(ScriptedLlm::new(outputs));
    let runtime = Runtime::new(RuntimeDeps {
        novels: kernel.clone(),
        runs: kernel.clone(),
        worlds: kernel.clone(),
        lookups: kernel.clone(),
        task_center: kernel.clone(),
        llm: llm.clone(),
        clock: Arc::new(FakeClock::new(1_000)),
        ids: Arc::new(SequentialIds::new()),
        faults: Arc::new(novel_application::NoFault),
        secrets: Arc::new(novel_application::NoModelSecrets),
        owner: "long-gen".into(),
        lease_ms: 120_000,
        concurrency: 1,
    });
    let novel = runtime
        .director
        .create_novel("长篇生成基准", "")
        .await
        .unwrap();
    let mut chapters = Vec::new();
    for order in 1..=100 {
        chapters.push(
            runtime
                .director
                .create_chapter(&novel.id, &format!("第{order}章"), order)
                .await
                .unwrap(),
        );
    }
    let mut prompt_snapshots = Vec::new();
    let mut character_hits = 0;
    for (index, chapter) in chapters.iter().enumerate() {
        let opened = runtime
            .director
            .open_run(OpenRunInput {
                command_id: format!("long-gen-{}", index + 1),
                novel_id: novel.id.clone(),
                target_chapter_id: chapter.id.clone(),
                provider: "mock".into(),
                model: "mock".into(),
                prompt_versions: {
                    let mut p = std::collections::BTreeMap::new();
                    p.insert("chapter_draft".into(), "chapter-draft-v2".into());
                    p
                },
                token_budget: Some(512),
                context_token_budget: Some(24_000),
                max_automatic_retries: 0,
                repair_budget: Some(0),
                endpoint: "https://api.example.com/v1/chat/completions".into(),
                allow_localhost_http: false,
                step_routes: Default::default(),
            })
            .await
            .unwrap();
        let mut status = "queued".to_string();
        for tick in 0..80 {
            match runtime.worker.tick().await.unwrap() {
                TickOutcome::Advanced {
                    status: next,
                    run_id,
                } => {
                    status = next;
                    if status == "waiting_approval" {
                        runtime
                            .director
                            .approve(&format!("long-gen-approve-{index}-{tick}"), &run_id)
                            .await
                            .unwrap();
                    }
                    if status == "succeeded" || status == "paused" {
                        break;
                    }
                }
                TickOutcome::Idle => break,
                other => panic!("unexpected worker outcome: {other:?}"),
            }
        }
        assert_eq!(
            status,
            "succeeded",
            "chapter {} did not finish: {status}",
            index + 1
        );
        let saved = novel_application::NovelRepository::get_chapter(
            kernel.as_ref(),
            &novel.id,
            &chapter.id,
        )
        .await
        .unwrap();
        assert!(saved.content.contains(&format!("事实节点 {}", index + 1)));
        if saved.content.contains("林动") {
            character_hits += 1;
        }
        let contexts = WorkflowRepository::run_artifacts(kernel.as_ref(), &opened.run_id)
            .await
            .unwrap();
        let context = contexts
            .iter()
            .find(|artifact| artifact.artifact_type == "context")
            .expect("context artifact");
        let payload: serde_json::Value = serde_json::from_str(&context.payload_json).unwrap();
        prompt_snapshots.push(
            payload
                .get("promptText")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        );
    }
    // Continuity: every chapter after the first must see the immediately previous official chapter.
    for (index, snapshot) in prompt_snapshots.iter().enumerate().skip(1) {
        assert!(
            snapshot.contains(&format!("第{index}章正文")),
            "chapter {} context is missing previous chapter",
            index + 1
        );
    }
    assert!(!prompt_snapshots[0].contains("未确认"));
    let debt: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM chapter_quality_debt WHERE run_id IS NOT NULL")
            .fetch_one(kernel.pool())
            .await
            .unwrap();
    let committed: i64 = sqlx::query_scalar("SELECT COUNT(DISTINCT chapter_id) FROM chapter_revision WHERE chapter_id IN (SELECT id FROM chapter WHERE novel_id = ?)").bind(&novel.id).fetch_one(kernel.pool()).await.unwrap();
    assert_eq!(
        committed, 100,
        "all hundred chapters must have official revisions"
    );
    let commits: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM workflow_artifact WHERE type = 'chapter_commit'")
            .fetch_one(kernel.pool())
            .await
            .unwrap();
    assert_eq!(commits, 100);
    println!(
        "P12 long-generation benchmark: chapters=100 commits=100 prior_context_recall=99/99 character_presence={character_hits}/100 review_issues={debt} repair_budget=0"
    );
}
