//! Sync-A1-R2 Phase 4：PR #29 的 Provider / 长流 / 取消改动与调用记账的兼容性。
//!
//! PR #29 调整了 `ProviderPolicy`（长响应、首字节、流空闲超时）、Reviewer 与
//! Run 输入冻结，但没有触碰记账路径。本套件用真实 `OpenAiCompatibleProvider`
//! 驱动完整 worker 链路，锁定四种生成结果在记账上的结论：
//!
//!   * 长流式响应成功 → 一条 known 记录；
//!   * 流空闲超时       → 一条 unknown 记录（可能已计费）；
//!   * 用户取消         → 一条 unknown 记录，且 run 进入 cancelled；
//!   * 上游 error 事件  → 一条 unknown 记录（服务端已受理）。
//!
//! 这里不使用 mock provider，确保 PR #29 的传输层改动被真实覆盖。

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use novel_application::{
    ModelSecrets, NovelRepository, OpenRunInput, PortError, Runtime, RuntimeDeps, TickOutcome,
};
use novel_llm::{LlmProvider, OpenAiCompatibleProvider, ProviderPolicy};
use novel_storage_sqlite::SqliteKernel;
use novel_testkit::{FakeClock, SequentialIds};
use sqlx::Row;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

struct FixtureSecrets;

#[async_trait]
impl ModelSecrets for FixtureSecrets {
    async fn api_key(&self, _provider: &str) -> Result<Option<String>, PortError> {
        Ok(Some("sk-test".into()))
    }
}

async fn read_request(stream: &mut tokio::net::TcpStream) {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 1024];
    loop {
        let read = stream.read(&mut chunk).await.unwrap();
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
        if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
}

/// 启动一个对所有连接返回同一应答的 SSE 服务端。
///
/// 必须持续接受连接：结构化步骤解析失败后 worker 会再发一次修复请求，
/// 只应答一次会让第二次请求直接撞上关闭的端口，把真实的传输结果掩盖掉。
async fn serve_repeating(body: String, status: u16) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                break;
            };
            let body = body.clone();
            tokio::spawn(async move {
                read_request(&mut stream).await;
                let response = format!(
                    "HTTP/1.1 {status} X\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.flush().await;
            });
        }
    });
    format!("http://{address}/v1/chat/completions")
}

/// 慢速服务端：先发一段正文，然后长时间不发送任何数据（用于触发流空闲超时）。
async fn serve_stalling(prefix: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        if let Ok((mut stream, _)) = listener.accept().await {
            read_request(&mut stream).await;
            let head =
                "HTTP/1.1 200 X\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n";
            let _ = stream.write_all(head.as_bytes()).await;
            let _ = stream.write_all(prefix.as_bytes()).await;
            let _ = stream.flush().await;
            // 远超测试里的 stream_idle_timeout，直到测试结束。
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
    });
    format!("http://{address}/v1/chat/completions")
}

fn sse(deltas: &[&str]) -> String {
    let mut body = String::new();
    for delta in deltas {
        body.push_str(&format!(
            "data: {{\"choices\":[{{\"delta\":{{\"content\":\"{delta}\"}}}}]}}\n\n"
        ));
    }
    body.push_str("data: {\"usage\":{\"prompt_tokens\":40,\"completion_tokens\":60}}\n\n");
    body.push_str("data: [DONE]\n\n");
    body
}

struct Harness {
    _dir: tempfile::TempDir,
    kernel: Arc<SqliteKernel>,
    runtime: Runtime,
    novel_id: String,
    chapter_id: String,
    endpoint: String,
    calls: Arc<AtomicUsize>,
}

/// 计数包装：统计 worker 发起的逻辑调用次数。
struct CountingProvider {
    inner: OpenAiCompatibleProvider,
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl LlmProvider for CountingProvider {
    fn capabilities(&self) -> novel_llm::ProviderCapabilities {
        self.inner.capabilities()
    }

    async fn generate(
        &self,
        request: novel_llm::LlmRequest,
        tokens: tokio::sync::mpsc::Sender<String>,
        cancel: CancellationToken,
    ) -> Result<novel_llm::LlmResponse, novel_llm::LlmError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.inner.generate(request, tokens, cancel).await
    }
}

async fn harness(endpoint: String, policy: ProviderPolicy) -> Harness {
    let dir = tempfile::tempdir().unwrap();
    let kernel = Arc::new(
        SqliteKernel::open(&dir.path().join("r2.sqlite"))
            .await
            .unwrap(),
    );
    let calls = Arc::new(AtomicUsize::new(0));

    // story_macro 是第一个结构化步骤，直接走真实 HTTP 传输。
    NovelRepository::create_novel(
        kernel.as_ref(),
        novel_application::NewNovel {
            id: "novel-r2".into(),
            title: "兼容复审".into(),
            description: "验证传输与记账".into(),
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    NovelRepository::create_chapter(
        kernel.as_ref(),
        novel_application::NewChapter {
            id: "ch-r2".into(),
            novel_id: "novel-r2".into(),
            title: "第一章".into(),
            sort_order: 1,
            volume_id: None,
            now_ms: 1,
        },
    )
    .await
    .unwrap();

    let llm: Arc<dyn LlmProvider> = Arc::new(CountingProvider {
        inner: OpenAiCompatibleProvider::with_policy(policy),
        calls: calls.clone(),
    });
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
        owner: "r2-owner".into(),
        lease_ms: 60_000,
        concurrency: 1,
    });

    Harness {
        _dir: dir,
        kernel,
        runtime,
        novel_id: "novel-r2".into(),
        chapter_id: "ch-r2".into(),
        endpoint,
        calls,
    }
}

impl Harness {
    async fn open_run(&self) -> String {
        let mut prompts = BTreeMap::new();
        prompts.insert("chapter_draft".into(), "chapter-draft-v3".into());
        prompts.insert("chapter_repair".into(), "chapter-repair-v2".into());
        self.runtime
            .director
            .open_run(OpenRunInput {
                command_id: "r2-open".into(),
                novel_id: self.novel_id.clone(),
                target_chapter_id: self.chapter_id.clone(),
                provider: "mock".into(),
                credential_ref: Some("mock".into()),
                model: "mock-model".into(),
                prompt_versions: prompts,
                token_budget: Some(2_048),
                context_token_budget: None,
                max_automatic_retries: 0,
                repair_budget: Some(0),
                endpoint: self.endpoint.clone(),
                allow_localhost_http: true,
                step_routes: Default::default(),
            })
            .await
            .unwrap()
            .run_id
    }

    async fn invocations(&self, run_id: &str) -> Vec<(String, String, String)> {
        let rows = sqlx::query(
            "SELECT step_id, status, usage_state FROM llm_invocation WHERE run_id = ? ORDER BY rowid",
        )
        .bind(run_id)
        .fetch_all(self.kernel.pool())
        .await
        .unwrap();
        rows.into_iter()
            .map(|row| {
                (
                    row.try_get("step_id").unwrap(),
                    row.try_get("status").unwrap(),
                    row.try_get("usage_state").unwrap(),
                )
            })
            .collect()
    }
}

fn policy_with_idle(idle: Duration) -> ProviderPolicy {
    ProviderPolicy {
        request_timeout: Some(Duration::from_secs(10)),
        first_byte_timeout: Duration::from_secs(10),
        stream_idle_timeout: idle,
        max_attempts: 1,
        retry_backoff: Duration::ZERO,
        ..ProviderPolicy::default()
    }
}

/// 长流式响应：PR #29 的 request_timeout / idle_timeout 下正常完成，记一条 known。
#[tokio::test]
async fn long_stream_completes_and_records_known_usage() {
    let body = sse(&["长", "流", "正", "文"]);
    let endpoint = serve_repeating(body, 200).await;
    let harness = harness(endpoint, policy_with_idle(Duration::from_secs(5))).await;
    let run_id = harness.open_run().await;

    let outcome = harness.runtime.worker.tick().await.unwrap();
    assert!(
        matches!(outcome, TickOutcome::Advanced { ref status, .. } if status != "cancelled"),
        "长流不应被取消: {outcome:?}"
    );

    let rows = harness.invocations(&run_id).await;
    // 首次请求拿到的是正文而非 JSON，因此会触发一次格式修复请求；
    // 两次都是真实的物理调用，各自记一条。
    assert_eq!(rows.len(), 2, "首次请求与修复请求分别记账: {rows:?}");
    assert_eq!(rows[0].0, "story_macro");
    assert_eq!(rows[0].2, "known", "首次请求本身成功返回，用量已确认");
    assert_eq!(rows[1].0, "story_macro");
}

/// 流空闲超时：请求已发出且已收到部分内容，费用未知。
#[tokio::test]
async fn stream_idle_timeout_records_unknown_usage() {
    let endpoint =
        serve_stalling("data: {\"choices\":[{\"delta\":{\"content\":\"半\"}}]}\n\n").await;
    let harness = harness(endpoint, policy_with_idle(Duration::from_millis(150))).await;
    let run_id = harness.open_run().await;

    let outcome = harness.runtime.worker.tick().await.unwrap();
    assert!(
        matches!(outcome, TickOutcome::Advanced { ref status, .. } if status == "paused"),
        "空闲超时应当暂停并等待作者处理: {outcome:?}"
    );

    let rows = harness.invocations(&run_id).await;
    assert_eq!(rows.len(), 1, "超时的物理请求必须留下记录");
    assert_eq!(rows[0].2, "unknown", "超时无法确认是否已计费");
    assert_eq!(harness.calls.load(Ordering::SeqCst), 1, "超时不得重发");

    let detail = harness
        .runtime
        .director
        .get_run_detail(&harness.novel_id, &run_id)
        .await
        .unwrap();
    assert_eq!(detail.usage.unknown_calls, 1);
    assert_eq!(detail.usage.known_calls, 0);
}

/// 上游 error 事件：服务端已受理，记 unknown 而不是「没花钱」。
#[tokio::test]
async fn provider_error_event_records_unknown_usage() {
    let body = "data: {\"error\":{\"message\":\"upstream overloaded\"}}\n\n".to_string();
    let endpoint = serve_repeating(body, 200).await;
    let harness = harness(endpoint, policy_with_idle(Duration::from_secs(5))).await;
    let run_id = harness.open_run().await;

    let outcome = harness.runtime.worker.tick().await.unwrap();
    assert!(
        matches!(outcome, TickOutcome::Advanced { ref status, .. } if status == "paused"),
        "上游错误应暂停: {outcome:?}"
    );

    let rows = harness.invocations(&run_id).await;
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].2, "unknown",
        "服务端下发了 error 事件，说明请求已被受理"
    );
}

/// 用户取消：run 进入 cancelled，记录保持 unknown，取消后不再发起请求。
#[tokio::test]
async fn cancel_during_stream_keeps_unknown_usage() {
    let endpoint =
        serve_stalling("data: {\"choices\":[{\"delta\":{\"content\":\"半\"}}]}\n\n").await;
    let harness = harness(endpoint, policy_with_idle(Duration::from_secs(30))).await;
    let run_id = harness.open_run().await;

    let worker = harness.runtime.worker.clone();
    let run_for_cancel = run_id.clone();
    let tick = tokio::spawn(async move { worker.tick().await });
    for _ in 0..100 {
        if harness.calls.load(Ordering::SeqCst) >= 1 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    harness.runtime.worker.cancel_generation(&run_for_cancel);
    let outcome = tick.await.unwrap().unwrap();
    assert!(
        matches!(outcome, TickOutcome::Advanced { ref status, .. } if status == "cancelled"),
        "取消必须终止本次运行: {outcome:?}"
    );

    let rows = harness.invocations(&run_id).await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].2, "unknown", "取消可能发生在请求已发出之后");
    let calls_after_cancel = harness.calls.load(Ordering::SeqCst);

    // 取消后继续调度不应再调用模型。
    let _ = tokio::time::timeout(Duration::from_millis(100), harness.runtime.worker.tick()).await;
    assert_eq!(
        harness.calls.load(Ordering::SeqCst),
        calls_after_cancel,
        "取消后不得再发起请求"
    );
}
