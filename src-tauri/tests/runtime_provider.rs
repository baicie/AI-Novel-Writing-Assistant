use std::sync::Arc;

use novel_llm::{LlmProvider, LlmRequest};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn tauri_composition_provider_uses_real_openai_http_sse() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = [0_u8; 32 * 1024];
        let _ = socket.read(&mut buf).await;
        let body = "data: {\"choices\":[{\"delta\":{\"content\":\"Tauri provider ok\"}}]}\n\ndata: [DONE]\n\n";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = socket.write_all(response.as_bytes()).await;
    });

    let provider: Arc<dyn LlmProvider> = novel_desktop::llm_provider();
    let (sender, mut receiver) = tokio::sync::mpsc::channel::<String>(64);
    let collect = tokio::spawn(async move {
        let mut text = String::new();
        while let Some(delta) = receiver.recv().await {
            text.push_str(&delta);
        }
        text
    });
    let response = provider
        .generate(
            LlmRequest {
                request_id: "tauri-provider-test".into(),
                model: "mock-model".into(),
                prompt: "test".into(),
                endpoint: format!("http://{address}/v1/chat/completions"),
                api_key: "local-test-key".into(),
                allow_localhost_http: true,
                max_output_tokens: Some(64),
            },
            sender,
            CancellationToken::new(),
        )
        .await
        .expect("Tauri composition provider must complete a real SSE request");
    assert_eq!(response.text, "Tauri provider ok");
    assert_eq!(collect.await.unwrap(), "Tauri provider ok");
}
