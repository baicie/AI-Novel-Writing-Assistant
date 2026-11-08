//! Upstream-Sync-A1：LLM 调用观测的最小闭环。
//!
//! 上游 `0026d0cd` 的核心结论是「一次物理 Provider 请求一条独立调用记录」。
//! 本套件把该结论转成 Fork 的行为契约，只断言计费可追溯性，不涉及正文内容：
//!
//! 1. 结构化首次请求与格式修复请求各自独立记账；
//! 2. 结果未知（可能已计费）的失败也留下记录，并带 `unknown` 状态；
//! 3. 取消与鉴权失败不制造「已计费」的假象；
//! 4. 运行汇总把 `known` 与 `unknown` 分开计数，不把未知当成 0。

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use novel_application::{
    ModelSecrets, NewChapter, NovelRepository, OpenRunInput, PortError, Runtime, RuntimeDeps,
    TickOutcome,
};
use novel_llm::{LlmError, LlmProvider, LlmRequest, LlmResponse, ProviderCapabilities, TokenUsage};
use novel_storage_sqlite::SqliteKernel;
use novel_testkit::{FakeClock, SequentialIds};
use sqlx::Row;
use tokio_util::sync::CancellationToken;

struct FixtureSecrets;

#[async_trait]
impl ModelSecrets for FixtureSecrets {
    async fn api_key(&self, _provider: &str) -> Result<Option<String>, PortError> {
        Ok(Some("fixture-key".into()))
    }
}

/// 按顺序返回脚本化结果；每个结果要么是一段文本，要么是一次 Provider 失败。
#[derive(Clone)]
enum Script {
    Text(String),
    Fail(LlmError),
    /// 先按文本流式输出，再以失败结束——对应「已经计费但结果不可用」的真实路径。
    TextThenFail(String, LlmError),
}

struct ScriptedFailingLlm {
    calls: AtomicUsize,
    script: Vec<Script>,
}

impl ScriptedFailingLlm {
    fn new(script: Vec<Script>) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            script,
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl LlmProvider for ScriptedFailingLlm {
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            streaming: true,
            structured_output: true,
            tool_call: false,
            reasoning: false,
            context_limit: 128_000,
        }
    }

    async fn generate(
        &self,
        _request: LlmRequest,
        tokens: tokio::sync::mpsc::Sender<String>,
        cancel: CancellationToken,
    ) -> Result<LlmResponse, LlmError> {
        if cancel.is_cancelled() {
            return Err(LlmError::Cancelled);
        }
        let index = self.calls.fetch_add(1, Ordering::SeqCst);
        let Some(step) = self.script.get(index).cloned() else {
            return Err(LlmError::InvalidResponse("没有更多脚本化响应".into()));
        };
        match step {
            Script::Text(text) => {
                let _ = tokens.send(text.clone()).await;
                Ok(LlmResponse {
                    text,
                    usage: TokenUsage {
                        prompt_tokens: Some(120),
                        completion_tokens: Some(80),
                    },
                })
            }
            Script::Fail(error) => Err(error),
            Script::TextThenFail(text, error) => {
                let _ = tokens.send(text).await;
                Err(error)
            }
        }
    }
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

async fn runtime_for(
    llm: Arc<dyn LlmProvider>,
    owner: &str,
) -> (tempfile::TempDir, Arc<SqliteKernel>, Runtime) {
    let dir = tempfile::tempdir().unwrap();
    let kernel = Arc::new(
        SqliteKernel::open(&dir.path().join("observability.sqlite"))
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
    let mut prompts = BTreeMap::new();
    prompts.insert("chapter_draft".into(), "chapter-draft-v3".into());
    prompts.insert("chapter_repair".into(), "chapter-repair-v2".into());
    runtime
        .director
        .open_run(OpenRunInput {
            command_id: command_id.into(),
            novel_id: novel_id.into(),
            target_chapter_id: chapter_id.into(),
            provider: "mock".into(),
            credential_ref: Some("mock".into()),
            model: "mock-model".into(),
            prompt_versions: prompts,
            token_budget: Some(4_096),
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

async fn tick_until_stop(runtime: &Runtime, novel_id: &str, run_id: &str, max_ticks: usize) {
    for tick in 0..max_ticks {
        match runtime.worker.tick().await.unwrap() {
            TickOutcome::Advanced { status, run_id: id } => {
                if status == "waiting_approval" {
                    runtime
                        .director
                        .approve(&format!("obs-approve-{tick}"), &id)
                        .await
                        .unwrap();
                }
                if status == "paused" || status == "succeeded" || status == "failed" {
                    return;
                }
            }
            TickOutcome::Idle => return,
            other => panic!("unexpected tick outcome: {other:?}"),
        }
    }
    let detail = runtime
        .director
        .get_run_detail(novel_id, run_id)
        .await
        .unwrap();
    panic!(
        "run did not stop within {max_ticks} ticks, status={:?}",
        detail
    );
}

struct InvocationRow {
    request_id: String,
    step_id: String,
    status: String,
    prompt_tokens: Option<i64>,
    completion_tokens: Option<i64>,
}

async fn invocations(kernel: &SqliteKernel, run_id: &str) -> Vec<InvocationRow> {
    let rows = sqlx::query(
        "SELECT request_id, step_id, status, prompt_tokens, completion_tokens
         FROM llm_invocation WHERE run_id = ? ORDER BY rowid ASC",
    )
    .bind(run_id)
    .fetch_all(kernel.pool())
    .await
    .unwrap();
    rows.into_iter()
        .map(|row| InvocationRow {
            request_id: row.try_get("request_id").unwrap(),
            step_id: row.try_get("step_id").unwrap(),
            status: row.try_get("status").unwrap(),
            prompt_tokens: row.try_get("prompt_tokens").unwrap(),
            completion_tokens: row.try_get("completion_tokens").unwrap(),
        })
        .collect()
}

async fn seed_novel(runtime: &Runtime) -> (String, String) {
    let novel = runtime
        .director
        .create_novel("调用观测", "只验证记账")
        .await
        .unwrap();
    let chapter = runtime
        .director
        .create_chapter(&novel.id, "第一章", 1)
        .await
        .unwrap();
    (novel.id, chapter.id)
}

/// 结构化首次请求与格式修复请求是两次独立的物理调用，必须各留一条记录。
/// 旧实现把修复调用折叠进首次请求，修复流量在运行记录里完全不可见。
#[tokio::test]
async fn structured_repair_is_recorded_as_its_own_physical_invocation() {
    let llm = Arc::new(ScriptedFailingLlm::new(vec![
        Script::Text("不是 JSON".into()),
        Script::Text("仍然不是 JSON".into()),
    ]));
    let (_dir, kernel, runtime) = runtime_for(llm.clone(), "sync-a1-repair").await;
    let (novel_id, chapter_id) = seed_novel(&runtime).await;
    let run_id = open_run(&runtime, &novel_id, &chapter_id, "sync-a1-repair-open").await;
    tick_until_stop(&runtime, &novel_id, &run_id, 20).await;
    assert_eq!(llm.calls(), 2, "首次请求与修复请求都应发生");

    let rows = invocations(kernel.as_ref(), &run_id).await;
    assert_eq!(
        rows.len(),
        2,
        "两次物理请求必须各有一条调用记录，实际为 {:?}",
        rows.iter()
            .map(|row| (&row.step_id, &row.request_id, &row.status))
            .collect::<Vec<_>>()
    );
    assert!(
        rows.iter().all(|row| row.step_id == "story_macro"),
        "修复请求仍属于同一步骤"
    );
    // 记录顺序由写入事务决定，测试只约束「两条不同 requestId」这一事实。
    let request_ids: std::collections::BTreeSet<&str> =
        rows.iter().map(|row| row.request_id.as_str()).collect();
    assert_eq!(
        request_ids.len(),
        2,
        "每条记录必须绑定自己那次物理请求的 requestId"
    );
    // 修复调用的用量不能丢失，也不能被并进首次请求。
    for row in &rows {
        assert_eq!(
            (row.prompt_tokens, row.completion_tokens),
            (Some(120), Some(80)),
            "每次物理调用都应保留自己的用量"
        );
    }
    let statuses: std::collections::BTreeSet<&str> =
        rows.iter().map(|row| row.status.as_str()).collect();
    assert_eq!(
        statuses,
        ["repair_failed", "succeeded"].into_iter().collect(),
        "首次请求成功、修复请求失败，两者不能合并成一条"
    );
}

/// Provider 失败且无法确认是否计费时，成本不能静默消失。
#[tokio::test]
async fn unknown_billed_failure_keeps_a_trace_with_unknown_usage() {
    let llm = Arc::new(ScriptedFailingLlm::new(vec![Script::TextThenFail(
        "写到一半".into(),
        LlmError::Unavailable {
            billed: true,
            reason: "模型服务连接中断，无法确认是否已计费".into(),
        },
    )]));
    let (_dir, kernel, runtime) = runtime_for(llm.clone(), "sync-a1-unknown").await;
    let (novel_id, chapter_id) = seed_novel(&runtime).await;
    let run_id = open_run(&runtime, &novel_id, &chapter_id, "sync-a1-unknown-open").await;
    tick_until_stop(&runtime, &novel_id, &run_id, 20).await;

    let rows = invocations(kernel.as_ref(), &run_id).await;
    assert_eq!(rows.len(), 1, "失败的物理请求也要留下记录");
    let row = &rows[0];
    assert_eq!(row.step_id, "story_macro");
    assert_eq!(row.status, "provider_failed");
    assert_eq!(
        (row.prompt_tokens, row.completion_tokens),
        (None, None),
        "结果未知必须保持未知，不能写成 0 计费"
    );

    let usage = novel_application::WorkflowRepository::run_usage(kernel.as_ref(), &run_id)
        .await
        .unwrap();
    assert_eq!(usage.unknown_calls, 1, "未知调用要单独计数");
    assert_eq!(usage.known_calls, 0);
}

/// 确定性失败（明确没有发出请求、鉴权失败、取消）不能伪装成「可能已计费」。
#[tokio::test]
async fn preflight_and_cancel_failures_are_not_billed() {
    let llm = Arc::new(ScriptedFailingLlm::new(vec![
        Script::Fail(LlmError::Unauthorized),
        Script::Fail(LlmError::Cancelled),
    ]));
    let (_dir, kernel, runtime) = runtime_for(llm.clone(), "sync-a1-unbilled").await;
    let (novel_id, chapter_id) = seed_novel(&runtime).await;
    let run_id = open_run(&runtime, &novel_id, &chapter_id, "sync-a1-unbilled-open").await;
    tick_until_stop(&runtime, &novel_id, &run_id, 20).await;

    let rows = invocations(kernel.as_ref(), &run_id).await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, "unauthorized");
    assert_eq!(
        (rows[0].prompt_tokens, rows[0].completion_tokens),
        (None, None)
    );

    let usage = novel_application::WorkflowRepository::run_usage(kernel.as_ref(), &run_id)
        .await
        .unwrap();
    assert_eq!(usage.unknown_calls, 0, "鉴权失败不代表已计费");
    assert_eq!(usage.known_calls, 0);
}

/// 已有成功调用时，后续未知失败不能被汇总逻辑抹平。
#[tokio::test]
async fn run_usage_separates_known_and_unknown_calls() {
    let llm = Arc::new(ScriptedFailingLlm::new(vec![
        Script::Text(story_macro()),
        Script::Fail(LlmError::Unavailable {
            billed: true,
            reason: "模型服务等待首字节超时，无法确认是否已计费".into(),
        }),
    ]));
    let (_dir, kernel, runtime) = runtime_for(llm.clone(), "sync-a1-mixed").await;
    let (novel_id, chapter_id) = seed_novel(&runtime).await;
    let run_id = open_run(&runtime, &novel_id, &chapter_id, "sync-a1-mixed-open").await;
    tick_until_stop(&runtime, &novel_id, &run_id, 20).await;

    let rows = invocations(kernel.as_ref(), &run_id).await;
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].step_id, "story_macro");
    assert_eq!(rows[0].status, "succeeded");
    assert_eq!(rows[1].step_id, "character_cast");
    assert_eq!(rows[1].status, "provider_failed");

    let usage = novel_application::WorkflowRepository::run_usage(kernel.as_ref(), &run_id)
        .await
        .unwrap();
    assert_eq!(usage.calls, 2);
    assert_eq!(usage.known_calls, 1);
    assert_eq!(usage.unknown_calls, 1);
    assert_eq!(usage.prompt_tokens, 120);
    assert_eq!(usage.completion_tokens, 80);
}

/// 取消语义不能被观测记录破坏：取消后不再继续调度。
#[tokio::test]
async fn cancel_stops_the_run_without_faking_usage() {
    let llm = Arc::new(ScriptedFailingLlm::new(vec![Script::Fail(
        LlmError::Cancelled,
    )]));
    let (_dir, kernel, runtime) = runtime_for(llm.clone(), "sync-a1-cancel").await;
    let (novel_id, chapter_id) = seed_novel(&runtime).await;
    let run_id = open_run(&runtime, &novel_id, &chapter_id, "sync-a1-cancel-open").await;
    tick_until_stop(&runtime, &novel_id, &run_id, 20).await;

    let detail = runtime
        .director
        .get_run_detail(&novel_id, &run_id)
        .await
        .unwrap();
    assert_eq!(detail.status, "cancelled");
    let rows = invocations(kernel.as_ref(), &run_id).await;
    assert_eq!(rows.len(), 1, "取消的物理请求也要留下记录");
    assert_eq!(rows[0].status, "cancelled");
    assert_eq!(
        (rows[0].prompt_tokens, rows[0].completion_tokens),
        (None, None),
        "取消不能伪造用量"
    );
    assert_eq!(llm.calls(), 1);
    // 取消可能发生在请求已经发出之后，因此不能断言未计费。
    let usage = novel_application::WorkflowRepository::run_usage(kernel.as_ref(), &run_id)
        .await
        .unwrap();
    assert_eq!(usage.unknown_calls, 1);
    assert_eq!(usage.unbilled_calls, 0);

    // 取消后继续 tick 不应再发起请求。
    let _ = tokio::time::timeout(Duration::from_millis(50), runtime.worker.tick()).await;
    assert_eq!(llm.calls(), 1);
}

/// 旧 Run（没有新增列语义）的历史记录仍可读，分类退回按用量判断。
#[tokio::test]
async fn legacy_succeeded_rows_still_count_as_known() {
    let llm = Arc::new(ScriptedFailingLlm::new(vec![Script::Text(story_macro())]));
    let (_dir, kernel, runtime) = runtime_for(llm, "sync-a1-legacy").await;
    let novel = runtime.director.create_novel("旧记录", "").await.unwrap();
    NovelRepository::create_chapter(
        kernel.as_ref(),
        NewChapter {
            id: "legacy-ch".into(),
            novel_id: novel.id.clone(),
            title: "第一章".into(),
            sort_order: 1,
            volume_id: None,
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO llm_invocation(id, request_id, run_id, step_id, provider, model, input_summary,
            started_at, finished_at, prompt_tokens, completion_tokens, status)
         VALUES ('legacy-inv', 'legacy-req', 'legacy-run', 'draft', 'mock', 'mock', 'x', 1, 2, 10, 5, 'succeeded')",
    )
    .execute(kernel.pool())
    .await
    .unwrap();

    let usage = novel_application::WorkflowRepository::run_usage(kernel.as_ref(), "legacy-run")
        .await
        .unwrap();
    assert_eq!(usage.calls, 1);
    assert_eq!(usage.known_calls, 1);
    assert_eq!(usage.unknown_calls, 0);
    assert_eq!(usage.prompt_tokens, 10);
    assert_eq!(usage.completion_tokens, 5);
}
