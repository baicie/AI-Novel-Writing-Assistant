//! Planner book 输入必须按 Run 冻结：Run 打开后作者再保存新版本，不能混入该 Run。
//!
//! 覆盖评审 P1：context 之前的 story_macro / chapter_list 更新窗口。

use std::collections::BTreeMap;
use std::sync::Arc;

use novel_application::{
    ModelSecrets, NewArtifact, OpenRunInput, PortError, Runtime, RuntimeDeps, TickOutcome,
    UpdateNovelFields, WorkflowRepository,
};
use novel_storage_sqlite::SqliteKernel;
use novel_testkit::{FakeClock, ScriptedLlm, SequentialIds};
use novel_workflow::book_scope;

struct FixtureSecrets;

#[async_trait::async_trait]
impl ModelSecrets for FixtureSecrets {
    async fn api_key(&self, _provider: &str) -> Result<Option<String>, PortError> {
        Ok(Some("fixture-key".into()))
    }
}

fn chapter_plan(summary: &str) -> String {
    serde_json::json!({
        "chapters": [{
            "chapterOrder": 1,
            "title": "第一章 雪夜",
            "volumeTitle": "山门卷",
            "summary": summary,
            "characterNames": ["林动", "老铁匠"],
            "obligations": [{
                "id": "ch1-req-1",
                "type": "required_event",
                "description": "林动亲眼撞见老铁匠被杀",
                "enforcement": "hard",
                "participants": ["林动", "老铁匠"]
            }]
        }]
    })
    .to_string()
}

fn story_macro(marker: &str) -> String {
    serde_json::json!({
        "mysteryBox": marker,
        "constraints": ["不提前揭露旧案"]
    })
    .to_string()
}

async fn seed_book_artifacts(kernel: &SqliteKernel, novel_id: &str, tag: &str, plan: &str) {
    for (kind, payload) in [
        (
            "story_macro",
            story_macro(&format!("{tag} 山门之下封着什么？")),
        ),
        (
            "character_cast",
            serde_json::json!({"members": [{"name": "林动", "role": "主角"}]}).to_string(),
        ),
        (
            "volume_strategy",
            serde_json::json!({"volumes": [{"volumeOrder": 1, "title": "山门卷"}]}).to_string(),
        ),
        ("chapter_list", plan.to_string()),
    ] {
        WorkflowRepository::insert_external_artifact(
            kernel,
            NewArtifact {
                id: format!("{tag}-{kind}"),
                novel_id: novel_id.to_string(),
                run_id: None,
                artifact_type: kind.to_string(),
                scope: book_scope().to_string(),
                status: "confirmed".into(),
                protected_user_content: false,
                payload_json: payload,
                content_sha256: format!("{tag}-{kind}"),
                created_at: 1,
            },
        )
        .await
        .unwrap();
    }
}

async fn runtime_for(
    llm: Arc<dyn novel_llm::LlmProvider>,
    owner: &str,
) -> (tempfile::TempDir, Arc<SqliteKernel>, Runtime) {
    let dir = tempfile::tempdir().unwrap();
    let kernel = Arc::new(
        SqliteKernel::open(&dir.path().join("freeze.sqlite"))
            .await
            .unwrap(),
    );
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
        secrets: Arc::new(FixtureSecrets),
        owner: owner.into(),
        lease_ms: 60_000,
        concurrency: 1,
    });
    (dir, kernel, runtime)
}

fn planner_prompts() -> BTreeMap<String, String> {
    let mut prompts = BTreeMap::new();
    prompts.insert("story_macro".into(), "story-macro-v2".into());
    prompts.insert("chapter_list".into(), "chapter-list-v3".into());
    prompts.insert("chapter_draft".into(), "chapter-draft-v3".into());
    prompts.insert("chapter_repair".into(), "chapter-repair-v2".into());
    prompts
}

fn open_input(novel_id: &str, chapter_id: &str, command_id: &str) -> OpenRunInput {
    OpenRunInput {
        command_id: command_id.into(),
        novel_id: novel_id.into(),
        target_chapter_id: chapter_id.into(),
        provider: "mock".into(),
        credential_ref: Some("mock".into()),
        model: "mock-model".into(),
        prompt_versions: planner_prompts(),
        token_budget: Some(512),
        context_token_budget: None,
        max_automatic_retries: 0,
        repair_budget: Some(1),
        endpoint: "https://api.example.com/v1/chat/completions".into(),
        allow_localhost_http: false,
        step_routes: Default::default(),
    }
}

async fn drive_until_paused_or_done(runtime: &Runtime, max_ticks: usize) -> String {
    let mut last_status = "queued".to_string();
    for tick in 0..max_ticks {
        match runtime.worker.tick().await.unwrap() {
            TickOutcome::Advanced { status, run_id: id } => {
                last_status = status.clone();
                if status == "waiting_approval" {
                    runtime
                        .director
                        .approve(&format!("p17410-freeze-approve-{tick}"), &id)
                        .await
                        .unwrap();
                }
                if status == "paused" || status == "succeeded" || status == "failed" {
                    break;
                }
            }
            TickOutcome::Idle => break,
            other => panic!("unexpected tick outcome: {other:?}"),
        }
    }
    last_status
}

/// Run 打开后作者保存了新的 story_macro / chapter_list，该 Run 仍必须读取打开时的版本。
#[tokio::test]
async fn planner_book_inputs_stay_frozen_after_author_updates() {
    let llm = Arc::new(ScriptedLlm::new(vec![
        (
            "STEP:chapter_draft",
            "候选正文：雪夜，林动看见老铁匠被杀。".into(),
        ),
        ("STEP:consistency_review", r#"{"issues":[]}"#.to_string()),
    ]));
    let (_dir, kernel, runtime) = runtime_for(llm.clone(), "p17410-freeze").await;
    let novel = runtime.director.create_novel("冻结输入", "").await.unwrap();
    let chapter = runtime
        .director
        .create_chapter(&novel.id, "第一章", 1)
        .await
        .unwrap();
    seed_book_artifacts(
        kernel.as_ref(),
        &novel.id,
        "v1",
        &chapter_plan("V1 计划：林动亲眼撞见老铁匠被灭口"),
    )
    .await;

    let run_id = runtime
        .director
        .open_run(open_input(&novel.id, &chapter.id, "p17410-freeze-open"))
        .await
        .unwrap()
        .run_id;

    // 作者在 Run 打开之后又保存了新版本；该 Run 不得读取。
    seed_book_artifacts(
        kernel.as_ref(),
        &novel.id,
        "v2",
        &chapter_plan("V2 计划：林动只是听见传闻"),
    )
    .await;

    let status = drive_until_paused_or_done(&runtime, 40).await;
    if status != "succeeded" {
        let detail = runtime
            .director
            .get_run_detail(&novel.id, &run_id)
            .await
            .unwrap();
        eprintln!(
            "run {status}: {:?} / {:?}",
            detail.pause_signal, detail.pause_reason
        );
    }
    assert_eq!(status, "succeeded");

    let artifacts = WorkflowRepository::list_artifacts(kernel.as_ref(), &novel.id)
        .await
        .unwrap();
    // 对照：作者更新确实产生了更高的版本，旧实现（取最新）会读到 V2。
    let plans: Vec<_> = artifacts
        .iter()
        .filter(|artifact| artifact.artifact_type == "chapter_list")
        .collect();
    assert!(
        plans
            .iter()
            .any(|artifact| artifact.version == 2 && artifact.payload_json.contains("V2 计划")),
        "the fixture must create a newer plan that the old latest-version path would read"
    );
    let context = artifacts
        .iter()
        .find(|artifact| artifact.artifact_type == "context")
        .expect("context artifact");
    let payload: serde_json::Value = serde_json::from_str(&context.payload_json).unwrap();
    assert_eq!(
        payload["chapterPlanVersion"], 1,
        "the run must keep the plan version frozen at open"
    );
    let prompt_text = payload["promptText"].as_str().unwrap();
    assert!(prompt_text.contains("V1 计划"));
    assert!(
        !prompt_text.contains("V2 计划"),
        "an author update saved after open must not enter the run context"
    );
    assert!(
        prompt_text.contains("v1 山门之下封着什么？"),
        "the story macro must also stay frozen"
    );
    assert!(!prompt_text.contains("v2 山门之下封着什么？"));

    let writer = llm
        .prompts()
        .into_iter()
        .find(|prompt| prompt.contains("STEP:chapter_draft"))
        .expect("writer prompt");
    assert!(writer.contains("V1 计划"));
    assert!(
        !writer.contains("V2 计划"),
        "the writer must not receive a plan saved after the run opened"
    );
}

/// 打开 Run 时存在的来源产物版本会被写入合同（缺一不可的部分由上下文冻结保证）。
#[tokio::test]
async fn run_contract_rejects_a_half_pinned_planner_pair() {
    let llm = Arc::new(ScriptedLlm::new(vec![]));
    let (_dir, _kernel, runtime) = runtime_for(llm, "p17410-pair").await;
    let novel = runtime.director.create_novel("版本成对", "").await.unwrap();
    let chapter = runtime
        .director
        .create_chapter(&novel.id, "第一章", 1)
        .await
        .unwrap();
    let mut input = open_input(&novel.id, &chapter.id, "p17410-pair-open");
    input.prompt_versions.remove("chapter_list");

    let error = runtime.director.open_run(input).await.unwrap_err();
    assert!(
        error.to_string().contains("成对"),
        "story-macro-v2 and chapter-list-v3 must be pinned together: {error}"
    );
}

fn runtime_story_macro(marker: &str) -> String {
    serde_json::json!({
        "expansion": {
            "expanded_premise": marker,
            "protagonist_core": "被逐出家族的少年",
            "conflict_engine": "旧案追杀",
            "conflict_layers": {"external": "山门追杀"},
            "mystery_box": "山门之下封着什么？",
            "emotional_line": "从自保到承担",
            "setpiece_seeds": ["雪夜"],
            "tone_reference": "冷峻"
        },
        "decomposition": {
            "selling_point": "少年卷入旧案",
            "core_conflict": "真相与秩序",
            "main_hook": "玉佩",
            "progression_loop": "发现秘密 -> 被追杀",
            "growth_path": "承担",
            "major_payoffs": ["旧案真相"],
            "ending_flavor": "新生"
        },
        "keyEvents": [{
            "id": "ch1-event-1",
            "chapterOrder": 1,
            "type": "required_event",
            "description": "林动亲眼撞见老铁匠被杀",
            "participants": ["林动", "老铁匠"],
            "enforcement": "hard"
        }],
        "constraints": [],
        "issues": []
    })
    .to_string()
}

fn runtime_cast() -> String {
    serde_json::json!({
        "members": [
            {"name": "林动", "role": "主角", "castRole": "protagonist", "storyFunction": "承担真相"},
            {"name": "苏晚", "role": "同伴", "castRole": "ally", "storyFunction": "协助"},
            {"name": "陆沉", "role": "对手", "castRole": "antagonist", "storyFunction": "施压"},
            {"name": "老铁匠", "role": "受害者", "castRole": "supporting", "storyFunction": "触发旧案"}
        ],
        "relations": [
            {"sourceName": "林动", "targetName": "苏晚", "surfaceRelation": "同门"},
            {"sourceName": "林动", "targetName": "陆沉", "surfaceRelation": "敌对"}
        ]
    })
    .to_string()
}

fn runtime_volume() -> String {
    serde_json::json!({
        "volumes": [{
            "volumeOrder": 1,
            "title": "山门卷",
            "roleLabel": "开局",
            "coreReward": "入山",
            "planningMode": "hard"
        }]
    })
    .to_string()
}

fn runtime_chapter_list() -> String {
    serde_json::json!({
        "chapters": [{
            "chapterOrder": 1,
            "title": "第一章",
            "volumeTitle": "山门卷",
            "summary": "林动亲眼撞见老铁匠被杀",
            "characterNames": ["林动", "老铁匠"],
            "obligations": [{
                "id": "ch1-req-1",
                "type": "required_event",
                "description": "林动亲眼撞见老铁匠被杀",
                "enforcement": "hard",
                "participants": ["林动", "老铁匠"],
                "sourceEventId": "ch1-event-1"
            }]
        }]
    })
    .to_string()
}

#[tokio::test]
async fn story_macro_uses_the_run_snapshot_after_novel_description_changes() {
    let llm = Arc::new(ScriptedLlm::new(vec![
        ("STEP:story_macro", runtime_story_macro("描述 A")),
        ("STEP:character_cast", runtime_cast()),
        ("STEP:volume_strategy", runtime_volume()),
        ("STEP:chapter_list", runtime_chapter_list()),
        ("STEP:chapter_draft", "林动亲眼撞见老铁匠被杀。".into()),
        ("STEP:consistency_review", r#"{"issues":[]}"#.into()),
    ]));
    let (_dir, kernel, runtime) = runtime_for(llm.clone(), "p17410-input-freeze").await;
    let novel = runtime
        .director
        .create_novel("标题 A", "描述 A")
        .await
        .unwrap();
    let chapter = runtime
        .director
        .create_chapter(&novel.id, "第一章", 1)
        .await
        .unwrap();
    let run_a = runtime
        .director
        .open_run(open_input(&novel.id, &chapter.id, "p17410-input-freeze-a"))
        .await
        .unwrap()
        .run_id;

    runtime
        .director
        .update_novel(
            &novel.id,
            &UpdateNovelFields {
                description: Some("描述 B".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    let status = drive_until_paused_or_done(&runtime, 40).await;
    assert_eq!(status, "succeeded");
    let story_prompt = llm
        .prompts()
        .into_iter()
        .find(|prompt| prompt.contains("STEP:story_macro"))
        .expect("story_macro prompt");
    assert!(story_prompt.contains("标题 A\n描述 A"), "{story_prompt}");
    assert!(!story_prompt.contains("描述 B"), "{story_prompt}");

    let contract_a: String =
        sqlx::query_scalar("SELECT contract_json FROM workflow_run WHERE id = ?")
            .bind(&run_a)
            .fetch_one(kernel.as_ref().pool())
            .await
            .unwrap();
    assert!(contract_a.contains("描述 A"));
    assert!(!contract_a.contains("描述 B"));

    let run_b = runtime
        .director
        .open_run(open_input(&novel.id, &chapter.id, "p17410-input-freeze-b"))
        .await
        .unwrap()
        .run_id;
    let contract_b: String =
        sqlx::query_scalar("SELECT contract_json FROM workflow_run WHERE id = ?")
            .bind(&run_b)
            .fetch_one(kernel.as_ref().pool())
            .await
            .unwrap();
    assert!(contract_b.contains("描述 B"));
}
