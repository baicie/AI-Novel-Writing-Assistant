use std::sync::Arc;

use async_trait::async_trait;
use novel_application::{ModelSecrets, PortError};
use novel_application::{OpenRunInput, Runtime, RuntimeDeps, TickOutcome, WorkflowRepository};
use novel_llm::OpenAiCompatibleProvider;
use novel_storage_sqlite::SqliteKernel;
use novel_testkit::{FakeClock, SequentialIds};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

struct TestSecrets;

#[async_trait]
impl ModelSecrets for TestSecrets {
    async fn api_key(&self, _provider: &str) -> Result<Option<String>, PortError> {
        Ok(Some("local-test-key".into()))
    }
}

async fn mock_sse_server(story_json: String) -> std::net::SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let story_json = story_json.clone();
            tokio::spawn(async move {
                let mut buf = [0_u8; 64 * 1024];
                let _ = socket.read(&mut buf).await;
                let event = serde_json::json!({
                    "choices": [{"delta": {"content": story_json}}]
                })
                .to_string();
                let body = format!("data: {event}\n\ndata: [DONE]\n\n");
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = socket.write_all(response.as_bytes()).await;
            });
        }
    });
    address
}

#[tokio::test]
async fn tauri_runtime_uses_real_openai_compatible_http_provider_for_story_macro() {
    let story = serde_json::json!({
        "expansion": {
            "expanded_premise": "真实 HTTP 路径",
            "protagonist_core": "主角",
            "conflict_engine": "冲突引擎",
            "conflict_layers": {"external": "外", "internal": "内", "relational": "关系"},
            "mystery_box": "未知",
            "emotional_line": "情绪",
            "setpiece_seeds": ["场面"],
            "tone_reference": "气质"
        },
        "decomposition": {
            "selling_point": "卖点",
            "core_conflict": "冲突",
            "main_hook": "钩子",
            "progression_loop": "循环",
            "growth_path": "成长",
            "major_payoffs": ["回报"],
            "ending_flavor": "结局"
        },
        "constraints": ["规则"],
        "issues": []
    })
    .to_string();
    let address = mock_sse_server(story).await;
    let endpoint = format!("http://{address}/v1/chat/completions");

    let directory = tempfile::tempdir().unwrap();
    let kernel = Arc::new(
        SqliteKernel::open(&directory.path().join("http.sqlite"))
            .await
            .unwrap(),
    );
    let llm: Arc<dyn novel_llm::LlmProvider> = Arc::new(OpenAiCompatibleProvider::new());
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
        secrets: Arc::new(TestSecrets),
        owner: "http-worker".into(),
        lease_ms: 60_000,
        concurrency: 1,
    });
    let novel = runtime
        .director
        .create_novel("HTTP 装配", "")
        .await
        .unwrap();
    let chapter = runtime
        .director
        .create_chapter(&novel.id, "第一章", 1)
        .await
        .unwrap();
    let opened = runtime
        .director
        .open_run(OpenRunInput {
            command_id: "http-open".into(),
            novel_id: novel.id.clone(),
            target_chapter_id: chapter.id,
            provider: "mock".into(),
            model: "mock-model".into(),
            prompt_versions: {
                let mut prompts = std::collections::BTreeMap::new();
                prompts.insert("chapter_draft".into(), "chapter-draft-v1".into());
                prompts
            },
            token_budget: Some(1024),
            context_token_budget: None,
            max_automatic_retries: 0,
            endpoint,
            allow_localhost_http: true,
        })
        .await
        .unwrap();
    let outcome = runtime.worker.tick().await.unwrap();
    assert!(
        !matches!(outcome, TickOutcome::Interrupted { .. }),
        "HTTP provider must not be interrupted: {outcome:?}"
    );
    let artifacts = WorkflowRepository::list_artifacts(kernel.as_ref(), &novel.id)
        .await
        .unwrap();
    let control = WorkflowRepository::run_control(kernel.as_ref(), &opened.run_id)
        .await
        .unwrap();
    println!("HTTP test pause: {:?}", control.pause);
    let story_artifact = artifacts
        .iter()
        .find(|a| a.artifact_type == "story_macro")
        .expect("story macro artifact from HTTP");
    let payload: serde_json::Value = serde_json::from_str(&story_artifact.payload_json).unwrap();
    assert_eq!(payload["expansion"]["expanded_premise"], "真实 HTTP 路径");
    assert_eq!(payload["issues"].as_array().unwrap().len(), 0);
    assert_eq!(
        story_artifact.status,
        novel_domain::ArtifactStatus::Confirmed
    );
    assert!(opened.run_id.starts_with("run-") || !opened.run_id.is_empty());
}
