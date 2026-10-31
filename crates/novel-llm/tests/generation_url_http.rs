//! 真实 HTTP 断言：生成请求的 METHOD / PATH / Authorization / BODY。
//!
//! 用 axum 起本地服务器，专门锁住「baseURL 与操作路径分离」：
//! 内置 Provider 只配置 /v1，生成时必须补 /chat/completions；历史配置已写全路径时不得重复追加。

use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::body::Bytes;
use axum::http::{HeaderMap, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::any;
use novel_llm::{LlmProvider, LlmRequest, OllamaProvider, OpenAiCompatibleProvider};
use tokio_util::sync::CancellationToken;

const SSE_BODY: &str = "data: {\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}\n\ndata: [DONE]\n\n";

#[derive(Debug, Clone, Default)]
struct Captured {
    method: Method,
    path: String,
    authorization: Option<String>,
    body: String,
}

type Store = Arc<Mutex<Vec<Captured>>>;

async fn record(
    store: Store,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let authorization = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    store.lock().unwrap().push(Captured {
        method,
        path: uri.path().to_string(),
        authorization,
        body: String::from_utf8_lossy(&body).to_string(),
    });
    (
        StatusCode::OK,
        [("content-type", "text/event-stream")],
        SSE_BODY,
    )
        .into_response()
}

async fn serve() -> (String, Store) {
    let store: Store = Arc::new(Mutex::new(Vec::new()));
    let state = store.clone();
    let app = Router::new().fallback(any(
        move |method: Method, uri: Uri, headers: HeaderMap, body: Bytes| {
            let state = state.clone();
            async move { record(state, method, uri, headers, body).await }
        },
    ));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    tokio::time::sleep(Duration::from_millis(80)).await;
    (format!("http://{address}"), store)
}

fn request_to(endpoint: &str) -> LlmRequest {
    LlmRequest {
        request_id: "req-url".into(),
        model: "deepseek-chat".into(),
        prompt: "写一句开场".into(),
        endpoint: endpoint.into(),
        api_key: "sk-secret".into(),
        allow_localhost_http: true,
        max_output_tokens: Some(64),
    }
}

async fn generate_openai(endpoint: &str) -> Result<novel_llm::LlmResponse, novel_llm::LlmError> {
    let provider = OpenAiCompatibleProvider::new();
    let (sender, mut receiver) = tokio::sync::mpsc::channel::<String>(8);
    let drain = tokio::spawn(async move { while receiver.recv().await.is_some() {} });
    let result = provider
        .generate(request_to(endpoint), sender, CancellationToken::new())
        .await;
    let _ = tokio::time::timeout(Duration::from_secs(2), drain).await;
    result
}

async fn generate_ollama(endpoint: &str) -> Result<novel_llm::LlmResponse, novel_llm::LlmError> {
    let provider = OllamaProvider::new();
    let (sender, mut receiver) = tokio::sync::mpsc::channel::<String>(8);
    let drain = tokio::spawn(async move { while receiver.recv().await.is_some() {} });
    let result = provider
        .generate(request_to(endpoint), sender, CancellationToken::new())
        .await;
    let _ = tokio::time::timeout(Duration::from_secs(2), drain).await;
    result
}

fn only(store: &Store) -> Captured {
    let guard = store.lock().unwrap();
    assert_eq!(
        guard.len(),
        1,
        "应当只发出一次请求，实际 {} 次",
        guard.len()
    );
    guard[0].clone()
}

#[tokio::test]
async fn builtin_v1_base_url_posts_to_chat_completions() {
    let (base, store) = serve().await;
    generate_openai(&format!("{base}/v1")).await.unwrap();
    let captured = only(&store);
    assert_eq!(captured.method, Method::POST);
    assert_eq!(captured.path, "/v1/chat/completions");
    assert_eq!(captured.authorization.as_deref(), Some("Bearer sk-secret"));
    assert!(captured.body.contains("\"model\":\"deepseek-chat\""));
    assert!(captured.body.contains("\"stream\":true"));
    assert!(captured.body.contains("写一句开场"));
}

#[tokio::test]
async fn gateway_prefix_is_kept_in_generation_path() {
    let (base, store) = serve().await;
    generate_openai(&format!("{base}/proxy/v1")).await.unwrap();
    assert_eq!(only(&store).path, "/proxy/v1/chat/completions");
}

#[tokio::test]
async fn legacy_full_endpoint_is_not_double_appended() {
    let (base, store) = serve().await;
    generate_openai(&format!("{base}/v1/chat/completions"))
        .await
        .unwrap();
    assert_eq!(only(&store).path, "/v1/chat/completions");
}

#[tokio::test]
async fn trailing_slash_base_url_still_resolves() {
    let (base, store) = serve().await;
    generate_openai(&format!("{base}/v1/")).await.unwrap();
    assert_eq!(only(&store).path, "/v1/chat/completions");
}

#[tokio::test]
async fn ollama_native_base_uses_api_chat() {
    let (base, store) = serve().await;
    // Ollama 读 JSON 而不是 SSE：这里只断言发出请求的路径与方法。
    let _ = generate_ollama(&base).await;
    let captured = only(&store);
    assert_eq!(captured.method, Method::POST);
    assert_eq!(captured.path, "/api/chat");
}

#[tokio::test]
async fn ollama_v1_base_uses_openai_path() {
    let (base, store) = serve().await;
    let _ = generate_ollama(&format!("{base}/v1")).await;
    assert_eq!(only(&store).path, "/v1/chat/completions");
}

/// 未知协议（例如 Anthropic 只读适配器）不得被当作 OpenAI 兼容协议发送请求。
#[tokio::test]
async fn unknown_protocol_is_rejected_without_sending_request() {
    use novel_llm::ProtocolRouterProvider;
    let (base, store) = serve().await;
    let router = ProtocolRouterProvider::new();
    let (sender, mut receiver) = tokio::sync::mpsc::channel::<String>(8);
    let drain = tokio::spawn(async move { while receiver.recv().await.is_some() {} });
    let error = router
        .generate_with_protocol(
            request_to(&format!("{base}/v1")),
            "anthropic-messages",
            None,
            sender,
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
    let _ = tokio::time::timeout(Duration::from_secs(2), drain).await;
    assert!(
        error.to_string().contains("anthropic-messages"),
        "错误要说明协议不支持：{error}"
    );
    assert!(
        store.lock().unwrap().is_empty(),
        "不支持的协议不能发出任何请求"
    );
}
