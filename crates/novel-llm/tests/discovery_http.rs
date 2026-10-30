//! P16.2：模型发现的真实 HTTP 行为（本地 mock server，不产生任何真实模型调用）。

use std::time::Duration;

use novel_llm::{
    AnthropicDiscovery, DiscoveryError, DiscoveryRequest, OllamaDiscovery,
    OpenAiCompatibleDiscovery, ProviderModelDiscovery,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

struct Reply {
    status: u16,
    body: &'static str,
    delay: Duration,
}

fn reply(status: u16, body: &'static str) -> Reply {
    Reply {
        status,
        body,
        delay: Duration::ZERO,
    }
}

/// 起一个只回答固定响应的本地 HTTP 服务，返回 base_url 与「收到的请求头」句柄。
async fn mock(replies: Vec<Reply>) -> (String, tokio::task::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = tokio::spawn(async move {
        let mut captured = Vec::new();
        for reply in replies {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            let mut buffer = vec![0u8; 4096];
            let read = socket.read(&mut buffer).await.unwrap_or(0);
            captured.push(String::from_utf8_lossy(&buffer[..read]).to_string());
            if !reply.delay.is_zero() {
                tokio::time::sleep(reply.delay).await;
            }
            let response = format!(
                "HTTP/1.1 {} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                reply.status,
                reply.body.len(),
                reply.body
            );
            let _ = socket.write_all(response.as_bytes()).await;
            let _ = socket.flush().await;
        }
        captured
    });
    (format!("http://127.0.0.1:{port}/v1"), handle)
}

fn request(base_url: &str, key: &str, allow_local: bool) -> DiscoveryRequest {
    DiscoveryRequest::new(base_url, key, allow_local).with_timeout(Duration::from_millis(400))
}

#[tokio::test]
async fn openai_success_parses_and_sends_bearer() {
    let (base, handle) = mock(vec![reply(
        200,
        r#"{"data":[{"id":"deepseek-chat","context_window":64000}]}"#,
    )])
    .await;
    let outcome = OpenAiCompatibleDiscovery
        .discover(&request(&base, "sk-test", true), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(outcome.models.len(), 1);
    assert_eq!(outcome.models[0].id, "deepseek-chat");
    let captured = handle.await.unwrap();
    assert!(
        captured[0].contains("/v1/models"),
        "必须带上路径前缀：{}",
        captured[0]
    );
    assert!(captured[0].contains("authorization: Bearer sk-test"));
}

#[tokio::test]
async fn empty_list_is_not_an_error() {
    let (base, handle) = mock(vec![reply(200, r#"{"data":[]}"#)]).await;
    let outcome = OpenAiCompatibleDiscovery
        .discover(&request(&base, "k", true), CancellationToken::new())
        .await
        .unwrap();
    assert!(outcome.models.is_empty());
    handle.await.unwrap();
}

#[tokio::test]
async fn duplicate_ids_are_collapsed() {
    let (base, handle) = mock(vec![reply(
        200,
        r#"{"data":[{"id":"a"},{"id":"a"},{"id":"b"}]}"#,
    )])
    .await;
    let outcome = OpenAiCompatibleDiscovery
        .discover(&request(&base, "k", true), CancellationToken::new())
        .await
        .unwrap();
    let ids: Vec<&str> = outcome
        .models
        .iter()
        .map(|model| model.id.as_str())
        .collect();
    assert_eq!(ids, vec!["a", "b"]);
    handle.await.unwrap();
}

#[tokio::test]
async fn invalid_json_maps_to_invalid_response() {
    let (base, handle) = mock(vec![reply(200, "not json")]).await;
    let error = OpenAiCompatibleDiscovery
        .discover(&request(&base, "k", true), CancellationToken::new())
        .await
        .unwrap_err();
    assert_eq!(error.code(), "INVALID_RESPONSE");
    handle.await.unwrap();
}

#[tokio::test]
async fn http_status_codes_map_to_error_codes() {
    for (status, code) in [
        (401, "AUTH_FAILED"),
        (403, "FORBIDDEN"),
        (404, "NOT_SUPPORTED"),
        (429, "RATE_LIMITED"),
        (500, "NETWORK_ERROR"),
    ] {
        let (base, handle) = mock(vec![reply(status, r#"{"error":"x"}"#)]).await;
        let error = OpenAiCompatibleDiscovery
            .discover(&request(&base, "k", true), CancellationToken::new())
            .await
            .unwrap_err();
        assert_eq!(error.code(), code, "状态码 {status} 映射错误");
        handle.await.unwrap();
    }
}

#[tokio::test]
async fn redirects_are_not_followed() {
    let (base, handle) = mock(vec![reply(302, r#"{"data":[]}"#)]).await;
    let error = OpenAiCompatibleDiscovery
        .discover(&request(&base, "k", true), CancellationToken::new())
        .await
        .unwrap_err();
    assert_eq!(error.code(), "INVALID_RESPONSE");
    let captured = handle.await.unwrap();
    assert_eq!(captured.len(), 1, "被重定向后不应有第二次请求");
}

#[tokio::test]
async fn slow_server_maps_to_timeout() {
    let (base, handle) = mock(vec![Reply {
        status: 200,
        body: r#"{"data":[]}"#,
        delay: Duration::from_secs(2),
    }])
    .await;
    let error = OpenAiCompatibleDiscovery
        .discover(&request(&base, "k", true), CancellationToken::new())
        .await
        .unwrap_err();
    assert_eq!(error.code(), "TIMEOUT");
    handle.abort();
}

#[tokio::test]
async fn pre_cancelled_token_maps_to_cancelled() {
    let (base, handle) = mock(vec![Reply {
        status: 200,
        body: r#"{"data":[]}"#,
        delay: Duration::from_secs(1),
    }])
    .await;
    let cancel = CancellationToken::new();
    cancel.cancel();
    let error = OpenAiCompatibleDiscovery
        .discover(&request(&base, "k", true), cancel)
        .await
        .unwrap_err();
    assert_eq!(error.code(), "CANCELLED");
    handle.abort();
}

#[tokio::test]
async fn missing_key_does_not_send_authorization_header() {
    let (base, handle) = mock(vec![reply(200, r#"{"data":[{"id":"a"}]}"#)]).await;
    OpenAiCompatibleDiscovery
        .discover(&request(&base, "  ", true), CancellationToken::new())
        .await
        .unwrap();
    let captured = handle.await.unwrap();
    assert!(!captured[0].to_lowercase().contains("authorization:"));
}

#[tokio::test]
async fn local_http_requires_explicit_permission() {
    let (base, handle) = mock(vec![reply(200, r#"{"data":[]}"#)]).await;
    let error = OpenAiCompatibleDiscovery
        .discover(&request(&base, "k", false), CancellationToken::new())
        .await
        .unwrap_err();
    assert_eq!(
        error.code(),
        "INVALID_RESPONSE",
        "本地 http 地址未授权时必须直接拒绝"
    );
    handle.abort();
}

#[tokio::test]
async fn https_endpoint_passes_address_validation() {
    // 公网 https 地址不应被地址校验拦掉；这里用一个不存在的域名，只会失败在网络层。
    let error = OpenAiCompatibleDiscovery
        .discover(
            &request("https://no-such-host.invalid/v1", "k", false),
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
    assert_ne!(
        error.code(),
        "INVALID_RESPONSE",
        "https 公网地址不应被地址校验拦掉"
    );
}

#[tokio::test]
async fn loopback_needs_permission_even_over_https() {
    let error = OpenAiCompatibleDiscovery
        .discover(
            &request("https://127.0.0.1:9/v1", "k", false),
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
    assert_eq!(error.code(), "INVALID_RESPONSE", "回环地址必须显式允许");
}

#[tokio::test]
async fn anthropic_paginates_and_uses_api_key_header() {
    let (base, handle) = mock(vec![
        reply(200, r#"{"data":[{"id":"claude-a","display_name":"A"}],"has_more":true,"last_id":"claude-a"}"#),
        reply(200, r#"{"data":[{"id":"claude-b"}],"has_more":false}"#),
    ])
    .await;
    let outcome = AnthropicDiscovery
        .discover(&request(&base, "sk-ant", true), CancellationToken::new())
        .await
        .unwrap();
    let ids: Vec<&str> = outcome
        .models
        .iter()
        .map(|model| model.id.as_str())
        .collect();
    assert_eq!(ids, vec!["claude-a", "claude-b"]);
    let captured = handle.await.unwrap();
    assert!(captured[0].contains("/v1/models"), "{}", captured[0]);
    assert!(captured[0].to_lowercase().contains("x-api-key: sk-ant"));
    assert!(captured[0].to_lowercase().contains("anthropic-version:"));
    assert!(
        captured[1].contains("after_id=claude-a"),
        "翻页需要带游标：{}",
        captured[1]
    );
}

#[tokio::test]
async fn anthropic_401_is_auth_failed_not_network() {
    let (base, handle) = mock(vec![reply(401, r#"{"error":"bad key"}"#)]).await;
    let error = AnthropicDiscovery
        .discover(&request(&base, "k", true), CancellationToken::new())
        .await
        .unwrap_err();
    assert_eq!(error.code(), "AUTH_FAILED");
    handle.await.unwrap();
}

#[tokio::test]
async fn ollama_reads_its_own_tags_shape() {
    // Ollama 的响应结构是 { models: [ { name, model, details } ] }，不是 OpenAI 的 data。
    let (base, handle) = mock(vec![reply(
        200,
        r#"{"models":[{"name":"llama3.1:8b","model":"llama3.1:8b","details":{"family":"llama"}},{"name":"qwen2.5:7b","model":"qwen2.5:7b"}]}"#,
    )])
    .await;
    let request = DiscoveryRequest::new(base.replace("/v1", ""), "", true)
        .with_timeout(Duration::from_millis(400));
    let outcome = OllamaDiscovery
        .discover(&request, CancellationToken::new())
        .await
        .unwrap();
    let ids: Vec<&str> = outcome
        .models
        .iter()
        .map(|model| model.id.as_str())
        .collect();
    assert_eq!(ids, vec!["llama3.1:8b", "qwen2.5:7b"]);
    let captured = handle.await.unwrap();
    assert!(
        captured[0].contains("GET /api/tags"),
        "必须打 Ollama 自己的端点：{}",
        captured[0]
    );
}

#[tokio::test]
async fn ollama_404_means_not_supported() {
    let (base, handle) = mock(vec![reply(404, "{}")]).await;
    let request = DiscoveryRequest::new(base.replace("/v1", ""), "", true)
        .with_timeout(Duration::from_millis(400));
    let error = OllamaDiscovery
        .discover(&request, CancellationToken::new())
        .await
        .unwrap_err();
    assert_eq!(error.code(), "NOT_SUPPORTED");
    handle.await.unwrap();
}

#[tokio::test]
async fn ollama_loopback_needs_permission() {
    let (base, handle) = mock(vec![reply(200, r#"{"models":[]}"#)]).await;
    let request = DiscoveryRequest::new(base.replace("/v1", ""), "", false)
        .with_timeout(Duration::from_millis(400));
    let error = OllamaDiscovery
        .discover(&request, CancellationToken::new())
        .await
        .unwrap_err();
    assert_eq!(
        error.code(),
        "INVALID_RESPONSE",
        "本地 Ollama 也必须显式允许"
    );
    handle.abort();
}

#[test]
fn discovery_error_codes_are_stable() {
    assert_eq!(DiscoveryError::Cancelled.code(), "CANCELLED");
    assert_eq!(DiscoveryError::Timeout.code(), "TIMEOUT");
}
