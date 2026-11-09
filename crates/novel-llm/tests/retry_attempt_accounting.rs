//! Sync-A1-R2 Phase 3：Provider 内部重试与「调用记录条数」的真实关系。
//!
//! `llm_invocation` 每次 `generate` 只写一行，但 Provider 在 `max_attempts > 1`
//! 时可能发出多次 HTTP 请求。本套件锁定**当前真实保证**，防止文档或界面
//! 继续宣称尚未实现的逐 HTTP 请求精度：
//!
//!   * 记录粒度 = 一次逻辑调用（一步骤里的首次请求或一次修复请求）；
//!   * 一个逻辑调用最多产生 2 次 HTTP 请求（默认 `max_attempts = 2`）；
//!   * 只有可安全重试的错误才会重发：429 与 `billed: false` 的 5xx；
//!   * 计费相关的重发（429）本身不增加费用；5xx（billed: false）时服务端
//!     尚未开始流式应答。
//!
//! 这些是 `ProviderPolicy` 的既有行为，本补丁没有改变任何重试条件或次数。

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use novel_llm::{LlmError, LlmProvider, LlmRequest, OpenAiCompatibleProvider, ProviderPolicy};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

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

/// 按顺序应答若干状态码；每次应答前计数一次真实 HTTP 请求。
async fn serve(statuses: Vec<u16>, body: &'static str) -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    tokio::spawn(async move {
        for status in statuses {
            let (mut stream, _) = listener.accept().await.unwrap();
            read_request(&mut stream).await;
            counter.fetch_add(1, Ordering::SeqCst);
            let response = format!(
                "HTTP/1.1 {status} X\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).await.unwrap();
        }
    });
    (format!("http://{address}/v1/chat/completions"), calls)
}

fn request(endpoint: &str) -> LlmRequest {
    LlmRequest {
        request_id: "req-attempt".into(),
        model: "test-model".into(),
        prompt: "写一段测试正文".into(),
        endpoint: endpoint.into(),
        api_key: "sk-test".into(),
        allow_localhost_http: true,
        max_output_tokens: Some(128),
    }
}

fn policy(max_attempts: u8) -> ProviderPolicy {
    ProviderPolicy {
        timeout: Duration::from_secs(2),
        max_attempts,
        retry_backoff: Duration::ZERO,
        ..ProviderPolicy::default()
    }
}

const OK_BODY: &str = "data: {\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}\n\ndata: [DONE]\n\n";

/// 默认策略允许多次尝试，所以一次逻辑调用最多对应 2 次真实 HTTP 请求。
#[tokio::test]
async fn one_logical_call_can_be_two_http_requests_on_retryable_failure() {
    let (endpoint, calls) = serve(vec![503, 200], OK_BODY).await;
    let provider = OpenAiCompatibleProvider::with_policy(policy(2));
    let (sender, _rx) = tokio::sync::mpsc::channel(64);
    let response = provider
        .generate(request(&endpoint), sender, CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(response.text, "ok");
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "一次逻辑调用在可重试失败后确实发出了 2 次 HTTP 请求"
    );
}

/// 5xx 在流开始前返回，服务端尚未计费，所以允许重发。
#[tokio::test]
async fn pre_stream_5xx_retries_without_billing() {
    let (endpoint, calls) = serve(vec![502, 200], OK_BODY).await;
    let provider = OpenAiCompatibleProvider::with_policy(policy(2));
    let (sender, _rx) = tokio::sync::mpsc::channel(64);
    provider
        .generate(request(&endpoint), sender, CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

/// 429 重试后仍然失败：最终只返回一个错误，但真实请求已经发生 2 次。
/// 记录层只有一条记录——这正是「逻辑调用」粒度，文档不能宣称逐请求精度。
#[tokio::test]
async fn exhausted_rate_limit_reports_one_error_after_two_requests() {
    let (endpoint, calls) = serve(vec![429, 429], "").await;
    let provider = OpenAiCompatibleProvider::with_policy(policy(2));
    let (sender, _rx) = tokio::sync::mpsc::channel(64);
    let error = provider
        .generate(request(&endpoint), sender, CancellationToken::new())
        .await
        .unwrap_err();
    assert_eq!(error, LlmError::RateLimit);
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "两次 429 都真实发生，但调用层只会记录一条"
    );
    // 429 不产生费用，所以这条唯一记录是 unbilled 而不是 unknown。
    assert!(matches!(error, LlmError::RateLimit));
}

/// 流已经开始后再断开：不能重发，否则同一段内容会被重复计费。
#[tokio::test]
async fn mid_stream_break_is_not_retried() {
    let truncated = "data: {\"choices\":[{\"delta\":{\"content\":\"截断\"}}]}\n\n";
    let (endpoint, calls) = serve(vec![200, 200], truncated).await;
    let provider = OpenAiCompatibleProvider::with_policy(policy(2));
    let (sender, _rx) = tokio::sync::mpsc::channel(8);
    let error = provider
        .generate(request(&endpoint), sender, CancellationToken::new())
        .await
        .unwrap_err();
    assert!(matches!(error, LlmError::Unavailable { billed: true, .. }));
    assert_eq!(calls.load(Ordering::SeqCst), 1, "流开始后断开不得重发");
}

/// `max_attempts = 1` 时与 HTTP 请求一一对应——M1 验收链路使用的就是这种配置。
#[tokio::test]
async fn single_attempt_policy_maps_one_to_one() {
    let (endpoint, calls) = serve(vec![503], "").await;
    let provider = OpenAiCompatibleProvider::with_policy(policy(1));
    let (sender, _rx) = tokio::sync::mpsc::channel(8);
    provider
        .generate(request(&endpoint), sender, CancellationToken::new())
        .await
        .unwrap_err();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
