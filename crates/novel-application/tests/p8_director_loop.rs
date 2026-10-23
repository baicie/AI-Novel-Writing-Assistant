use std::sync::Arc;

use novel_application::{
    NovelRepository, OpenRunInput, Runtime, RuntimeDeps, TickOutcome, WorkflowRepository,
};
use novel_storage_sqlite::SqliteKernel;
use novel_testkit::{FakeClock, ScriptedLlm, SequentialIds};
use sqlx::Row;

fn prompts() -> std::collections::BTreeMap<String, String> {
    let mut prompts = std::collections::BTreeMap::new();
    prompts.insert("chapter_draft".into(), "chapter-draft-v1".into());
    prompts
}

#[tokio::test]
async fn mock_director_loop_reaches_committed_chapter_and_survives_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("novel.sqlite");
    let kernel = Arc::new(SqliteKernel::open(&path).await.unwrap());
    let story_macro = serde_json::json!({
        "expansion": {
            "expanded_premise": "林动被迫进入山门，发现宗门秘密。",
            "protagonist_core": "被逐出家族却拥有未知血脉。",
            "conflict_engine": "每揭开一层秘密，旧秩序就追捕他一次。",
            "conflict_layers": {"external": "宗门追杀", "internal": "身份恐惧", "relational": "师门背叛"},
            "mystery_box": "山门之下封着什么？",
            "emotional_line": "从自保到承担",
            "setpiece_seeds": ["山门夜战", "祖祠认亲"],
            "tone_reference": "冷峻、克制、悬念推进"
        },
        "decomposition": {
            "selling_point": "被逐少年逆袭宗门",
            "core_conflict": "个人真相与宗门秩序",
            "main_hook": "血脉秘密",
            "progression_loop": "发现秘密 -> 被追杀 -> 变强 -> 揭开更大秘密",
            "growth_path": "从逃避到承担",
            "major_payoffs": ["宗门大比", "祖祠真相"],
            "ending_flavor": "代价后的新生"
        },
        "constraints": ["不提前揭露终极秘密"],
        "issues": []
    }).to_string();
    let character_cast = serde_json::json!({
        "members": [
            {"name":"林动","role":"主角","castRole":"protagonist","storyFunction":"承载成长与真相","personality":"坚韧"},
            {"name":"苏晚","role":"同伴","castRole":"ally","storyFunction":"关系压力与协助","personality":"冷静"},
            {"name":"陆沉","role":"对立者","castRole":"antagonist","storyFunction":"持续施压","personality":"偏执"}
        ],
        "relations": [
            {"sourceName":"林动","targetName":"苏晚","surfaceRelation":"同门","conflictSource":"秘密隐瞒"},
            {"sourceName":"林动","targetName":"陆沉","surfaceRelation":"敌对","conflictSource":"宗门继承"}
        ]
    }).to_string();
    let volume_strategy = serde_json::json!({
        "volumes": [
            {"volumeOrder":1,"title":"山门卷","roleLabel":"开局","coreReward":"进入宗门","planningMode":"hard"},
            {"volumeOrder":2,"title":"祖祠卷","roleLabel":"揭示","coreReward":"血脉真相","planningMode":"soft"}
        ]
    }).to_string();
    let chapter_list = serde_json::json!({
        "chapters": [
            {"chapterOrder":1,"title":"第一章 山门","volumeTitle":"山门卷","summary":"林动入山","characterNames":["林动","苏晚"]},
            {"chapterOrder":2,"title":"第二章 夜战","volumeTitle":"山门卷","summary":"夜战","characterNames":["林动","陆沉"]}
        ]
    }).to_string();
    let consistency = serde_json::json!({"issues":[]}).to_string();
    let llm = Arc::new(ScriptedLlm::new(vec![
        ("STEP:story_macro", story_macro),
        ("STEP:character_cast", character_cast),
        ("STEP:volume_strategy", volume_strategy),
        ("STEP:chapter_list", chapter_list),
        ("STEP:consistency_review", consistency),
        ("根据下面的章节目标", "导演流程生成的候选正文".into()),
    ]));
    let runtime = Runtime::new(RuntimeDeps {
        novels: kernel.clone(),
        runs: kernel.clone(),
        worlds: kernel.clone(),
        lookups: kernel.clone(),
        task_center: kernel.clone(),
        llm,
        clock: Arc::new(FakeClock::new(1_000)),
        ids: Arc::new(SequentialIds::new()),
        faults: Arc::new(novel_application::NoFault),
        secrets: Arc::new(novel_application::NoModelSecrets),
        owner: "p8-worker".into(),
        lease_ms: 60_000,
        concurrency: 1,
    });
    let novel = runtime.director.create_novel("导演闭环", "").await.unwrap();
    let chapter = runtime
        .director
        .create_chapter(&novel.id, "第一章", 1)
        .await
        .unwrap();
    let opened = runtime
        .director
        .open_run(OpenRunInput {
            command_id: "p8-open".into(),
            novel_id: novel.id.clone(),
            target_chapter_id: chapter.id.clone(),
            provider: "mock".into(),
            model: "mock-model".into(),
            prompt_versions: prompts(),
            token_budget: Some(512),
            max_automatic_retries: 0,
            endpoint: "https://api.example.com/v1/chat/completions".into(),
            allow_localhost_http: false,
        })
        .await
        .unwrap();
    assert_eq!(opened.status, "queued");

    let mut approvals = 0;
    let mut completed = false;
    for tick in 0..40 {
        match runtime.worker.tick().await.unwrap() {
            TickOutcome::Advanced { status, run_id } => {
                if status == "waiting_approval" {
                    runtime
                        .director
                        .approve(&format!("p8-approve-{tick}"), &run_id)
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
            TickOutcome::Interrupted { .. } => panic!("mock loop must not be interrupted"),
            TickOutcome::LostLease { .. } | TickOutcome::Saturated | TickOutcome::Stopped => {
                panic!("unexpected worker outcome")
            }
        }
    }
    assert!(completed, "director run must reach succeeded");
    assert!(
        approvals >= 3,
        "book and chapter gates must require approval"
    );

    let chapter_after = runtime
        .director
        .get_chapter(&novel.id, &chapter.id)
        .await
        .unwrap();
    assert_eq!(chapter_after.content, "导演流程生成的候选正文");
    assert_eq!(chapter_after.official_version, 1);
    assert_eq!(chapter_after.protection, "none");
    let artifacts = WorkflowRepository::list_artifacts(kernel.as_ref(), &novel.id)
        .await
        .unwrap();
    let story = artifacts
        .iter()
        .find(|artifact| artifact.artifact_type == "story_macro")
        .expect("story macro artifact");
    let story_payload: serde_json::Value = serde_json::from_str(&story.payload_json).unwrap();
    assert!(
        story_payload["expansion"]["expanded_premise"]
            .as_str()
            .is_some()
    );
    assert!(
        story_payload["decomposition"]["core_conflict"]
            .as_str()
            .is_some()
    );
    let cast = artifacts
        .iter()
        .find(|artifact| artifact.artifact_type == "character_cast")
        .expect("character cast artifact");
    let cast_payload: serde_json::Value = serde_json::from_str(&cast.payload_json).unwrap();
    assert_eq!(cast_payload["members"].as_array().unwrap().len(), 3);
    assert_eq!(cast_payload["relations"].as_array().unwrap().len(), 2);
    assert_eq!(cast.status, novel_domain::ArtifactStatus::Confirmed);
    let status: String = sqlx::query("SELECT status FROM workflow_control WHERE run_id = ?")
        .bind(&opened.run_id)
        .fetch_one(kernel.pool())
        .await
        .unwrap()
        .try_get("status")
        .unwrap();
    assert_eq!(status, "succeeded");

    let reopened = SqliteKernel::open(&path).await.unwrap();
    let reopened_chapter = NovelRepository::get_chapter(&reopened, &novel.id, &chapter.id)
        .await
        .unwrap();
    assert_eq!(reopened_chapter.content, "导演流程生成的候选正文");
    let revisions: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM chapter_revision WHERE chapter_id = ?")
            .bind(&chapter.id)
            .fetch_one(reopened.pool())
            .await
            .unwrap();
    assert_eq!(revisions, 1);
}
