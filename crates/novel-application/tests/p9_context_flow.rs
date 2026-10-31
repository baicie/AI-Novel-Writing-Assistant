use std::sync::Arc;

use novel_application::{
    NewWorld, NovelRepository, OpenRunInput, Runtime, RuntimeDeps, SaveChapterRecord, TickOutcome,
    WorkflowRepository, WorldRepository,
};
use novel_storage_sqlite::SqliteKernel;
use novel_testkit::{FakeClock, ScriptedLlm, SequentialIds};

fn prompts() -> std::collections::BTreeMap<String, String> {
    let mut prompts = std::collections::BTreeMap::new();
    prompts.insert("chapter_draft".into(), "chapter-draft-v2".into());
    prompts
}

#[tokio::test]
async fn context_artifact_feeds_the_real_draft_prompt_with_confirmed_facts() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = Arc::new(
        SqliteKernel::open(&dir.path().join("p9.sqlite"))
            .await
            .unwrap(),
    );
    let story = serde_json::json!({
        "expansion": {"expanded_premise":"林动进入山门。","protagonist_core":"被逐少年","conflict_engine":"宗门秘密","conflict_layers":{"external":"外","internal":"内"},"mystery_box":"祖祠","emotional_line":"成长","setpiece_seeds":["夜战"],"tone_reference":"冷峻"},
        "decomposition": {"selling_point":"逆袭","core_conflict":"真相与秩序","main_hook":"血脉","progression_loop":"发现","growth_path":"承担","major_payoffs":["真相"],"ending_flavor":"新生"},
        "constraints":["不揭露终极秘密"],"issues":[]
    }).to_string();
    let cast = serde_json::json!({"members":[
        {"name":"林动","role":"主角","castRole":"protagonist","storyFunction":"承载真相","personality":"坚韧"},
        {"name":"苏晚","role":"同伴","castRole":"ally","storyFunction":"协助","personality":"冷静"},
        {"name":"陆沉","role":"对手","castRole":"antagonist","storyFunction":"施压","personality":"偏执"}],
        "relations":[{"sourceName":"林动","targetName":"苏晚","surfaceRelation":"同门"},{"sourceName":"林动","targetName":"陆沉","surfaceRelation":"敌对"}]}).to_string();
    let volume = serde_json::json!({"volumes":[{"volumeOrder":1,"title":"山门卷","roleLabel":"开局","coreReward":"入门","planningMode":"hard"}]}).to_string();
    let chapters = serde_json::json!({"chapters":[
        {"chapterOrder":1,"title":"第一章","volumeTitle":"山门卷","summary":"入山","characterNames":["林动"]},
        {"chapterOrder":2,"title":"第二章","volumeTitle":"山门卷","summary":"相遇","characterNames":["林动","苏晚"]},
        {"chapterOrder":3,"title":"第三章","volumeTitle":"山门卷","summary":"夜战","characterNames":["林动","陆沉"]},
        {"chapterOrder":4,"title":"第四章","volumeTitle":"山门卷","summary":"祖祠前夜","characterNames":["林动","陆沉"]}
    ]}).to_string();
    let review = serde_json::json!({"issues":[]}).to_string();
    let llm = Arc::new(ScriptedLlm::new(vec![
        ("STEP:story_macro", story),
        ("STEP:character_cast", cast),
        ("STEP:volume_strategy", volume),
        ("STEP:chapter_list", chapters),
        ("STEP:consistency_review", review),
        (
            "STEP:chapter_draft",
            "第四章候选正文：林动在祖祠前夜拔剑。".into(),
        ),
    ]));
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
        owner: "p9-context-worker".into(),
        lease_ms: 60_000,
        concurrency: 1,
    });
    let novel = runtime
        .director
        .create_novel("上下文书", "林动被逐出家族后进入山门。")
        .await
        .unwrap();
    for order in 1..=3 {
        let chapter = runtime
            .director
            .create_chapter(&novel.id, &format!("第{order}章"), order)
            .await
            .unwrap();
        NovelRepository::save_user_chapter(
            kernel.as_ref(),
            SaveChapterRecord {
                novel_id: novel.id.clone(),
                chapter_id: chapter.id,
                expected_version: 0,
                title: chapter.title,
                content: format!("第{order}章正式正文：林动在山门中的经历。"),
                revision_id: format!("rev-{order}"),
                artifact_id: format!("art-{order}"),
                content_sha256: format!("hash-{order}"),
                now_ms: 10 + i64::from(order),
            },
        )
        .await
        .unwrap();
    }
    let target = runtime
        .director
        .create_chapter(&novel.id, "第四章", 4)
        .await
        .unwrap();
    WorldRepository::create_world(
        kernel.as_ref(),
        NewWorld {
            id: "world-p9".into(),
            name: "山门世界".into(),
            description: "山门以剑为尊，祖祠只在月圆之夜开启。".into(),
            overview_summary: "外门弟子不得进入祖祠。".into(),
            source_template_id: None,
            owner_novel_id: Some(novel.id.clone()),
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    sqlx::query("INSERT INTO character(id, novel_id, name, role, profile_json, created_at, updated_at) VALUES ('char-p9', ?, '林动', '主角', '{\"state\":\"刚入外门\"}', 1, 1)")
        .bind(&novel.id).execute(kernel.as_ref().pool()).await.unwrap();
    let opened = runtime
        .director
        .open_run(OpenRunInput {
            command_id: "p9-context-open".into(),
            novel_id: novel.id.clone(),
            target_chapter_id: target.id.clone(),
            provider: "mock".into(),
            model: "mock-model".into(),
            prompt_versions: prompts(),
            token_budget: Some(512),
            context_token_budget: Some(24_000),
            max_automatic_retries: 0,
            repair_budget: None,
            endpoint: "https://api.example.com/v1/chat/completions".into(),
            allow_localhost_http: false,
            step_routes: Default::default(),
        })
        .await
        .unwrap();
    let mut approvals = 0;
    let mut completed = false;
    for tick in 0..40 {
        match runtime.worker.tick().await.unwrap() {
            TickOutcome::Advanced { status, run_id } => {
                if status == "waiting_approval" {
                    runtime
                        .director
                        .approve(&format!("p9-approve-{tick}"), &run_id)
                        .await
                        .unwrap();
                    approvals += 1;
                }
                if status == "succeeded" {
                    completed = true;
                    break;
                }
            }
            TickOutcome::Idle => break,
            other => panic!("unexpected outcome: {other:?}"),
        }
    }
    assert!(completed);
    assert!(approvals >= 3);
    let prompts = llm.prompts();
    let draft_prompt = prompts
        .iter()
        .find(|prompt| prompt.starts_with("STEP:chapter_draft"))
        .expect("draft prompt");
    assert!(draft_prompt.contains("山门以剑为尊"));
    assert!(draft_prompt.contains("第1章正式正文"));
    assert!(draft_prompt.contains("第3章正式正文"));
    assert!(draft_prompt.contains("刚入外门"));
    let artifacts = WorkflowRepository::list_artifacts(kernel.as_ref(), &novel.id)
        .await
        .unwrap();
    let context = artifacts
        .iter()
        .find(|artifact| artifact.artifact_type == "context")
        .expect("context artifact");
    let payload: serde_json::Value = serde_json::from_str(&context.payload_json).unwrap();
    assert_eq!(payload["trace"]["assemblyVersion"], "director-context-v1");
    assert!(
        payload["trace"]["sources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|source| source["sourceType"] == "world_facts" && source["selected"] == true)
    );
    let saved = runtime
        .director
        .get_chapter(&novel.id, &target.id)
        .await
        .unwrap();
    assert_eq!(saved.content, "第四章候选正文：林动在祖祠前夜拔剑。");
    assert_eq!(saved.official_version, 1);
    let reopened = SqliteKernel::open(&dir.path().join("p9.sqlite"))
        .await
        .unwrap();
    let reopened_artifacts = WorkflowRepository::list_artifacts(&reopened, &novel.id)
        .await
        .unwrap();
    assert!(
        reopened_artifacts
            .iter()
            .any(|artifact| artifact.artifact_type == "context")
    );
    let _ = opened;
}
