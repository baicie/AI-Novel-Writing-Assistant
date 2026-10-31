//! R2.6-E：Ollama 无密钥的完整生成链路（本地 Mock）。

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use axum::Router;
use axum::body::Bytes;
use axum::http::{HeaderMap, Method, Uri};
use axum::routing::any;
use novel_llm::{LlmProvider, LlmRequest, OllamaProvider};
use tokio_util::sync::CancellationToken;

const OLLAMA_JSON: &str = "{\"message\":{\"content\":\"开场白\"},\"prompt_eval_count\":11,\"eval_count\":7,\"done\":true}";

async fn serve(hits: Arc<AtomicUsize>) -> String {
    let app = Router::new().fallback(any(
        move |_method: Method, _uri: Uri, _headers: HeaderMap, _body: Bytes| {
            let hits = hits.clone();
            async move {
                hits.fetch_add(1, Ordering::SeqCst);
                ([("content-type", "application/json")], OLLAMA_JSON)
            }
        },
    ));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    tokio::time::sleep(Duration::from_millis(80)).await;
    format!("http://{address}")
}

fn request(endpoint: &str) -> LlmRequest {
    LlmRequest {
        request_id: "req-ollama".into(),
        model: "llama3".into(),
        prompt: "写一句开场".into(),
        endpoint: endpoint.into(),
        // 本地 Ollama 场景：没有密钥。
        api_key: String::new(),
        allow_localhost_http: true,
        max_output_tokens: Some(32),
    }
}

#[tokio::test]
async fn keyless_local_ollama_generates_and_reports_usage() {
    let hits = Arc::new(AtomicUsize::new(0));
    let endpoint = serve(hits.clone()).await;
    let provider = OllamaProvider::new();
    let (sender, mut receiver) = tokio::sync::mpsc::channel::<String>(8);
    let drain = tokio::spawn(async move { while receiver.recv().await.is_some() {} });

    let response = provider
        .generate(request(&endpoint), sender, CancellationToken::new())
        .await
        .expect("本机 Ollama 无密钥应当可以生成");
    let _ = tokio::time::timeout(Duration::from_secs(2), drain).await;

    assert_eq!(response.text, "开场白");
    assert_eq!(response.usage.prompt_tokens, Some(11));
    assert_eq!(response.usage.completion_tokens, Some(7));
    assert_eq!(hits.load(Ordering::SeqCst), 1, "只应发出一次请求");
}

#[tokio::test]
async fn cancelled_keyless_ollama_does_not_send_request() {
    let hits = Arc::new(AtomicUsize::new(0));
    let endpoint = serve(hits.clone()).await;
    let provider = OllamaProvider::new();
    let (sender, mut receiver) = tokio::sync::mpsc::channel::<String>(8);
    let drain = tokio::spawn(async move { while receiver.recv().await.is_some() {} });

    let cancel = CancellationToken::new();
    cancel.cancel();
    let result = provider.generate(request(&endpoint), sender, cancel).await;
    let _ = tokio::time::timeout(Duration::from_secs(2), drain).await;

    assert!(result.is_err(), "取消后不应返回成功");
    assert_eq!(hits.load(Ordering::SeqCst), 0, "取消后不应发出请求");
}
