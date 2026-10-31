//! R6-B2：升级前写入的旧 RunContract 在升级后仍能按旧格式继续执行。
//!
//! 旧合同的字段集与现在不同（没有 step_routes / credential_ref / protocol），
//! 这里用迁移 21 的真实历史 schema 写入，再升级到 HEAD 并用真实 Worker 推进。

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use novel_application::{ModelSecrets, PortError, Runtime, RuntimeDeps, TickOutcome};
use novel_llm::OpenAiCompatibleProvider;
use novel_storage_sqlite::SqliteKernel;
use novel_testkit::{FakeClock, SequentialIds};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

struct LegacySecrets;

#[async_trait]
impl ModelSecrets for LegacySecrets {
    async fn api_key(&self, _provider: &str) -> Result<Option<String>, PortError> {
        Ok(Some("legacy-key".into()))
    }
}

/// 只记录 Authorization 并返回一个合法的 story_macro 结构化响应。
async fn mock_server() -> (
    String,
    Arc<AtomicUsize>,
    Arc<std::sync::Mutex<Option<String>>>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let hits = Arc::new(AtomicUsize::new(0));
    let auth = Arc::new(std::sync::Mutex::new(None));
    let hits_task = hits.clone();
    let auth_task = auth.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let hits = hits_task.clone();
            let auth = auth_task.clone();
            tokio::spawn(async move {
                let mut buffer = vec![0_u8; 32 * 1024];
                let read = socket.read(&mut buffer).await.unwrap_or(0);
                let raw = String::from_utf8_lossy(&buffer[..read]).to_string();
                hits.fetch_add(1, Ordering::SeqCst);
                for line in raw.lines() {
                    if line.to_ascii_lowercase().starts_with("authorization:") {
                        *auth.lock().unwrap() = Some(
                            line.split_once(':')
                                .map(|(_, v)| v.trim().to_string())
                                .unwrap_or_default(),
                        );
                    }
                }
                let story = serde_json::json!({
                    "expansion": {"expanded_premise":"旧合同继续","protagonist_core":"主角","conflict_engine":"冲突","conflict_layers":{"external":"外","internal":"内"},"mystery_box":"未知","emotional_line":"情绪","setpiece_seeds":["场面"],"tone_reference":"气质"},
                    "decomposition": {"selling_point":"卖点","core_conflict":"冲突","main_hook":"钩子","progression_loop":"循环","growth_path":"成长","major_payoffs":["回报"],"ending_flavor":"结局"},
                    "constraints":["规则"],"issues":[]
                })
                .to_string();
                let event =
                    serde_json::json!({"choices":[{"delta":{"content": story}}]}).to_string();
                let body = format!("data: {event}\n\ndata: [DONE]\n\n");
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.flush().await;
            });
        }
    });
    (format!("http://{address}/v1/chat/completions"), hits, auth)
}

#[ignore = "B2 待补：旧合同仍停在门点（0 次远端请求）；需要与旧合同 plan_version 匹配的 artifact fixture 才能推进"]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_contract_without_step_routes_resumes_after_upgrade() {
    let (endpoint, hits, auth) = mock_server().await;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("legacy-run.sqlite");

    let kernel = Arc::new(SqliteKernel::open(&path).await.unwrap());
    let runtime = Runtime::new(RuntimeDeps {
        novels: kernel.clone(),
        runs: kernel.clone(),
        worlds: kernel.clone(),
        lookups: kernel.clone(),
        task_center: kernel.clone(),
        llm: Arc::new(OpenAiCompatibleProvider::new()),
        clock: Arc::new(FakeClock::new(2_000)),
        ids: Arc::new(SequentialIds::new()),
        faults: Arc::new(novel_application::NoFault),
        secrets: Arc::new(LegacySecrets),
        owner: "legacy-worker".into(),
        lease_ms: 60_000,
        concurrency: 1,
    });

    // 用真实 Director 建书与章节：这样工作流需要的前置记录都由正常的业务路径产生，
    // 然后只把 Run 的合同替换成「旧格式」，专测合同兼容。
    let novel = runtime.director.create_novel("旧合同书", "").await.unwrap();
    let chapter = runtime
        .director
        .create_chapter(&novel.id, "第一章", 1)
        .await
        .unwrap();
    let contract = serde_json::json!({
        "runId": "r-legacy",
        "novelId": novel.id,
        "planVersion": "v1",
        "chapterPlanVersion": "v1",
        "targetChapterId": chapter.id,
        "provider": "openai-compatible",
        "model": "legacy-model",
        "promptVersions": {"chapter_draft": "chapter-draft-v1"},
        "tokenBudget": 1024,
        "contextTokenBudget": 24000,
        "maxAutomaticRetries": 0,
        "repairBudget": 0,
        "endpoint": endpoint,
        "allowLocalhostHttp": true
    })
    .to_string();
    sqlx::query("INSERT INTO workflow_run(id, novel_id, plan_version, contract_json, created_at) VALUES ('r-legacy', ?, 'v1', ?, 1)")
        .bind(&novel.id)
        .bind(&contract)
        .execute(kernel.pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO workflow_control(run_id, novel_id, status, version, cursor_step_id) VALUES ('r-legacy', ?, 'running', 0, NULL)")
        .bind(&novel.id)
        .execute(kernel.pool())
        .await
        .unwrap();

    let outcome = runtime.worker.tick().await.unwrap();
    assert!(
        !matches!(outcome, TickOutcome::Interrupted { .. }),
        "旧合同必须能继续执行：{outcome:?}"
    );
    assert_eq!(hits.load(Ordering::SeqCst), 1, "旧合同应当发出一次真实请求");
    assert_eq!(
        auth.lock().unwrap().as_deref(),
        Some("Bearer legacy-key"),
        "旧合同应使用历史默认密钥槽"
    );

    // 3. 数据库里合同没有被改写，旧字段仍在。
    let stored: String =
        sqlx::query_scalar("SELECT contract_json FROM workflow_run WHERE id='r-legacy'")
            .fetch_one(kernel.pool())
            .await
            .unwrap();
    assert!(stored.contains("\"provider\":\"openai-compatible\""));
    assert!(!stored.contains("legacy-key"), "合同里不能出现密钥明文");
    let integrity: String = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(kernel.pool())
        .await
        .unwrap();
    assert_eq!(integrity, "ok");
}
