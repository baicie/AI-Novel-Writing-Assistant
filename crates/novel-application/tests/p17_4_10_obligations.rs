use std::collections::BTreeMap;
use std::sync::Arc;

use novel_application::{
    ModelSecrets, NewChapter, NewNovel, NovelRepository, OpenRunInput, PortError, Runtime,
    RuntimeDeps, TickOutcome, WorkflowRepository, assemble_director_context,
    preview_director_context,
};
use novel_domain::{ArtifactStatus, ArtifactView};
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

fn artifact(ty: &str, version: i64, scope: &str, payload: serde_json::Value) -> ArtifactView {
    ArtifactView {
        artifact_type: ty.into(),
        scope: scope.into(),
        version,
        run_id: None,
        status: ArtifactStatus::Confirmed,
        protected_user_content: false,
        payload_json: payload.to_string(),
    }
}

fn story() -> serde_json::Value {
    serde_json::json!({
        "expansion": {
            "expanded_premise": "林动在雪夜发现玉佩，被卷入山门旧案。",
            "protagonist_core": "被逐出家族的少年",
            "conflict_engine": "每揭开一层旧案就多一个追杀者",
            "conflict_layers": {"external": "山门追杀"},
            "mystery_box": "山门之下封着什么？",
            "emotional_line": "从自保到承担",
            "setpiece_seeds": ["雪夜玉佩"],
            "tone_reference": "冷峻"
        },
        "decomposition": {
            "selling_point": "少年卷入山门旧案",
            "core_conflict": "真相与秩序",
            "main_hook": "玉佩",
            "progression_loop": "发现秘密 -> 被追杀",
            "growth_path": "承担",
            "major_payoffs": ["旧案真相"],
            "ending_flavor": "代价后的新生"
        },
        "constraints": ["不提前揭露旧案"],
        "issues": []
    })
}

fn cast() -> serde_json::Value {
    serde_json::json!({
        "members": [
            {"name":"林动","role":"主角","castRole":"protagonist","storyFunction":"承担真相"},
            {"name":"苏晚","role":"同伴","castRole":"ally","storyFunction":"协助"},
            {"name":"陆沉","role":"对手","castRole":"antagonist","storyFunction":"施压"},
            {"name":"老铁匠","role":"受害者","castRole":"supporting","storyFunction":"触发旧案"}
        ],
        "relations": [
            {"sourceName":"林动","targetName":"苏晚","surfaceRelation":"同门","conflictSource":"秘密隐瞒"},
            {"sourceName":"林动","targetName":"陆沉","surfaceRelation":"敌对","conflictSource":"旧案"}
        ]
    })
}

fn volume() -> serde_json::Value {
    serde_json::json!({"volumes":[{"volumeOrder":1,"title":"山门卷","roleLabel":"开局","coreReward":"入山","planningMode":"hard"}]})
}

fn chapter_plan(with_obligations: bool) -> serde_json::Value {
    let chapter_one = if with_obligations {
        serde_json::json!({
            "chapterOrder":1,
            "title":"第一章 雪夜",
            "volumeTitle":"山门卷",
            "summary":"林动亲眼撞见老铁匠被灭口",
            "characterNames":["林动","老铁匠"],
            "obligations":[
                {
                    "id":"ch1-req-1",
                    "type":"required_event",
                    "description":"林动亲眼撞见老铁匠被杀",
                    "enforcement":"hard",
                    "participants":["林动","老铁匠"],
                    "evidence":"第1章计划"
                },
                {
                    "id":"ch1-order-1",
                    "type":"ordering_constraint",
                    "description":"先听见异响，再撞见灭口",
                    "enforcement":"hard",
                    "participants":["林动"]
                }
            ]
        })
    } else {
        serde_json::json!({
            "chapterOrder":1,
            "title":"第一章 雪夜",
            "volumeTitle":"山门卷",
            "summary":"林动亲眼撞见老铁匠被灭口",
            "characterNames":["林动","老铁匠"]
        })
    };
    serde_json::json!({
        "chapters": [
            chapter_one,
            {
                "chapterOrder":2,
                "title":"第二章 追查",
                "volumeTitle":"山门卷",
                "summary":"林动追查旧案",
                "characterNames":["林动"],
                "obligations":[
                    {
                        "id":"ch2-req-1",
                        "type":"required_event",
                        "description":"第二章才揭露旧案线索",
                        "enforcement":"hard",
                        "participants":["林动"]
                    }
                ]
            }
        ]
    })
}

fn artifacts(plan: serde_json::Value) -> Vec<ArtifactView> {
    vec![
        artifact("story_macro", 1, book_scope(), story()),
        artifact("character_cast", 1, book_scope(), cast()),
        artifact("volume_strategy", 1, book_scope(), volume()),
        artifact("chapter_list", 1, book_scope(), plan),
    ]
}

async fn context_fixture() -> (tempfile::TempDir, Arc<SqliteKernel>, String, String, String) {
    let dir = tempfile::tempdir().unwrap();
    let kernel = Arc::new(
        SqliteKernel::open(&dir.path().join("obligations.sqlite"))
            .await
            .unwrap(),
    );
    NovelRepository::create_novel(
        kernel.as_ref(),
        NewNovel {
            id: "novel-a".into(),
            title: "山门旧案".into(),
            description: "雪夜玉佩与山门旧案。".into(),
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    for (id, title, order) in [("ch1", "第一章", 1), ("ch2", "第二章", 2)] {
        NovelRepository::create_chapter(
            kernel.as_ref(),
            NewChapter {
                id: id.into(),
                novel_id: "novel-a".into(),
                title: title.into(),
                sort_order: order,
                volume_id: None,
                now_ms: order as i64,
            },
        )
        .await
        .unwrap();
    }
    (dir, kernel, "novel-a".into(), "ch1".into(), "ch2".into())
}

#[tokio::test]
async fn target_obligations_reach_context_and_future_chapter_ids_do_not_leak() {
    let (_dir, kernel, novel, ch1, _ch2) = context_fixture().await;
    let assembly = assemble_director_context(
        kernel.as_ref(),
        &artifacts(chapter_plan(true)),
        &novel,
        &ch1,
        24_000,
        8_000,
    )
    .await
    .unwrap();
    assert_eq!(assembly.chapter_plan_version, Some(1));
    assert!(assembly.obligation_ids.iter().any(|id| id == "ch1-req-1"));
    assert!(assembly.obligations_text.contains("林动亲眼撞见老铁匠被杀"));
    assert!(assembly.prompt_text.contains("## chapter_obligations"));
    assert!(assembly.prompt_text.contains("ch1-req-1"));
    assert!(
        !assembly.prompt_text.contains("ch2-req-1"),
        "future chapter obligations must not enter the current context"
    );
    assert!(
        assembly
            .trace
            .sources
            .iter()
            .any(|source| { source.source_type == "chapter_obligations" && source.selected })
    );
}

#[tokio::test]
async fn missing_target_plan_fails_closed_and_preview_reports_reason() {
    let (_dir, kernel, novel, _ch1, ch2) = context_fixture().await;
    let plan = serde_json::json!({
        "chapters": [{
            "chapterOrder": 1,
            "title": "第一章",
            "volumeTitle": "山门卷",
            "summary": "林动入山",
            "characterNames": ["林动"]
        }]
    });
    let error = assemble_director_context(
        kernel.as_ref(),
        &artifacts(plan.clone()),
        &novel,
        &ch2,
        24_000,
        8_000,
    )
    .await
    .unwrap_err();
    assert!(
        error.to_string().contains("目标章节 2"),
        "missing target plan must fail explicitly: {error}"
    );
    let preview = preview_director_context(
        kernel.as_ref(),
        &artifacts(plan),
        &novel,
        &ch2,
        24_000,
        8_000,
    )
    .await
    .unwrap();
    assert!(
        preview
            .blocked_reasons
            .iter()
            .any(|reason| reason.contains("目标章节 2")),
        "preview must expose the blocked reason: {:?}",
        preview.blocked_reasons
    );
}

#[tokio::test]
async fn oversized_obligations_fail_instead_of_silent_truncation() {
    let (_dir, kernel, novel, ch1, _ch2) = context_fixture().await;
    let mut plan = chapter_plan(true);
    let obligations = plan["chapters"][0]["obligations"]
        .as_array_mut()
        .expect("obligations");
    for index in 0..80 {
        obligations.push(serde_json::json!({
            "id": format!("ch1-bulk-{index}"),
            "type": "required_event",
            "description": "必须保留的长义务".repeat(20),
            "enforcement": "hard",
            "participants": ["林动"]
        }));
    }
    let error =
        assemble_director_context(kernel.as_ref(), &artifacts(plan), &novel, &ch1, 512, 8_000)
            .await
            .unwrap_err();
    assert!(
        error.to_string().contains("上下文预算不足"),
        "mandatory obligations must not be silently dropped: {error}"
    );
}

#[tokio::test]
async fn old_plan_without_obligations_remains_compatible() {
    let (_dir, kernel, novel, ch1, _ch2) = context_fixture().await;
    let assembly = assemble_director_context(
        kernel.as_ref(),
        &artifacts(chapter_plan(false)),
        &novel,
        &ch1,
        24_000,
        8_000,
    )
    .await
    .unwrap();
    assert!(assembly.obligation_ids.is_empty());
    assert!(assembly.obligations_text.is_empty());
    assert!(
        !assembly
            .trace
            .sources
            .iter()
            .any(|source| source.source_type == "chapter_obligations"),
        "old plans must remain readable without inventing obligations"
    );
    assert!(assembly.prompt_text.contains("第一章 雪夜"));
}

#[test]
fn chapter_list_v2_and_repair_v2_templates_carry_the_contract() {
    let list = novel_prompt::resolve("chapter_list", "chapter-list-v2").unwrap();
    assert!(list.template.contains("obligations"));
    assert!(list.template.contains("story_macro"));
    assert!(list.template.contains("不得引用未来章节"));

    let repair = novel_prompt::resolve("chapter_repair", "chapter-repair-v2").unwrap();
    let mut vars = BTreeMap::new();
    vars.insert(
        "obligations".into(),
        "[ch1-req-1] 林动亲眼撞见老铁匠被杀".into(),
    );
    vars.insert("context".into(), "上下文".into());
    vars.insert("issues".into(), "[]".into());
    vars.insert("candidate".into(), "候选正文".into());
    let rendered = novel_prompt::render(repair.template, &vars).unwrap();
    assert!(rendered.contains("ch1-req-1"));
    assert!(rendered.contains("不得改写"));
}

// ---------------------------------------------------------------------------
// Worker integration: scripted LLM, no real model calls.
// ---------------------------------------------------------------------------

fn runtime_prompts() -> BTreeMap<String, String> {
    let mut prompts = BTreeMap::new();
    prompts.insert("chapter_draft".into(), "chapter-draft-v3".into());
    prompts.insert("chapter_repair".into(), "chapter-repair-v2".into());
    prompts
}

async fn runtime_for(
    llm: Arc<dyn novel_llm::LlmProvider>,
    owner: &str,
) -> (tempfile::TempDir, Arc<SqliteKernel>, Runtime) {
    let dir = tempfile::tempdir().unwrap();
    let kernel = Arc::new(
        SqliteKernel::open(&dir.path().join("worker.sqlite"))
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

async fn open_run(runtime: &Runtime, novel_id: &str, chapter_id: &str, command_id: &str) -> String {
    runtime
        .director
        .open_run(OpenRunInput {
            command_id: command_id.into(),
            novel_id: novel_id.into(),
            target_chapter_id: chapter_id.into(),
            provider: "mock".into(),
            credential_ref: Some("mock".into()),
            model: "mock-model".into(),
            prompt_versions: runtime_prompts(),
            token_budget: Some(512),
            context_token_budget: None,
            max_automatic_retries: 0,
            repair_budget: Some(1),
            endpoint: "https://api.example.com/v1/chat/completions".into(),
            allow_localhost_http: false,
            step_routes: Default::default(),
        })
        .await
        .unwrap()
        .run_id
}

fn story_macro() -> String {
    serde_json::json!({
        "expansion": {
            "expanded_premise": "林动在雪夜发现玉佩，被卷入山门旧案。",
            "protagonist_core": "被逐出家族的少年",
            "conflict_engine": "每揭开一层旧案就多一个追杀者",
            "conflict_layers": {"external": "山门追杀"},
            "mystery_box": "山门之下封着什么？",
            "emotional_line": "从自保到承担",
            "setpiece_seeds": ["雪夜玉佩"],
            "tone_reference": "冷峻"
        },
        "decomposition": {
            "selling_point": "少年卷入山门旧案",
            "core_conflict": "真相与秩序",
            "main_hook": "玉佩",
            "progression_loop": "发现秘密 -> 被追杀",
            "growth_path": "承担",
            "major_payoffs": ["旧案真相"],
            "ending_flavor": "代价后的新生"
        },
        "constraints": ["不提前揭露旧案"],
        "issues": []
    })
    .to_string()
}

fn scripted_cast() -> String {
    serde_json::json!({
        "members": [
            {"name":"林动","role":"主角","castRole":"protagonist","storyFunction":"承担真相"},
            {"name":"苏晚","role":"同伴","castRole":"ally","storyFunction":"协助"},
            {"name":"陆沉","role":"对手","castRole":"antagonist","storyFunction":"施压"},
            {"name":"老铁匠","role":"受害者","castRole":"supporting","storyFunction":"触发旧案"}
        ],
        "relations": [
            {"sourceName":"林动","targetName":"苏晚","surfaceRelation":"同门","conflictSource":"秘密隐瞒"},
            {"sourceName":"林动","targetName":"陆沉","surfaceRelation":"敌对","conflictSource":"旧案"}
        ]
    })
    .to_string()
}

fn scripted_volume() -> String {
    serde_json::json!({"volumes":[{"volumeOrder":1,"title":"山门卷","roleLabel":"开局","coreReward":"入山","planningMode":"hard"}]})
        .to_string()
}

fn chapter_list() -> String {
    serde_json::json!({
        "chapters": [
            {
                "chapterOrder":1,
                "title":"第一章 雪夜",
                "volumeTitle":"山门卷",
                "summary":"林动亲眼撞见老铁匠被灭口",
                "characterNames":["林动","老铁匠"],
                "obligations":[
                    {
                        "id":"ch1-req-1",
                        "type":"required_event",
                        "description":"林动亲眼撞见老铁匠被杀",
                        "enforcement":"hard",
                        "participants":["林动","老铁匠"],
                        "evidence":"第1章计划"
                    },
                    {
                        "id":"ch1-order-1",
                        "type":"ordering_constraint",
                        "description":"先听见异响，再撞见灭口",
                        "enforcement":"hard",
                        "participants":["林动"]
                    },
                    {
                        "id":"ch1-adv-1",
                        "type":"continuity_constraint",
                        "description":"可以补一句雪夜环境描写",
                        "enforcement":"advisory",
                        "participants":["林动"]
                    }
                ]
            },
            {
                "chapterOrder":2,
                "title":"第二章 追查",
                "volumeTitle":"山门卷",
                "summary":"林动追查旧案",
                "characterNames":["林动"],
                "obligations":[
                    {"id":"ch2-req-1","type":"required_event","description":"第二章才揭露旧案线索","enforcement":"hard","participants":["林动"]}
                ]
            }
        ]
    })
    .to_string()
}

fn chapter_list_with_unknown_character() -> String {
    let mut plan: serde_json::Value = serde_json::from_str(&chapter_list()).unwrap();
    plan["chapters"][0]["characterNames"] = serde_json::json!(["未知角色"]);
    plan.to_string()
}

fn chapter_list_without_obligations() -> String {
    let mut plan: serde_json::Value = serde_json::from_str(&chapter_list()).unwrap();
    for chapter in plan["chapters"].as_array_mut().expect("chapters") {
        chapter
            .as_object_mut()
            .expect("chapter")
            .remove("obligations");
    }
    plan.to_string()
}

fn chapter_list_with_empty_obligations() -> String {
    let mut plan: serde_json::Value = serde_json::from_str(&chapter_list()).unwrap();
    for chapter in plan["chapters"].as_array_mut().expect("chapters") {
        chapter["obligations"] = serde_json::json!([]);
    }
    plan.to_string()
}

fn clean_review() -> String {
    r#"{"issues":[]}"#.to_string()
}

fn known_issue_review() -> String {
    r#"{"issues":[{"type":"timeline","severity":"high","evidence":"林动赶到时，老铁匠已经死亡","suggestion":"改为主角亲眼撞见灭口","obligationId":"ch1-req-1"}]}"#.to_string()
}

fn advisory_only_review() -> String {
    r#"{"issues":[{"type":"style","severity":"low","evidence":"没有补雪夜环境描写","suggestion":"可以补一句","obligationId":"ch1-adv-1","needsAuthorConfirmation":true}]}"#.to_string()
}

fn invented_evidence_review() -> String {
    r#"{"issues":[{"type":"timeline","severity":"high","evidence":"林动亲手杀死了老铁匠","suggestion":"改为主角亲眼撞见灭口","obligationId":"ch1-req-1"}]}"#.to_string()
}

fn unknown_issue_review() -> String {
    r#"{"issues":[{"type":"timeline","severity":"high","evidence":"主角到达时老铁匠已经死亡","suggestion":"改为主角亲眼撞见灭口","obligationId":"missing-id"}]}"#.to_string()
}

async fn drive_until_paused_or_done(
    runtime: &Runtime,
    novel_id: &str,
    run_id: &str,
    max_ticks: usize,
) -> (String, Option<String>) {
    let mut last_status = "queued".to_string();
    for tick in 0..max_ticks {
        match runtime.worker.tick().await.unwrap() {
            TickOutcome::Advanced { status, run_id: id } => {
                last_status = status.clone();
                if status == "waiting_approval" {
                    runtime
                        .director
                        .approve(&format!("p17410-approve-{tick}"), &id)
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
    let detail = runtime
        .director
        .get_run_detail(novel_id, run_id)
        .await
        .unwrap();
    (last_status, detail.pause_signal)
}

#[tokio::test]
async fn planner_writer_and_reviewer_receive_the_obligation_contract() {
    let llm = Arc::new(ScriptedLlm::new(vec![
        ("STEP:story_macro", story_macro()),
        ("STEP:character_cast", scripted_cast()),
        ("STEP:volume_strategy", scripted_volume()),
        ("STEP:chapter_list", chapter_list()),
        (
            "STEP:chapter_draft",
            "候选正文：雪夜，林动看见老铁匠被杀。".into(),
        ),
        ("STEP:consistency_review", clean_review()),
    ]));
    let (_dir, kernel, runtime) = runtime_for(llm.clone(), "p17410-transfer").await;
    let novel = runtime.director.create_novel("义务传递", "").await.unwrap();
    let chapter = runtime
        .director
        .create_chapter(&novel.id, "第一章", 1)
        .await
        .unwrap();
    let run_id = open_run(&runtime, &novel.id, &chapter.id, "p17410-transfer-open").await;
    let (_status, _signal) = drive_until_paused_or_done(&runtime, &novel.id, &run_id, 40).await;
    let prompts = llm.prompts();

    let planner = prompts
        .iter()
        .find(|prompt| prompt.contains("STEP:chapter_list"))
        .expect("chapter_list prompt");
    assert!(
        planner.contains("story_macro"),
        "planner must read the macro"
    );
    assert!(planner.contains("山门之下封着什么？"));

    let writer = prompts
        .iter()
        .find(|prompt| prompt.contains("STEP:chapter_draft"))
        .expect("draft prompt");
    assert!(writer.contains("ch1-req-1"));
    assert!(writer.contains("林动亲眼撞见老铁匠被杀"));
    assert!(writer.contains("【硬约束｜必须遵守"));
    assert!(writer.contains("【创作建议｜可合理取舍"));

    let reviewer = prompts
        .iter()
        .find(|prompt| prompt.contains("STEP:consistency_review"))
        .expect("review prompt");
    assert!(reviewer.contains("ch1-req-1"));
    assert!(!reviewer.contains("ch2-req-1"));

    let artifacts = WorkflowRepository::list_artifacts(kernel.as_ref(), &novel.id)
        .await
        .unwrap();
    let context = artifacts
        .iter()
        .find(|artifact| artifact.artifact_type == "context")
        .expect("context artifact");
    let payload: serde_json::Value = serde_json::from_str(&context.payload_json).unwrap();
    assert_eq!(payload["chapterPlanVersion"], 1);
    assert!(
        payload["obligationIds"]
            .as_array()
            .is_some_and(|ids| ids.iter().any(|id| id == "ch1-req-1"))
    );
    assert!(
        payload["obligationsText"]
            .as_str()
            .is_some_and(|text| text.contains("林动亲眼撞见老铁匠被杀"))
    );
}

#[tokio::test]
async fn unknown_review_obligation_id_pauses_without_repair() {
    let llm = Arc::new(ScriptedLlm::new(vec![
        ("STEP:story_macro", story_macro()),
        ("STEP:character_cast", scripted_cast()),
        ("STEP:volume_strategy", scripted_volume()),
        ("STEP:chapter_list", chapter_list()),
        ("STEP:chapter_draft", "候选正文".into()),
        ("STEP:consistency_review", unknown_issue_review()),
    ]));
    let (_dir, _kernel, runtime) = runtime_for(llm.clone(), "p17410-unknown-id").await;
    let novel = runtime.director.create_novel("未知义务", "").await.unwrap();
    let chapter = runtime
        .director
        .create_chapter(&novel.id, "第一章", 1)
        .await
        .unwrap();
    let run_id = open_run(&runtime, &novel.id, &chapter.id, "p17410-unknown-open").await;
    let (status, signal) = drive_until_paused_or_done(&runtime, &novel.id, &run_id, 40).await;
    assert_eq!(status, "paused");
    assert_eq!(signal.as_deref(), Some("data_integrity"));
    let prompts = llm.prompts();
    assert_eq!(
        prompts
            .iter()
            .filter(|prompt| prompt.contains("STEP:consistency_review"))
            .count(),
        1,
        "unknown obligation ids must be rejected before any review repair"
    );
    assert!(
        !prompts
            .iter()
            .any(|prompt| prompt.contains("STEP:chapter_repair")),
        "review JSON must not be repaired"
    );
}

#[tokio::test]
async fn invalid_review_json_pauses_without_becoming_zero_issues() {
    let llm = Arc::new(ScriptedLlm::new(vec![
        ("STEP:story_macro", story_macro()),
        ("STEP:character_cast", scripted_cast()),
        ("STEP:volume_strategy", scripted_volume()),
        ("STEP:chapter_list", chapter_list()),
        ("STEP:chapter_draft", "候选正文".into()),
        ("STEP:consistency_review", "不是 JSON".into()),
    ]));
    let (_dir, _kernel, runtime) = runtime_for(llm.clone(), "p17410-invalid-review").await;
    let novel = runtime.director.create_novel("无效审校", "").await.unwrap();
    let chapter = runtime
        .director
        .create_chapter(&novel.id, "第一章", 1)
        .await
        .unwrap();
    let run_id = open_run(&runtime, &novel.id, &chapter.id, "p17410-invalid-open").await;
    let (status, signal) = drive_until_paused_or_done(&runtime, &novel.id, &run_id, 40).await;
    assert_eq!(status, "paused");
    assert_eq!(signal.as_deref(), Some("data_integrity"));
    let prompts = llm.prompts();
    assert_eq!(
        prompts
            .iter()
            .filter(|prompt| prompt.contains("STEP:consistency_review"))
            .count(),
        1
    );
    assert!(
        !prompts
            .iter()
            .any(|prompt| prompt.contains("STEP:chapter_repair")),
        "invalid review JSON must fail closed, not become zero issues"
    );
}

#[tokio::test]
async fn chapter_plan_reference_error_pauses_instead_of_worker_error() {
    let llm = Arc::new(ScriptedLlm::new(vec![
        ("STEP:story_macro", story_macro()),
        ("STEP:character_cast", scripted_cast()),
        ("STEP:volume_strategy", scripted_volume()),
        ("STEP:chapter_list", chapter_list_with_unknown_character()),
        ("STEP:chapter_draft", "不应到达的正文".into()),
        ("STEP:consistency_review", clean_review()),
    ]));
    let (_dir, _kernel, runtime) = runtime_for(llm.clone(), "p17410-bad-ref").await;
    let novel = runtime.director.create_novel("坏引用", "").await.unwrap();
    let chapter = runtime
        .director
        .create_chapter(&novel.id, "第一章", 1)
        .await
        .unwrap();
    let run_id = open_run(&runtime, &novel.id, &chapter.id, "p17410-bad-ref-open").await;
    let (status, signal) = drive_until_paused_or_done(&runtime, &novel.id, &run_id, 40).await;
    assert_eq!(status, "paused");
    assert_eq!(signal.as_deref(), Some("data_integrity"));
    assert!(
        !llm.prompts()
            .iter()
            .any(|prompt| prompt.contains("STEP:chapter_draft")),
        "an invalid chapter plan must not reach the writer"
    );
}

#[tokio::test]
async fn repair_prompt_carries_obligation_ids() {
    let llm = Arc::new(ScriptedLlm::new(vec![
        ("STEP:story_macro", story_macro()),
        ("STEP:character_cast", scripted_cast()),
        ("STEP:volume_strategy", scripted_volume()),
        ("STEP:chapter_list", chapter_list()),
        ("STEP:chapter_draft", "林动赶到时，老铁匠已经死亡。".into()),
        ("STEP:consistency_review", known_issue_review()),
        ("STEP:chapter_repair", "修复后的候选正文".into()),
        ("STEP:consistency_review", clean_review()),
    ]));
    let (_dir, kernel, runtime) = runtime_for(llm.clone(), "p17410-repair").await;
    let novel = runtime.director.create_novel("修复义务", "").await.unwrap();
    let chapter = runtime
        .director
        .create_chapter(&novel.id, "第一章", 1)
        .await
        .unwrap();
    let _run_id = open_run(&runtime, &novel.id, &chapter.id, "p17410-repair-open").await;
    let mut seen_repair = false;
    for tick in 0..40 {
        match runtime.worker.tick().await.unwrap() {
            TickOutcome::Advanced { status, run_id: id } => {
                if status == "waiting_approval" {
                    runtime
                        .director
                        .approve(&format!("p17410-repair-approve-{tick}"), &id)
                        .await
                        .unwrap();
                }
                if llm
                    .prompts()
                    .iter()
                    .any(|prompt| prompt.contains("STEP:chapter_repair"))
                {
                    seen_repair = true;
                    break;
                }
                if status == "paused" || status == "succeeded" || status == "failed" {
                    break;
                }
            }
            TickOutcome::Idle => break,
            other => panic!("unexpected tick outcome: {other:?}"),
        }
    }
    assert!(seen_repair, "repair prompt must be sent");
    let prompts = llm.prompts();
    let repair = prompts
        .iter()
        .find(|prompt| prompt.contains("STEP:chapter_repair"))
        .expect("repair prompt");
    assert!(repair.contains("ch1-req-1"));
    assert!(repair.contains("本章章节义务"));
    assert!(repair.contains("硬约束必须保留"));
    assert!(repair.contains("创作建议可以合理取舍"));
    assert!(repair.contains("【硬约束｜必须遵守"));
    assert!(repair.contains("【创作建议｜可合理取舍"));

    let artifacts = WorkflowRepository::list_artifacts(kernel.as_ref(), &novel.id)
        .await
        .unwrap();
    let review = artifacts
        .iter()
        .filter(|artifact| artifact.artifact_type == "consistency_review")
        .find(|artifact| {
            serde_json::from_str::<serde_json::Value>(&artifact.payload_json)
                .ok()
                .and_then(|payload| {
                    payload["issueDetails"][0]["obligationId"]
                        .as_str()
                        .map(|id| id == "ch1-req-1")
                })
                .unwrap_or(false)
        })
        .expect("review artifact with obligation id");
    let payload: serde_json::Value = serde_json::from_str(&review.payload_json).unwrap();
    let issue = &payload["issueDetails"][0];
    assert_eq!(issue["obligationId"], "ch1-req-1");
    assert_eq!(issue["obligationEnforcement"], "hard");
    assert!(
        issue["evidence"]
            .as_str()
            .is_some_and(|evidence| evidence.contains("老铁匠")),
        "reviewer evidence must be preserved: {issue}"
    );
}

#[tokio::test]
async fn chapter_list_without_obligations_pauses_before_writing() {
    let llm = Arc::new(ScriptedLlm::new(vec![
        ("STEP:story_macro", story_macro()),
        ("STEP:character_cast", scripted_cast()),
        ("STEP:volume_strategy", scripted_volume()),
        ("STEP:chapter_list", chapter_list_without_obligations()),
        ("STEP:chapter_list", chapter_list_without_obligations()),
        ("STEP:chapter_draft", "不应到达的正文".into()),
    ]));
    let (_dir, _kernel, runtime) = runtime_for(llm.clone(), "p17410-missing-obligations").await;
    let novel = runtime.director.create_novel("缺少义务", "").await.unwrap();
    let chapter = runtime
        .director
        .create_chapter(&novel.id, "第一章", 1)
        .await
        .unwrap();
    let run_id = open_run(&runtime, &novel.id, &chapter.id, "p17410-missing-open").await;
    let (status, signal) = drive_until_paused_or_done(&runtime, &novel.id, &run_id, 40).await;
    assert_eq!(status, "paused");
    assert_eq!(signal.as_deref(), Some("data_integrity"));
    let prompts = llm.prompts();
    assert_eq!(
        prompts
            .iter()
            .filter(|prompt| prompt.contains("STEP:chapter_list"))
            .count(),
        2,
        "missing obligations must be rejected once and fail after the bounded repair"
    );
    assert!(
        !prompts
            .iter()
            .any(|prompt| prompt.contains("STEP:chapter_draft")),
        "an invalid chapter plan must never reach the writer"
    );
}

#[tokio::test]
async fn empty_or_advisory_only_obligations_are_rejected() {
    let llm = Arc::new(ScriptedLlm::new(vec![
        ("STEP:story_macro", story_macro()),
        ("STEP:character_cast", scripted_cast()),
        ("STEP:volume_strategy", scripted_volume()),
        ("STEP:chapter_list", chapter_list_with_empty_obligations()),
        ("STEP:chapter_list", chapter_list_with_empty_obligations()),
        ("STEP:chapter_draft", "不应到达的正文".into()),
    ]));
    let (_dir, _kernel, runtime) = runtime_for(llm.clone(), "p17410-empty-obligations").await;
    let novel = runtime.director.create_novel("空义务", "").await.unwrap();
    let chapter = runtime
        .director
        .create_chapter(&novel.id, "第一章", 1)
        .await
        .unwrap();
    let run_id = open_run(&runtime, &novel.id, &chapter.id, "p17410-empty-open").await;
    let (status, signal) = drive_until_paused_or_done(&runtime, &novel.id, &run_id, 40).await;
    assert_eq!(status, "paused");
    assert_eq!(signal.as_deref(), Some("data_integrity"));
    assert!(
        !llm.prompts()
            .iter()
            .any(|prompt| prompt.contains("STEP:chapter_draft")),
        "an empty obligation array is not a valid v2 plan"
    );
}

#[tokio::test]
async fn advisory_only_review_does_not_trigger_repair_or_block_completion() {
    let llm = Arc::new(ScriptedLlm::new(vec![
        ("STEP:story_macro", story_macro()),
        ("STEP:character_cast", scripted_cast()),
        ("STEP:volume_strategy", scripted_volume()),
        ("STEP:chapter_list", chapter_list()),
        ("STEP:chapter_draft", "林动赶到时，老铁匠已经死亡。".into()),
        ("STEP:consistency_review", advisory_only_review()),
    ]));
    let (_dir, kernel, runtime) = runtime_for(llm.clone(), "p17410-advisory").await;
    let novel = runtime.director.create_novel("建议义务", "").await.unwrap();
    let chapter = runtime
        .director
        .create_chapter(&novel.id, "第一章", 1)
        .await
        .unwrap();
    let run_id = open_run(&runtime, &novel.id, &chapter.id, "p17410-advisory-open").await;
    let (status, _signal) = drive_until_paused_or_done(&runtime, &novel.id, &run_id, 40).await;
    assert_eq!(status, "succeeded");
    assert!(
        !llm.prompts()
            .iter()
            .any(|prompt| prompt.contains("STEP:chapter_repair")),
        "an unimplemented advisory must not trigger a repair call"
    );
    let artifacts = WorkflowRepository::list_artifacts(kernel.as_ref(), &novel.id)
        .await
        .unwrap();
    let review = artifacts
        .iter()
        .filter(|artifact| artifact.artifact_type == "consistency_review")
        .max_by_key(|artifact| artifact.version)
        .expect("review artifact");
    let payload: serde_json::Value = serde_json::from_str(&review.payload_json).unwrap();
    assert_eq!(payload["issues"], 0);
    assert_eq!(payload["advisoryIssueCount"], 1);
    assert_eq!(
        payload["issueDetails"][0]["obligationEnforcement"],
        "advisory"
    );
    assert_eq!(payload["issueDetails"][0]["needsAuthorConfirmation"], true);
}

#[tokio::test]
async fn review_evidence_must_be_locatable_in_the_current_candidate() {
    let llm = Arc::new(ScriptedLlm::new(vec![
        ("STEP:story_macro", story_macro()),
        ("STEP:character_cast", scripted_cast()),
        ("STEP:volume_strategy", scripted_volume()),
        ("STEP:chapter_list", chapter_list()),
        ("STEP:chapter_draft", "林动赶到时，老铁匠已经死亡。".into()),
        ("STEP:consistency_review", invented_evidence_review()),
    ]));
    let (_dir, _kernel, runtime) = runtime_for(llm.clone(), "p17410-evidence").await;
    let novel = runtime.director.create_novel("伪造证据", "").await.unwrap();
    let chapter = runtime
        .director
        .create_chapter(&novel.id, "第一章", 1)
        .await
        .unwrap();
    let run_id = open_run(&runtime, &novel.id, &chapter.id, "p17410-evidence-open").await;
    let (status, signal) = drive_until_paused_or_done(&runtime, &novel.id, &run_id, 40).await;
    assert_eq!(status, "paused");
    assert_eq!(signal.as_deref(), Some("data_integrity"));
    let prompts = llm.prompts();
    assert_eq!(
        prompts
            .iter()
            .filter(|prompt| prompt.contains("STEP:consistency_review"))
            .count(),
        1
    );
    assert!(
        !prompts
            .iter()
            .any(|prompt| prompt.contains("STEP:chapter_repair")),
        "fabricated evidence must fail closed instead of driving a repair"
    );
}

/// Beta-A1 诊断：结构化步骤失败时保留首次与修复的原始输出，便于不重跑付费模型即可定位。
#[tokio::test]
async fn structured_failure_artifact_keeps_raw_outputs_for_diagnosis() {
    let llm = Arc::new(ScriptedLlm::new(vec![
        ("STEP:story_macro", "不是 JSON".into()),
        ("STEP:story_macro", "仍然不是 JSON".into()),
    ]));
    let (_dir, kernel, runtime) = runtime_for(llm.clone(), "beta-a1-diag").await;
    let novel = runtime
        .director
        .create_novel("结构化失败诊断", "")
        .await
        .unwrap();
    let chapter = runtime
        .director
        .create_chapter(&novel.id, "第一章", 1)
        .await
        .unwrap();
    let run_id = open_run(&runtime, &novel.id, &chapter.id, "beta-a1-diag-open").await;
    let (status, signal) = drive_until_paused_or_done(&runtime, &novel.id, &run_id, 20).await;
    assert_eq!(status, "paused");
    assert_eq!(signal.as_deref(), Some("data_integrity"));

    let artifacts = WorkflowRepository::list_artifacts(kernel.as_ref(), &novel.id)
        .await
        .unwrap();
    let diagnostic = artifacts
        .iter()
        .find(|artifact| artifact.artifact_type == "structured_failure")
        .expect("structured_failure artifact");
    let payload: serde_json::Value = serde_json::from_str(&diagnostic.payload_json).unwrap();
    assert_eq!(payload["stepId"], "story_macro");
    assert_eq!(payload["firstRaw"], "不是 JSON");
    assert_eq!(payload["repairRaw"], "仍然不是 JSON");
    assert!(
        payload["firstError"]
            .as_str()
            .is_some_and(|text| !text.is_empty())
    );
    assert!(
        payload["repairError"]
            .as_str()
            .is_some_and(|text| !text.is_empty())
    );
}

fn story_macro_with_key_events() -> String {
    let mut value: serde_json::Value = serde_json::from_str(&story_macro()).unwrap();
    value["keyEvents"] = serde_json::json!([{
        "id": "ch1-event-1",
        "chapterOrder": 1,
        "type": "required_event",
        "description": "林动亲眼撞见老铁匠被杀",
        "participants": ["林动", "老铁匠"],
        "enforcement": "hard"
    }]);
    value.to_string()
}

fn chapter_list_with_key_coverage() -> String {
    let mut value: serde_json::Value = serde_json::from_str(&chapter_list()).unwrap();
    value["chapters"][0]["obligations"][0]["sourceEventId"] = serde_json::json!("ch1-event-1");
    value.to_string()
}

#[tokio::test]
async fn missing_story_key_event_coverage_pauses_before_writing() {
    let llm = Arc::new(ScriptedLlm::new(vec![
        ("STEP:story_macro", story_macro_with_key_events()),
        ("STEP:character_cast", scripted_cast()),
        ("STEP:volume_strategy", scripted_volume()),
        ("STEP:chapter_list", chapter_list()),
        ("STEP:chapter_draft", "不应到达的正文".into()),
        ("STEP:consistency_review", clean_review()),
    ]));
    let (_dir, _kernel, runtime) = runtime_for(llm.clone(), "p17410-coverage-missing").await;
    let novel = runtime
        .director
        .create_novel("关键事件覆盖", "")
        .await
        .unwrap();
    let chapter = runtime
        .director
        .create_chapter(&novel.id, "第一章", 1)
        .await
        .unwrap();
    let run_id = open_run(&runtime, &novel.id, &chapter.id, "p17410-coverage-open").await;
    let (status, signal) = drive_until_paused_or_done(&runtime, &novel.id, &run_id, 40).await;
    assert_eq!(status, "paused");
    assert_eq!(signal.as_deref(), Some("data_integrity"));
    let detail = runtime
        .director
        .get_run_detail(&novel.id, &run_id)
        .await
        .unwrap();
    assert!(
        detail
            .pause_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("未覆盖")),
        "coverage error must be explicit: {:?}",
        detail.pause_reason
    );
    assert!(
        !llm.prompts()
            .iter()
            .any(|prompt| prompt.contains("STEP:chapter_draft")),
        "an uncovered key event must not reach the writer"
    );
}

#[tokio::test]
async fn covered_story_key_events_reach_the_writer_with_source_id() {
    let llm = Arc::new(ScriptedLlm::new(vec![
        ("STEP:story_macro", story_macro_with_key_events()),
        ("STEP:character_cast", scripted_cast()),
        ("STEP:volume_strategy", scripted_volume()),
        ("STEP:chapter_list", chapter_list_with_key_coverage()),
        ("STEP:chapter_draft", "候选正文".into()),
        ("STEP:consistency_review", clean_review()),
    ]));
    let (_dir, _kernel, runtime) = runtime_for(llm.clone(), "p17410-coverage-ok").await;
    let novel = runtime
        .director
        .create_novel("关键事件已覆盖", "")
        .await
        .unwrap();
    let chapter = runtime
        .director
        .create_chapter(&novel.id, "第一章", 1)
        .await
        .unwrap();
    let run_id = open_run(&runtime, &novel.id, &chapter.id, "p17410-coverage-ok-open").await;
    let (status, _signal) = drive_until_paused_or_done(&runtime, &novel.id, &run_id, 40).await;
    assert_eq!(status, "succeeded");
    let writer = llm
        .prompts()
        .into_iter()
        .find(|prompt| prompt.contains("STEP:chapter_draft"))
        .expect("writer prompt");
    assert!(
        writer.contains("来源事件：ch1-event-1"),
        "writer must receive the frozen key event reference"
    );
}
/// Beta-A1 运行时探针：直接检查 worker 本轮实际发送的 Prompt 文本与产物，
/// 用版本独有的模板措辞证明运行时确实使用了 chapter-list-v2 / chapter-draft-v3 /
/// consistency-review-v2 / chapter-repair-v2，而不是只检查源码默认值。
#[tokio::test]
async fn beta_a1_runtime_prompt_versions_are_observable() {
    let llm = Arc::new(ScriptedLlm::new(vec![
        ("STEP:story_macro", story_macro()),
        ("STEP:character_cast", scripted_cast()),
        ("STEP:volume_strategy", scripted_volume()),
        ("STEP:chapter_list", chapter_list()),
        ("STEP:chapter_draft", "林动赶到时，老铁匠已经死亡。".into()),
        ("STEP:consistency_review", known_issue_review()),
        ("STEP:chapter_repair", "林动亲眼撞见老铁匠被杀。".into()),
        ("STEP:consistency_review", clean_review()),
    ]));
    let (_dir, kernel, runtime) = runtime_for(llm.clone(), "beta-a1-runtime").await;
    let novel = runtime
        .director
        .create_novel("Beta-A1 运行时探针", "")
        .await
        .unwrap();
    let chapter = runtime
        .director
        .create_chapter(&novel.id, "第一章", 1)
        .await
        .unwrap();
    let run_id = open_run(&runtime, &novel.id, &chapter.id, "beta-a1-runtime-open").await;
    let (status, signal) = drive_until_paused_or_done(&runtime, &novel.id, &run_id, 60).await;

    let prompts = llm.prompts();
    let planner = prompts
        .iter()
        .find(|prompt| prompt.contains("STEP:chapter_list"))
        .expect("chapter_list prompt");
    assert!(
        planner.contains("obligations 是本章必须遵守的结构化义务数组"),
        "runtime planner prompt must be chapter-list-v2"
    );
    assert!(planner.contains("每章至少一条 hard 义务"));
    assert!(
        planner.contains("山门之下封着什么？"),
        "planner prompt must include story_macro"
    );

    let writer = prompts
        .iter()
        .find(|prompt| prompt.contains("STEP:chapter_draft"))
        .expect("chapter_draft prompt");
    assert!(
        writer.contains("章节义务：\n【硬约束｜必须遵守"),
        "runtime writer prompt must use the chapter-draft-v3 template slot"
    );
    assert!(writer.contains("硬约束必须遵守，不得改写关键事件"));
    assert!(
        !writer.contains("如果正文与硬约束冲突，必须调整正文"),
        "v3 must not go through the v2 render-then-append path"
    );

    let reviewer = prompts
        .iter()
        .find(|prompt| prompt.contains("STEP:consistency_review"))
        .expect("review prompt");
    assert!(
        reviewer.contains("没有采用 advisory 建议不构成问题"),
        "runtime reviewer prompt must be consistency-review-v2"
    );
    assert!(reviewer.contains("obligationId"));
    assert!(reviewer.contains("林动赶到时，老铁匠已经死亡。"));

    let repair = prompts
        .iter()
        .find(|prompt| prompt.contains("STEP:chapter_repair"))
        .expect("repair prompt");
    assert!(
        repair.contains("本章章节义务") && repair.contains("硬约束必须保留"),
        "runtime repair prompt must be chapter-repair-v2"
    );
    assert!(repair.contains("创作建议可以合理取舍"));
    assert!(repair.contains("ch1-req-1"));

    println!(
        "BETA_A1_PROMPT_PROBE chapter_list=chapter-list-v2 chapter_draft=chapter-draft-v3 consistency_review=consistency-review-v2 chapter_repair=chapter-repair-v2"
    );
    println!("BETA_A1_RUN status={status} signal={signal:?}");

    let artifacts = WorkflowRepository::list_artifacts(kernel.as_ref(), &novel.id)
        .await
        .unwrap();
    let context = artifacts
        .iter()
        .find(|artifact| artifact.artifact_type == "context")
        .expect("context artifact");
    let context: serde_json::Value = serde_json::from_str(&context.payload_json).unwrap();
    assert_eq!(context["runId"].as_str(), Some(run_id.as_str()));
    assert_eq!(context["chapterPlanVersion"], 1);
    let ids = context["obligationIds"].as_array().expect("obligation ids");
    assert!(ids.iter().any(|id| id == "ch1-req-1"));
    assert!(ids.iter().any(|id| id == "ch1-order-1"));
    assert!(ids.iter().any(|id| id == "ch1-adv-1"));
    let text = context["obligationsText"]
        .as_str()
        .expect("obligations text");
    assert!(text.contains("【硬约束｜必须遵守"));
    assert!(text.contains("【创作建议｜可合理取舍"));
    let trace = &context["trace"];
    assert_eq!(trace["chapterPlanVersion"], 1);
    assert!(trace["sources"].as_array().is_some_and(|sources| {
        sources.iter().any(|source| {
            source["sourceType"] == "chapter_obligations"
                && source["selected"] == true
                && source["contentHash"].is_string()
        })
    }));

    let review = artifacts
        .iter()
        .filter(|artifact| artifact.artifact_type == "consistency_review")
        .find(|artifact| {
            serde_json::from_str::<serde_json::Value>(&artifact.payload_json)
                .ok()
                .and_then(|payload| {
                    payload["issueDetails"][0]["obligationId"]
                        .as_str()
                        .map(|id| id == "ch1-req-1")
                })
                .unwrap_or(false)
        })
        .expect("hard review artifact");
    let review: serde_json::Value = serde_json::from_str(&review.payload_json).unwrap();
    assert_eq!(review["issueDetails"][0]["obligationEnforcement"], "hard");
    assert_eq!(review["issues"], 1);
}
