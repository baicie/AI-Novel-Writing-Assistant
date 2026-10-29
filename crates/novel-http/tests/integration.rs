//! 本地创作内核 HTTP 入口的集成测试。
//!
//! 这里直接起一个 axum server 在随机 loopback 端口，再用 reqwest 走 HTTP 调用。
//! 这些测试不依赖 Tauri 或 Node 端，可以在 `cargo test -p novel-http` 时独立跑通。

use std::sync::Arc;

use async_trait::async_trait;
use novel_application::{
    ModelSecrets, NoFault, PortError, Runtime, RuntimeDeps, SystemClock, UuidIds,
};
use novel_http::{KernelState, router};
use novel_llm::UnconfiguredProvider;
use novel_platform::{MemorySecretStore, SecretStore};
use novel_storage_sqlite::SqliteKernel;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

/// 把 `SecretStore` 适配成 `ModelSecrets` 端口，给测试用。
struct SecretStoreAdapter {
    inner: Arc<dyn SecretStore>,
}

#[async_trait]
impl ModelSecrets for SecretStoreAdapter {
    async fn api_key(&self, provider: &str) -> Result<Option<String>, PortError> {
        self.inner
            .get(provider)
            .await
            .map_err(|_| PortError::Storage("secret store unavailable".into()))
    }
}

struct TestServer {
    base_url: String,
    shutdown: Option<oneshot::Sender<()>>,
    _temp_dir: TempDir,
}

impl TestServer {
    async fn start() -> Self {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let db_path = temp_dir.path().join("kernel.sqlite");
        let kernel = Arc::new(SqliteKernel::open(&db_path).await.expect("open kernel"));
        let secrets: Arc<dyn novel_platform::SecretStore> = Arc::new(MemorySecretStore::default());
        let model_secrets: Arc<dyn novel_application::ModelSecrets> = Arc::new(SecretStoreAdapter {
            inner: secrets.clone(),
        });
        let runtime = Arc::new(Runtime::new(RuntimeDeps {
            novels: kernel.clone(),
            runs: kernel.clone(),
            worlds: kernel.clone(),
            lookups: kernel.clone(),
            task_center: kernel.clone(),
            llm: Arc::new(UnconfiguredProvider),
            clock: Arc::new(SystemClock),
            ids: Arc::new(UuidIds),
            faults: Arc::new(NoFault),
            secrets: model_secrets,
            owner: "test".into(),
            lease_ms: 30_000,
            concurrency: 1,
        }));
        let state = KernelState::new(runtime, secrets, kernel);

        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let address = listener.local_addr().expect("local addr");
        let app = router(state);
        let (tx, rx) = oneshot::channel::<()>();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    let _ = rx.await;
                })
                .await;
        });
        Self {
            base_url: format!("http://{}", address),
            shutdown: Some(tx),
            _temp_dir: temp_dir,
        }
    }

    async fn stop(mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
    }
}

#[tokio::test]
async fn health_returns_ok_and_supported_paths() {
    let server = TestServer::start().await;
    let response = reqwest::get(format!("{}/api/health", server.base_url))
        .await
        .expect("request");
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("json");
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["status"], "ok");
    let supported = body["data"]["supportedPaths"]
        .as_array()
        .expect("array");
    assert!(!supported.is_empty(), "supported paths should not be empty");
    server.stop().await;
}

#[tokio::test]
async fn novel_crud_round_trip() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();

    // 空库：先列一下
    let response = client
        .get(format!("{}/api/novels", server.base_url))
        .send()
        .await
        .expect("list");
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("json");
    assert_eq!(body["success"], true);
    assert_eq!(
        body["data"]["items"].as_array().map(|a| a.len()),
        Some(0),
        "empty library should list 0 items"
    );
    assert_eq!(body["data"]["total"], 0);

    // 创建一本新书
    let response = client
        .post(format!("{}/api/novels", server.base_url))
        .json(&serde_json::json!({
            "title": "Rust 化的第一本书",
            "description": "从 Node 服务切到 Rust 内核的冒烟测试"
        }))
        .send()
        .await
        .expect("create");
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("json");
    let novel_id = body["data"]["id"]
        .as_str()
        .expect("id")
        .to_string();
    assert_eq!(body["data"]["title"], "Rust 化的第一本书");

    // 创建一章
    let response = client
        .post(format!(
            "{}/api/novels/{}/chapters",
            server.base_url, novel_id
        ))
        .json(&serde_json::json!({
            "title": "第一章 测试",
            "sortOrder": 1
        }))
        .send()
        .await
        .expect("create chapter");
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("json");
    let chapter_id = body["data"][0]["id"].as_str().expect("cid").to_string();

    // 列出章节
    let response = client
        .get(format!(
            "{}/api/novels/{}/chapters",
            server.base_url, novel_id
        ))
        .send()
        .await
        .expect("list chapters");
    let body: serde_json::Value = response.json().await.expect("json");
    assert_eq!(body["data"].as_array().map(|a| a.len()), Some(1));

    // 拿到章节
    let response = client
        .get(format!(
            "{}/api/novels/{}/chapters/{}",
            server.base_url, novel_id, chapter_id
        ))
        .send()
        .await
        .expect("detail");
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("json");
    assert_eq!(body["data"][0]["id"], chapter_id);

    // 保存章节正文
    let response = client
        .put(format!(
            "{}/api/novels/{}/chapters/{}",
            server.base_url, novel_id, chapter_id
        ))
        .json(&serde_json::json!({
            "expectedVersion": 0,
            "title": "第一章 测试",
            "content": "正文内容：本地创作内核的第一个测试章节。"
        }))
        .send()
        .await
        .expect("save");
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("json");
    assert_eq!(body["data"][0]["title"], "第一章 测试");

    server.stop().await;
}

#[tokio::test]
async fn unimplemented_route_returns_501() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/api/knowledge", server.base_url))
        .send()
        .await
        .expect("get");
    assert_eq!(response.status(), 501);
    let body: serde_json::Value = response.json().await.expect("json");
    assert_eq!(body["success"], false);
    assert_eq!(body["migration"], "pending");
    let error = body["error"].as_str().expect("error string");
    assert!(error.contains("knowledge"), "error should mention path: {error}");
    server.stop().await;
}

/// 3.0a 任务中心：空库时 overview 全是 0；recovery-candidates 为空。
#[tokio::test]
async fn tasks_overview_and_recovery_empty_when_no_runs() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();

    // overview
    let response = client
        .get(format!("{}/api/tasks/overview", server.base_url))
        .send()
        .await
        .expect("overview");
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("json");
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["queuedCount"], 0);
    assert_eq!(body["data"]["runningCount"], 0);
    assert_eq!(body["data"]["failedCount"], 0);
    assert_eq!(body["data"]["cancelledCount"], 0);
    assert_eq!(body["data"]["waitingApprovalCount"], 0);
    assert_eq!(body["data"]["recoveryCandidateCount"], 0);

    // recovery-candidates
    let response = client
        .get(format!(
            "{}/api/tasks/recovery-candidates",
            server.base_url
        ))
        .send()
        .await
        .expect("recovery");
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("json");
    assert_eq!(body["success"], true);
    assert_eq!(
        body["data"]["items"].as_array().map(|a| a.len()),
        Some(0),
        "空库下没有 paused run"
    );

    server.stop().await;
}

/// 3.0a 任务中心：3.0b~f 还没完成的写操作必须返回 501 + migration pending。
#[tokio::test]
async fn tasks_write_routes_still_return_501() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();

    // POST /api/tasks/:kind/:id/retry 还没实现
    let response = client
        .post(format!(
            "{}/api/tasks/novel_workflow/abc/retry",
            server.base_url
        ))
        .json(&serde_json::json!({}))
        .send()
        .await
        .expect("retry");
    assert_eq!(response.status(), 501);
    let body: serde_json::Value = response.json().await.expect("json");
    assert_eq!(body["migration"], "pending");

    // POST /api/tasks/:kind/:id/cancel 还没实现
    let response = client
        .post(format!(
            "{}/api/tasks/novel_workflow/abc/cancel",
            server.base_url
        ))
        .send()
        .await
        .expect("cancel");
    assert_eq!(response.status(), 501);

    // GET /api/tasks/:kind/:id 还没实现
    let response = client
        .get(format!(
            "{}/api/tasks/novel_workflow/abc",
            server.base_url
        ))
        .send()
        .await
        .expect("detail");
    assert_eq!(response.status(), 501);

    // GET /api/tasks/ 列表还没实现
    let response = client
        .get(format!("{}/api/tasks", server.base_url))
        .send()
        .await
        .expect("list");
    assert_eq!(response.status(), 501);

    server.stop().await;
}

#[tokio::test]
async fn list_runs_returns_empty_for_novel_without_runs() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();

    // 先创建小说
    let novel = serde_json::json!({
        "title": "测试小说",
        "description": "测试",
        "narrativeForm": "third_person",
        "writingMode": "ai_generated",
        "projectMode": "structured",
        "creationExperience": "complete_beginner",
    });
    let response = client
        .post(format!("{}/api/novels", server.base_url))
        .json(&novel)
        .send()
        .await
        .expect("create");
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("json");
    let novel_id = body["data"]["id"].as_str().expect("novel id");

    // 获取运行列表，应该为空
    let response = client
        .get(format!("{}/api/novels/{}/runs", server.base_url, novel_id))
        .send()
        .await
        .expect("list runs");
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("json");
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["total"], 0);
    assert_eq!(body["data"]["items"].as_array().unwrap().len(), 0);

    server.stop().await;
}

#[tokio::test]
async fn get_chapter_full_returns_content() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();

    // 创建小说
    let novel = serde_json::json!({
        "title": "测试小说",
        "description": "测试",
        "narrativeForm": "third_person",
        "writingMode": "ai_generated",
        "projectMode": "structured",
        "creationExperience": "complete_beginner",
    });
    let response = client
        .post(format!("{}/api/novels", server.base_url))
        .json(&novel)
        .send()
        .await
        .expect("create");
    let body: serde_json::Value = response.json().await.expect("json");
    let novel_id = body["data"]["id"].as_str().expect("novel id");

    // 创建章节
    let chapter = serde_json::json!({
        "title": "第一章",
        "sortOrder": 1,
    });
    let response = client
        .post(format!(
            "{}/api/novels/{}/chapters",
            server.base_url, novel_id
        ))
        .json(&chapter)
        .send()
        .await
        .expect("create chapter");
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("json");
    let chapter_id = body["data"][0]["id"].as_str().expect("chapter id");

    // 获取完整详情
    let response = client
        .get(format!(
            "{}/api/novels/{}/chapters/{}/full",
            server.base_url, novel_id, chapter_id
        ))
        .send()
        .await
        .expect("get full");
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("json");
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["title"], "第一章");
    assert_eq!(body["data"]["sortOrder"], 1);
    assert!(body["data"]["content"].is_string());

    server.stop().await;
}

/// 测试：JSON 轮询模式仍然正常工作（向后兼容）
#[tokio::test]
async fn json_replay_mode_works() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();

    // 创建小说和章节
    let novel = serde_json::json!({
        "title": "SSE 测试小说",
        "description": "测试",
        "narrativeForm": "third_person",
        "writingMode": "ai_generated",
        "projectMode": "structured",
        "creationExperience": "complete_beginner",
    });
    let response = client
        .post(format!("{}/api/novels", server.base_url))
        .json(&novel)
        .send()
        .await
        .expect("create novel");
    let body: serde_json::Value = response.json().await.expect("json");
    let novel_id = body["data"]["id"].as_str().expect("novel id");

    let chapter = serde_json::json!({
        "title": "第一章",
        "sortOrder": 1,
    });
    let response = client
        .post(format!(
            "{}/api/novels/{}/chapters",
            server.base_url, novel_id
        ))
        .json(&chapter)
        .send()
        .await
        .expect("create chapter");
    let body: serde_json::Value = response.json().await.expect("json");
    let chapter_id = body["data"][0]["id"].as_str().expect("chapter id");

    // 打开运行
    let run_input = serde_json::json!({
        "commandId": "cmd-test-123",
        "targetChapterId": chapter_id,
        "provider": "openai",
        "model": "gpt-4",
        "promptVersions": { "chapter_draft": "chapter-draft-v1" },
        "tokenBudget": 8000,
        "maxAutomaticRetries": 0,
        "endpoint": "https://api.openai.com",
        "allowLocalhostHttp": false,
    });
    let response = client
        .post(format!("{}/api/novels/{}/runs", server.base_url, novel_id))
        .json(&run_input)
        .send()
        .await
        .expect("open run");
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("json");
    let run_id = body["data"]["runId"].as_str().expect("run id");

    // 查询事件（JSON 模式，不指定 stream 参数或 stream=false）
    let response = client
        .get(format!("{}/api/runs/{}/events?afterSeq=0", server.base_url, run_id))
        .send()
        .await
        .expect("replay");
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("json");
    assert_eq!(body["success"], true);
    assert!(
        body["data"].is_array(),
        "replay 应该返回数组，当前得到 {:?}",
        body["data"]
    );

    // 查询时指定 stream=false，应该仍返回 JSON
    let response = client
        .get(format!(
            "{}/api/runs/{}/events?afterSeq=0&stream=false",
            server.base_url, run_id
        ))
        .send()
        .await
        .expect("replay json");
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("json");
    assert_eq!(body["success"], true);
    assert!(body["data"].is_array());

    server.stop().await;
}

/// 测试：confirm_event 端点可调用
#[tokio::test]
async fn confirm_event_endpoint_works() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();

    // 创建小说和章节
    let novel = serde_json::json!({
        "title": "确认测试小说",
        "description": "测试",
        "narrativeForm": "third_person",
        "writingMode": "ai_generated",
        "projectMode": "structured",
        "creationExperience": "complete_beginner",
    });
    let response = client
        .post(format!("{}/api/novels", server.base_url))
        .json(&novel)
        .send()
        .await
        .expect("create novel");
    let body: serde_json::Value = response.json().await.expect("json");
    let novel_id = body["data"]["id"].as_str().expect("novel id");

    let chapter = serde_json::json!({
        "title": "第一章",
        "sortOrder": 1,
    });
    let response = client
        .post(format!(
            "{}/api/novels/{}/chapters",
            server.base_url, novel_id
        ))
        .json(&chapter)
        .send()
        .await
        .expect("create chapter");
    let body: serde_json::Value = response.json().await.expect("json");
    let chapter_id = body["data"][0]["id"].as_str().expect("chapter id");

    // 打开运行
    let run_input = serde_json::json!({
        "commandId": "cmd-test-456",
        "targetChapterId": chapter_id,
        "provider": "openai",
        "model": "gpt-4",
        "promptVersions": { "chapter_draft": "chapter-draft-v1" },
        "tokenBudget": 8000,
        "maxAutomaticRetries": 0,
        "endpoint": "https://api.openai.com",
        "allowLocalhostHttp": false,
    });
    let response = client
        .post(format!("{}/api/novels/{}/runs", server.base_url, novel_id))
        .json(&run_input)
        .send()
        .await
        .expect("open run");
    let body: serde_json::Value = response.json().await.expect("json");
    let run_id = body["data"]["runId"].as_str().expect("run id");

    // 调用 confirm_event 端点
    let confirm_body = serde_json::json!({
        "lastConfirmedSeq": 5,
    });
    let response = client
        .post(format!("{}/api/runs/{}/confirm", server.base_url, run_id))
        .json(&confirm_body)
        .send()
        .await
        .expect("confirm");
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("json");
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["confirmed"], true);
    // 返回的是 snake_case 格式 last_confirmed_seq
    assert_eq!(body["data"]["last_confirmed_seq"], 5);

    // 验证无效序号被拒绝
    let confirm_body = serde_json::json!({
        "lastConfirmedSeq": -1,
    });
    let response = client
        .post(format!("{}/api/runs/{}/confirm", server.base_url, run_id))
        .json(&confirm_body)
        .send()
        .await
        .expect("confirm negative");
    assert_eq!(response.status(), 400);

    server.stop().await;
}
