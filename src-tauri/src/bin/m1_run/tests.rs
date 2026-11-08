//! M1 执行器契约与预算测试。

use super::budget::CostGuard;
use super::*;
use novel_llm::{LlmError, LlmProvider, LlmRequest, LlmResponse, ProviderCapabilities, TokenUsage};
use novel_storage_sqlite::SqliteKernel;
use novel_testkit::{FakeClock, ScriptedLlm, SequentialIds};

fn request() -> LlmRequest {
    LlmRequest {
        request_id: "test-request".into(),
        model: "mock-model".into(),
        prompt: "hello".into(),
        endpoint: "https://api.example.com/v1/chat/completions".into(),
        api_key: "test-key".into(),
        allow_localhost_http: false,
        max_output_tokens: Some(64),
    }
}

fn channel() -> tokio::sync::mpsc::Sender<String> {
    tokio::sync::mpsc::channel(4).0
}

struct StubProvider {
    calls: std::sync::atomic::AtomicU64,
    usage: TokenUsage,
    error: Option<LlmError>,
}

impl StubProvider {
    fn ok(usage: TokenUsage) -> Arc<Self> {
        Arc::new(Self {
            calls: std::sync::atomic::AtomicU64::new(0),
            usage,
            error: None,
        })
    }

    fn failing(error: LlmError) -> Arc<Self> {
        Arc::new(Self {
            calls: std::sync::atomic::AtomicU64::new(0),
            usage: TokenUsage::unknown(),
            error: Some(error),
        })
    }

    fn calls(&self) -> u64 {
        self.calls.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn respond(&self) -> Result<LlmResponse, LlmError> {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        match &self.error {
            Some(LlmError::Timeout) => Err(LlmError::Timeout),
            Some(LlmError::Cancelled) => Err(LlmError::Cancelled),
            Some(other) => Err(LlmError::InvalidResponse(other.to_string())),
            None => Ok(LlmResponse {
                text: "ok".into(),
                usage: self.usage.clone(),
            }),
        }
    }
}

#[async_trait::async_trait]
impl LlmProvider for StubProvider {
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            streaming: false,
            structured_output: false,
            tool_call: false,
            reasoning: false,
            context_limit: 8_192,
        }
    }

    async fn generate(
        &self,
        _request: LlmRequest,
        _tokens: tokio::sync::mpsc::Sender<String>,
        _cancel: tokio_util::sync::CancellationToken,
    ) -> Result<LlmResponse, LlmError> {
        self.respond()
    }

    async fn generate_with_protocol(
        &self,
        _request: LlmRequest,
        _protocol: &str,
        _reasoning_effort: Option<&str>,
        _tokens: tokio::sync::mpsc::Sender<String>,
        _cancel: tokio_util::sync::CancellationToken,
    ) -> Result<LlmResponse, LlmError> {
        self.respond()
    }
}

fn cancel() -> tokio_util::sync::CancellationToken {
    tokio_util::sync::CancellationToken::new()
}

// ---- Phase 2：M1 固定新 Planner 契约 ----

#[test]
fn prompt_versions_pin_the_new_planner_contract() {
    let prompts = m1_prompt_versions();
    assert_eq!(
        prompts.get("story_macro").map(String::as_str),
        Some("story-macro-v2")
    );
    assert_eq!(
        prompts.get("chapter_list").map(String::as_str),
        Some("chapter-list-v3")
    );
    assert_eq!(
        prompts.get("chapter_draft").map(String::as_str),
        Some("chapter-draft-v3")
    );
    assert_eq!(
        prompts.get("chapter_repair").map(String::as_str),
        Some("chapter-repair-v2")
    );
    let line = prompt_versions_line(&prompts);
    assert!(line.contains("story_macro=story-macro-v2"));
    assert!(line.contains("chapter_list=chapter-list-v3"));
    assert!(
        !line.contains("key") && !line.contains("http"),
        "版本清单必须脱敏"
    );
}

#[test]
fn planner_templates_load() {
    assert!(novel_prompt::resolve("story_macro", "story-macro-v2").is_ok());
    assert!(novel_prompt::resolve("chapter_list", "chapter-list-v3").is_ok());
}

async fn m1_runtime(llm: Arc<ScriptedLlm>) -> (tempfile::TempDir, Arc<SqliteKernel>, Runtime) {
    let dir = tempfile::tempdir().unwrap();
    let kernel = Arc::new(
        SqliteKernel::open(&dir.path().join("m1.sqlite"))
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
        faults: Arc::new(NoFault),
        secrets: Arc::new(DryRunSecrets),
        owner: "m1-test".into(),
        lease_ms: 60_000,
        concurrency: 1,
    });
    (dir, kernel, runtime)
}

async fn open_m1_run(
    runtime: &Runtime,
    novel_id: &str,
    chapter_id: &str,
    prompt_versions: BTreeMap<String, String>,
) -> String {
    runtime
        .director
        .open_run(OpenRunInput {
            command_id: format!("m1-test-open-{}", novel_platform::now_ms()),
            novel_id: novel_id.into(),
            target_chapter_id: chapter_id.into(),
            provider: "mock".into(),
            credential_ref: Some("mock".into()),
            model: "mock-model".into(),
            prompt_versions,
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

async fn drive(runtime: &Runtime, novel_id: &str, max_ticks: usize) -> String {
    let mut last = "queued".to_string();
    for tick in 0..max_ticks {
        match runtime.worker.tick().await.unwrap() {
            TickOutcome::Advanced { status, run_id } => {
                last = status.clone();
                if status == "waiting_approval" {
                    let detail = runtime
                        .director
                        .get_run_detail(novel_id, &run_id)
                        .await
                        .unwrap();
                    let reviewed = detail
                        .artifacts
                        .iter()
                        .any(|artifact| artifact.artifact_type == "consistency_review");
                    if reviewed {
                        break;
                    }
                    runtime
                        .director
                        .approve(&format!("m1-test-approve-{tick}"), &run_id)
                        .await
                        .unwrap();
                }
                if matches!(status.as_str(), "paused" | "succeeded" | "failed") {
                    break;
                }
            }
            TickOutcome::Idle => break,
            _ => break,
        }
    }
    last
}

fn story_macro_v2() -> String {
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
        "keyEvents": [{
            "id": "ch1-event-1",
            "chapterOrder": 1,
            "type": "required_event",
            "description": "林动亲眼撞见老铁匠被杀",
            "participants": ["林动", "老铁匠"],
            "enforcement": "hard"
        }],
        "constraints": ["不提前揭露旧案"],
        "issues": []
    })
    .to_string()
}

fn story_macro_v1() -> String {
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
        "constraints": [],
        "issues": []
    })
    .to_string()
}

fn scripted_cast() -> String {
    serde_json::json!({
        "members": [
            {"name": "林动", "role": "主角", "castRole": "protagonist", "storyFunction": "承担真相"},
            {"name": "苏晚", "role": "同伴", "castRole": "ally", "storyFunction": "协助"},
            {"name": "陆沉", "role": "对手", "castRole": "antagonist", "storyFunction": "施压"},
            {"name": "老铁匠", "role": "受害者", "castRole": "supporting", "storyFunction": "触发旧案"}
        ],
        "relations": [
            {"sourceName": "林动", "targetName": "苏晚", "surfaceRelation": "同门", "conflictSource": "秘密隐瞒"},
            {"sourceName": "林动", "targetName": "陆沉", "surfaceRelation": "敌对", "conflictSource": "旧案"}
        ]
    })
    .to_string()
}

fn scripted_volume() -> String {
    serde_json::json!({"volumes": [{"volumeOrder": 1, "title": "山门卷", "roleLabel": "开局", "coreReward": "入山", "planningMode": "hard"}]}).to_string()
}

fn chapter_list_v3() -> String {
    serde_json::json!({
        "chapters": [{
            "chapterOrder": 1,
            "title": "第一章 雪夜",
            "volumeTitle": "山门卷",
            "summary": "林动亲眼撞见老铁匠被灭口",
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

fn chapter_list_rewritten_event() -> String {
    serde_json::json!({
        "chapters": [{
            "chapterOrder": 1,
            "title": "第一章 雪夜",
            "volumeTitle": "山门卷",
            "summary": "林动到达时老铁匠已经死亡",
            "characterNames": ["林动", "老铁匠"],
            "obligations": [{
                "id": "ch1-req-1",
                "type": "required_event",
                "description": "林动到达时老铁匠已经死亡",
                "enforcement": "hard",
                "participants": ["林动", "老铁匠"],
                "sourceEventId": "ch1-event-1"
            }]
        }]
    })
    .to_string()
}

fn chapter_list_v1() -> String {
    serde_json::json!({
        "chapters": [{
            "chapterOrder": 1,
            "title": "第一章 雪夜",
            "volumeTitle": "山门卷",
            "summary": "林动在雪夜发现玉佩",
            "characterNames": ["林动"],
            "obligations": [{
                "id": "ch1-req-1",
                "type": "required_event",
                "description": "林动在雪夜发现玉佩",
                "enforcement": "hard",
                "participants": ["林动"]
            }]
        }]
    })
    .to_string()
}

fn clean_review() -> String {
    r#"{"issues":[]}"#.to_string()
}

const DRAFT: &str = "候选正文：雪夜，林动看见老铁匠被杀。";

#[tokio::test]
async fn m1_run_uses_v2_prompts_and_writer_gets_source_event() {
    let llm = Arc::new(ScriptedLlm::new(vec![
        ("STEP:story_macro", story_macro_v2()),
        ("STEP:character_cast", scripted_cast()),
        ("STEP:volume_strategy", scripted_volume()),
        ("STEP:chapter_list", chapter_list_v3()),
        ("STEP:chapter_draft", DRAFT.into()),
        ("STEP:consistency_review", clean_review()),
    ]));
    let (_dir, _kernel, runtime) = m1_runtime(llm.clone()).await;
    let novel = runtime
        .director
        .create_novel(M1_TITLE, M1_DESCRIPTION)
        .await
        .unwrap();
    let chapter = runtime
        .director
        .create_chapter(&novel.id, M1_CHAPTER_TITLE, 1)
        .await
        .unwrap();
    let run_id = open_m1_run(&runtime, &novel.id, &chapter.id, m1_prompt_versions()).await;
    let status = drive(&runtime, &novel.id, 60).await;
    assert_eq!(status, "waiting_approval");
    let prompts = llm.prompts();
    let story = prompts
        .iter()
        .find(|prompt| prompt.contains("STEP:story_macro"))
        .expect("story prompt");
    assert!(
        story.contains("keyEvents 是跨章节必须发生的关键事件数组"),
        "M1 must use the story-macro-v2 template, not v1"
    );
    let planner = prompts
        .iter()
        .find(|prompt| prompt.contains("STEP:chapter_list"))
        .expect("chapter_list prompt");
    assert!(
        planner.contains("sourceEventId 填该事件 id"),
        "M1 must use the chapter-list-v3 template, not v2"
    );
    let writer = prompts
        .iter()
        .find(|prompt| prompt.contains("STEP:chapter_draft"))
        .expect("writer prompt");
    assert!(
        writer.contains("来源事件：ch1-event-1"),
        "writer must receive the canonical source event id"
    );
    let detail = runtime
        .director
        .get_run_detail(&novel.id, &run_id)
        .await
        .unwrap();
    assert!(
        detail
            .artifacts
            .iter()
            .any(|artifact| artifact.artifact_type == "consistency_review")
    );
}

#[tokio::test]
async fn missing_key_events_stops_the_m1_run() {
    let llm = Arc::new(ScriptedLlm::new(vec![
        ("STEP:story_macro", story_macro_v1()),
        ("STEP:story_macro", story_macro_v1()),
        ("STEP:chapter_draft", "不应到达的正文".into()),
    ]));
    let (_dir, _kernel, runtime) = m1_runtime(llm.clone()).await;
    let novel = runtime
        .director
        .create_novel(M1_TITLE, M1_DESCRIPTION)
        .await
        .unwrap();
    let chapter = runtime
        .director
        .create_chapter(&novel.id, M1_CHAPTER_TITLE, 1)
        .await
        .unwrap();
    let run_id = open_m1_run(&runtime, &novel.id, &chapter.id, m1_prompt_versions()).await;
    let status = drive(&runtime, &novel.id, 20).await;
    assert_eq!(status, "paused");
    let detail = runtime
        .director
        .get_run_detail(&novel.id, &run_id)
        .await
        .unwrap();
    assert_eq!(detail.pause_signal.as_deref(), Some("data_integrity"));
    assert!(
        !llm.prompts()
            .iter()
            .any(|prompt| prompt.contains("STEP:chapter_draft")),
        "the writer must not start without keyEvents"
    );
}

#[tokio::test]
async fn rewritten_canonical_event_stops_the_m1_run() {
    let llm = Arc::new(ScriptedLlm::new(vec![
        ("STEP:story_macro", story_macro_v2()),
        ("STEP:character_cast", scripted_cast()),
        ("STEP:volume_strategy", scripted_volume()),
        ("STEP:chapter_list", chapter_list_rewritten_event()),
        ("STEP:chapter_list", chapter_list_rewritten_event()),
        ("STEP:chapter_draft", "不应到达的正文".into()),
    ]));
    let (_dir, _kernel, runtime) = m1_runtime(llm.clone()).await;
    let novel = runtime
        .director
        .create_novel(M1_TITLE, M1_DESCRIPTION)
        .await
        .unwrap();
    let chapter = runtime
        .director
        .create_chapter(&novel.id, M1_CHAPTER_TITLE, 1)
        .await
        .unwrap();
    let run_id = open_m1_run(&runtime, &novel.id, &chapter.id, m1_prompt_versions()).await;
    let status = drive(&runtime, &novel.id, 20).await;
    assert_eq!(status, "paused");
    let detail = runtime
        .director
        .get_run_detail(&novel.id, &run_id)
        .await
        .unwrap();
    assert_eq!(detail.pause_signal.as_deref(), Some("data_integrity"));
    assert!(
        !llm.prompts()
            .iter()
            .any(|prompt| prompt.contains("STEP:chapter_draft"))
    );
}

#[tokio::test]
async fn legacy_v1_prompts_stay_compatible() {
    let mut prompts = BTreeMap::new();
    prompts.insert("chapter_draft".into(), "chapter-draft-v3".into());
    prompts.insert("chapter_repair".into(), "chapter-repair-v2".into());
    let llm = Arc::new(ScriptedLlm::new(vec![
        ("STEP:story_macro", story_macro_v1()),
        ("STEP:character_cast", scripted_cast()),
        ("STEP:volume_strategy", scripted_volume()),
        ("STEP:chapter_list", chapter_list_v1()),
        ("STEP:chapter_draft", DRAFT.into()),
        ("STEP:consistency_review", clean_review()),
    ]));
    let (_dir, _kernel, runtime) = m1_runtime(llm.clone()).await;
    let novel = runtime
        .director
        .create_novel(M1_TITLE, M1_DESCRIPTION)
        .await
        .unwrap();
    let chapter = runtime
        .director
        .create_chapter(&novel.id, M1_CHAPTER_TITLE, 1)
        .await
        .unwrap();
    let run_id = open_m1_run(&runtime, &novel.id, &chapter.id, prompts).await;
    let status = drive(&runtime, &novel.id, 60).await;
    assert_eq!(
        status, "waiting_approval",
        "legacy v1 prompts must keep working for historical runs"
    );
    let detail = runtime
        .director
        .get_run_detail(&novel.id, &run_id)
        .await
        .unwrap();
    assert!(
        detail
            .artifacts
            .iter()
            .any(|artifact| artifact.artifact_type == "consistency_review")
    );
}

// ---- Phase 3：请求前调用预算与费用门禁 ----

fn ok_usage() -> TokenUsage {
    TokenUsage {
        prompt_tokens: Some(10),
        completion_tokens: Some(5),
    }
}

#[tokio::test]
async fn single_request_counts_once() {
    let inner = StubProvider::ok(ok_usage());
    let budget = CallBudget::new(2);
    let llm = BudgetedLlm::new(inner.clone(), budget.clone());
    llm.generate_with_protocol(request(), "openai-compatible", None, channel(), cancel())
        .await
        .unwrap();
    assert_eq!(inner.calls(), 1);
    assert_eq!(budget.snapshot(), (1, 0));
}

#[tokio::test]
async fn structured_call_and_repair_each_consume_budget() {
    let inner = StubProvider::ok(ok_usage());
    let budget = CallBudget::new(2);
    let llm = BudgetedLlm::new(inner.clone(), budget.clone());
    for _ in 0..2 {
        llm.generate_with_protocol(request(), "openai-compatible", None, channel(), cancel())
            .await
            .unwrap();
    }
    assert_eq!(inner.calls(), 2);
    assert_eq!(budget.snapshot().0, 2);
}

#[tokio::test]
async fn call_limit_blocks_before_the_request_is_sent() {
    let inner = StubProvider::ok(ok_usage());
    let budget = CallBudget::new(1);
    let llm = BudgetedLlm::new(inner.clone(), budget.clone());
    llm.generate_with_protocol(request(), "openai-compatible", None, channel(), cancel())
        .await
        .unwrap();
    let error = llm
        .generate_with_protocol(request(), "openai-compatible", None, channel(), cancel())
        .await
        .unwrap_err();
    assert!(matches!(error, LlmError::BudgetExceeded(_)));
    assert_eq!(
        inner.calls(),
        1,
        "rejected requests never reach the provider"
    );
    assert_eq!(budget.snapshot().0, 1);
}

#[tokio::test]
async fn missing_usage_is_recorded_as_unknown() {
    let inner = StubProvider::ok(TokenUsage::unknown());
    let budget = CallBudget::new(1);
    let llm = BudgetedLlm::new(inner.clone(), budget.clone());
    llm.generate_with_protocol(request(), "openai-compatible", None, channel(), cancel())
        .await
        .unwrap();
    assert_eq!(budget.snapshot(), (1, 1));
}

#[tokio::test]
async fn provider_timeout_still_consumes_the_call_and_marks_usage_unknown() {
    let inner = StubProvider::failing(LlmError::Timeout);
    let budget = CallBudget::new(1);
    let llm = BudgetedLlm::new(inner.clone(), budget.clone());
    let error = llm
        .generate_with_protocol(request(), "openai-compatible", None, channel(), cancel())
        .await
        .unwrap_err();
    assert!(matches!(error, LlmError::Timeout));
    assert_eq!(inner.calls(), 1);
    assert_eq!(budget.snapshot(), (1, 1));
}

#[tokio::test]
async fn cancelled_request_still_consumes_the_call() {
    let inner = StubProvider::failing(LlmError::Cancelled);
    let budget = CallBudget::new(1);
    let llm = BudgetedLlm::new(inner.clone(), budget.clone());
    let error = llm
        .generate_with_protocol(request(), "openai-compatible", None, channel(), cancel())
        .await
        .unwrap_err();
    assert!(matches!(error, LlmError::Cancelled));
    assert_eq!(budget.snapshot(), (1, 1));
}

#[test]
fn real_mode_requires_a_provider_side_cost_guard() {
    let error = resolve_cost_guard(false, None).unwrap_err();
    assert!(error.contains("供应商侧"), "{error}");
    assert!(error.contains("M1_COST_GUARD=provider_side"), "{error}");
    assert_eq!(
        resolve_cost_guard(false, Some("provider_side")).unwrap(),
        CostGuard::ProviderSide
    );
    assert_eq!(resolve_cost_guard(true, None).unwrap(), CostGuard::DryRun);
}
