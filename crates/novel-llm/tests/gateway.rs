use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use novel_llm::{LlmProvider, LlmRequest, OpenAiCompatibleProvider, ProviderPolicy};
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

async fn serve_statuses(statuses: Vec<u16>, body: &'static str) -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_for_task = calls.clone();
    tokio::spawn(async move {
        for status in statuses {
            let (mut stream, _) = listener.accept().await.unwrap();
            read_request(&mut stream).await;
            calls_for_task.fetch_add(1, Ordering::SeqCst);
            let response = format!(
                "HTTP/1.1 {status} X\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).await.unwrap();
        }
    });
    (format!("http://{address}/v1/chat/completions"), calls)
}

fn request(endpoint: &str, budget: u64) -> LlmRequest {
    LlmRequest {
        request_id: "req-test".into(),
        model: "test-model".into(),
        prompt: "写一段测试正文".into(),
        endpoint: endpoint.into(),
        api_key: "sk-test".into(),
        allow_localhost_http: true,
        max_output_tokens: Some(budget),
    }
}

#[tokio::test]
async fn streams_openai_compatible_sse_chunks() {
    let body = concat!(
        "data: {\"choices\":[{\"delta\":{\"content\":\"林\"}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{\"content\":\"动\"}}]}\n\n",
        "data: {\"usage\":{\"completion_tokens\":2}}\n\n",
        "data: [DONE]\n\n"
    );
    let (endpoint, calls) = serve_statuses(vec![200], body).await;
    let provider = OpenAiCompatibleProvider::with_policy(ProviderPolicy {
        timeout: Duration::from_secs(2),
        max_attempts: 1,
        retry_backoff: Duration::ZERO,
        ..ProviderPolicy::default()
    });
    let (sender, mut receiver) = tokio::sync::mpsc::channel(64);
    let response = provider
        .generate(request(&endpoint, 128), sender, CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(response.text, "林动");
    assert_eq!(response.usage.completion_tokens, Some(2));
    assert_eq!(receiver.recv().await.as_deref(), Some("林"));
    assert_eq!(receiver.recv().await.as_deref(), Some("动"));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn retries_5xx_once_then_streams() {
    let body = "data: {\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}\n\ndata: [DONE]\n\n";
    let (endpoint, calls) = serve_statuses(vec![503, 200], body).await;
    let provider = OpenAiCompatibleProvider::with_policy(ProviderPolicy {
        timeout: Duration::from_secs(2),
        max_attempts: 2,
        retry_backoff: Duration::ZERO,
        ..ProviderPolicy::default()
    });
    let (sender, _receiver) = tokio::sync::mpsc::channel(64);
    let response = provider
        .generate(request(&endpoint, 128), sender, CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(response.text, "ok");
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn incomplete_stream_is_rejected_without_retry() {
    let body = "data: {\"choices\":[{\"delta\":{\"content\":\"截断\"}}]}\n\n";
    let (endpoint, calls) = serve_statuses(vec![200], body).await;
    let provider = OpenAiCompatibleProvider::with_policy(ProviderPolicy {
        timeout: Duration::from_secs(2),
        max_attempts: 2,
        retry_backoff: Duration::ZERO,
        ..ProviderPolicy::default()
    });
    let (sender, _receiver) = tokio::sync::mpsc::channel(8);
    let error = provider
        .generate(request(&endpoint, 128), sender, CancellationToken::new())
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        novel_llm::LlmError::Unavailable { billed: true, .. }
    ));
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "truncated stream must not be retried"
    );
}

#[tokio::test]
async fn rejects_unbounded_or_zero_output_budget_before_network() {
    let provider = OpenAiCompatibleProvider::with_policy(ProviderPolicy::default());
    let (sender, _receiver) = tokio::sync::mpsc::channel(64);
    let error = provider
        .generate(
            request("https://api.example.com/v1/chat/completions", 0),
            sender,
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
    assert!(matches!(error, novel_llm::LlmError::BudgetExceeded(_)));
    assert!(!error.retryable_without_new_charge());
}
