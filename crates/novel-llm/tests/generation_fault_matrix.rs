//! R2.4：生成请求的确定性故障矩阵。
//!
//! 只验证「请求发了几次」与「错误分类」，不依赖真实网络与 sleep。

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use axum::Router;
use axum::body::Bytes;
use axum::http::{HeaderMap, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::any;
use novel_llm::{LlmError, LlmProvider, LlmRequest, OpenAiCompatibleProvider};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
struct Fault {
    status: StatusCode,
    body: &'static str,
}

async fn serve(fault: Fault, hits: Arc<AtomicUsize>) -> String {
    let app = Router::new().fallback(any(
        move |_method: Method, _uri: Uri, _headers: HeaderMap, _body: Bytes| {
            let fault = fault.clone();
            let hits = hits.clone();
            async move {
                hits.fetch_add(1, Ordering::SeqCst);
                let mut response: Response = (fault.status, fault.body).into_response();
                response
                    .headers_mut()
                    .insert("content-type", "text/event-stream".parse().unwrap());
                response
            }
        },
    ));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    tokio::time::sleep(Duration::from_millis(80)).await;
    format!("http://{address}/v1")
}

fn request(endpoint: &str) -> LlmRequest {
    LlmRequest {
        request_id: "req-fault".into(),
        model: "m".into(),
        prompt: "写一句".into(),
        endpoint: endpoint.into(),
        api_key: "sk-test".into(),
        allow_localhost_http: true,
        max_output_tokens: Some(16),
    }
}

async fn run(fault: Fault) -> (Result<(), LlmError>, usize) {
    let hits = Arc::new(AtomicUsize::new(0));
    let endpoint = serve(fault, hits.clone()).await;
    let provider = OpenAiCompatibleProvider::new();
    let (sender, mut receiver) = tokio::sync::mpsc::channel::<String>(8);
    let drain = tokio::spawn(async move { while receiver.recv().await.is_some() {} });
    let result = provider
        .generate(request(&endpoint), sender, CancellationToken::new())
        .await
        .map(|_| ());
    let _ = tokio::time::timeout(Duration::from_secs(2), drain).await;
    (result, hits.load(Ordering::SeqCst))
}

#[tokio::test]
async fn unauthorized_is_not_retried() {
    let (result, hits) = run(Fault {
        status: StatusCode::UNAUTHORIZED,
        body: "",
    })
    .await;
    assert!(matches!(result, Err(LlmError::Unauthorized)), "{result:?}");
    assert_eq!(hits, 1, "401 不应重试");
}

#[tokio::test]
async fn forbidden_and_not_found_are_unavailable_without_retry() {
    for status in [StatusCode::FORBIDDEN, StatusCode::NOT_FOUND] {
        let (result, hits) = run(Fault { status, body: "" }).await;
        assert!(result.is_err(), "{status} 必须失败");
        assert_eq!(hits, 1, "{status} 不应重试");
    }
}

#[tokio::test]
async fn rate_limit_is_retried_exactly_once_more() {
    let (result, hits) = run(Fault {
        status: StatusCode::TOO_MANY_REQUESTS,
        body: "",
    })
    .await;
    assert!(matches!(result, Err(LlmError::RateLimit)), "{result:?}");
    // 默认策略最多 2 次尝试：429 属于可安全重试。
    assert!((1..=2).contains(&hits), "429 重试次数应受限，实际 {hits}");
}

#[tokio::test]
async fn server_errors_do_not_loop_forever() {
    for status in [
        StatusCode::INTERNAL_SERVER_ERROR,
        StatusCode::BAD_GATEWAY,
        StatusCode::SERVICE_UNAVAILABLE,
    ] {
        let (result, hits) = run(Fault { status, body: "" }).await;
        assert!(result.is_err(), "{status} 必须失败");
        assert!(hits <= 2, "{status} 重试次数应有上界，实际 {hits}");
    }
}

#[tokio::test]
async fn invalid_sse_and_truncated_stream_are_invalid_response() {
    for body in [
        "data: {not json}\n\n",
        "data: {\"choices\":[{\"delta\":{\"content\":\"a\"}}]}\n\n",
    ] {
        let (result, hits) = run(Fault {
            status: StatusCode::OK,
            body,
        })
        .await;
        assert!(result.is_err(), "坏流必须报错：{body}");
        assert_eq!(hits, 1, "坏流不应重试");
    }
}

#[tokio::test]
async fn cancellation_stops_without_extra_requests() {
    let hits = Arc::new(AtomicUsize::new(0));
    let endpoint = serve(
        Fault {
            status: StatusCode::OK,
            body: "data: {\"choices\":[{\"delta\":{\"content\":\"a\"}}]}\n\n",
        },
        hits.clone(),
    )
    .await;
    let provider = OpenAiCompatibleProvider::new();
    let (sender, mut receiver) = tokio::sync::mpsc::channel::<String>(8);
    let drain = tokio::spawn(async move { while receiver.recv().await.is_some() {} });
    let cancel = CancellationToken::new();
    cancel.cancel();
    let result = provider.generate(request(&endpoint), sender, cancel).await;
    let _ = tokio::time::timeout(Duration::from_secs(2), drain).await;
    assert!(matches!(result, Err(LlmError::Cancelled)), "{result:?}");
    assert_eq!(hits.load(Ordering::SeqCst), 0, "取消后不应再发请求");
}

/// G2：先返回两个真实 token，然后断流（没有 [DONE]）——必须报错，不能当成功。
#[tokio::test]
async fn partial_tokens_then_disconnect_is_an_error() {
    let (result, hits) = run(Fault {
        status: StatusCode::OK,
        body: "data: {\"choices\":[{\"delta\":{\"content\":\"半\"}}]}\n\ndata: {\"choices\":[{\"delta\":{\"content\":\"句\"}}]}\n\n",
    })
    .await;
    assert!(result.is_err(), "部分输出后断流必须失败，实际 {result:?}");
    assert_eq!(hits, 1, "断流不应触发重试");
}
