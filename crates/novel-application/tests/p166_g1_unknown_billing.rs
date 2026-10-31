//! R5.1-G1：远端已收到生成请求但没有响应 → 计费状态未知，不得自动重发。
//!
//! 判定依据是 Mock Server 实际收到的请求次数，以及 Run 的持久化状态。

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use novel_application::{
    CredentialAction, CredentialIntents, CredentialVault, ModelSecrets, OpenRunInput,
    PendingCredentialIntent, PortError, ProviderDraft, ProviderRegistry, ProviderService, Runtime,
    RuntimeDeps, TickOutcome,
};
use novel_domain::ProviderProtocol;
use novel_llm::{OpenAiCompatibleProvider, ProviderPolicy};
use novel_storage_sqlite::SqliteKernel;
use novel_testkit::{FakeClock, SequentialIds};
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;

/// 收到请求后永不响应：连接保持打开，客户端只能超时。
async fn silent_server() -> (String, std::net::SocketAddr, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let hits = Arc::new(AtomicUsize::new(0));
    let hits_for_task = hits.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let hits = hits_for_task.clone();
            tokio::spawn(async move {
                let mut buffer = vec![0_u8; 32 * 1024];
                let _ = socket.read(&mut buffer).await;
                hits.fetch_add(1, Ordering::SeqCst);
                // 故意不写任何响应字节：连接保持到测试结束。
                tokio::time::sleep(Duration::from_secs(3600)).await;
                drop(socket);
            });
        }
    });
    (format!("http://{address}/v1"), address, hits)
}

#[derive(Default)]
struct TestVault {
    values: std::sync::Mutex<std::collections::HashMap<String, String>>,
}

#[async_trait]
impl CredentialVault for TestVault {
    async fn write(&self, reference: &str, secret: &str) -> Result<(), PortError> {
        self.values
            .lock()
            .unwrap()
            .insert(reference.to_string(), secret.to_string());
        Ok(())
    }
    async fn delete(&self, reference: &str) -> Result<(), PortError> {
        self.values.lock().unwrap().remove(reference);
        Ok(())
    }
    async fn present(&self, reference: &str) -> Result<bool, PortError> {
        Ok(self.values.lock().unwrap().contains_key(reference))
    }
}

#[derive(Default)]
struct TestIntents;

#[async_trait]
impl CredentialIntents for TestIntents {
    async fn begin(
        &self,
        _id: &str,
        _provider_id: &str,
        _secret_ref: &str,
        _action: &str,
        _now_ms: i64,
    ) -> Result<(), PortError> {
        Ok(())
    }
    async fn finish(
        &self,
        _id: &str,
        _state: &str,
        _last_error: Option<&str>,
        _now_ms: i64,
    ) -> Result<(), PortError> {
        Ok(())
    }
    async fn pending(&self) -> Result<Vec<PendingCredentialIntent>, PortError> {
        Ok(Vec::new())
    }
}

struct TestSecrets(Arc<TestVault>);

#[async_trait]
impl ModelSecrets for TestSecrets {
    async fn api_key(&self, provider: &str) -> Result<Option<String>, PortError> {
        Ok(self.0.values.lock().unwrap().get(provider).cloned())
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn delivered_request_without_response_is_not_resent() {
    let (base_url, _address, hits) = silent_server().await;

    let directory = tempfile::tempdir().unwrap();
    let kernel = Arc::new(
        SqliteKernel::open(&directory.path().join("g1.sqlite"))
            .await
            .unwrap(),
    );
    let vault = Arc::new(TestVault::default());
    let service = ProviderService::new(
        kernel.clone() as Arc<dyn ProviderRegistry>,
        vault.clone() as Arc<dyn CredentialVault>,
        Arc::new(TestIntents),
    );
    service
        .upsert_provider(
            ProviderDraft {
                id: "gw-silent".into(),
                display_name: Some("静默网关".into()),
                protocol: ProviderProtocol::OpenAiCompatible,
                base_url: base_url.clone(),
                allow_localhost_http: true,
                is_builtin: false,
                sort_order: 0,
            },
            CredentialAction::Replace("key-S".into()),
            None,
            1,
        )
        .await
        .unwrap();
    service
        .upsert_model(
            novel_domain::ModelProfile {
                provider_id: "gw-silent".into(),
                model_id: "model-silent".into(),
                display_name: Some("model-silent".into()),
                capabilities: Default::default(),
                source: novel_domain::ModelSource::User,
                overridden: Default::default(),
            },
            false,
            None,
            1,
        )
        .await
        .unwrap();
    kernel
        .set_binding_cas(
            &novel_domain::ModelBinding {
                task_type: "planner".into(),
                provider_id: "gw-silent".into(),
                model_id: "model-silent".into(),
                reasoning_effort: None,
            },
            None,
            1,
        )
        .await
        .unwrap();

    let step_routes = service.frozen_step_routes().await.unwrap();

    // 慢策略：800ms 超时 + 允许 3 次尝试，用来证明「超时不会被当成可安全重试」。
    let llm = Arc::new(OpenAiCompatibleProvider::with_policy(ProviderPolicy {
        timeout: Duration::from_millis(800),
        max_attempts: 3,
        ..ProviderPolicy::default()
    }));
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
        secrets: Arc::new(TestSecrets(vault.clone())),
        owner: "g1-worker".into(),
        lease_ms: 60_000,
        concurrency: 1,
    });
    let novel = runtime
        .director
        .create_novel("计费未知书", "")
        .await
        .unwrap();
    let chapter = runtime
        .director
        .create_chapter(&novel.id, "第一章", 1)
        .await
        .unwrap();
    let mut prompts = std::collections::BTreeMap::new();
    prompts.insert("chapter_draft".into(), "chapter-draft-v1".into());
    let opened = runtime
        .director
        .open_run(OpenRunInput {
            command_id: "g1-open".into(),
            novel_id: novel.id.clone(),
            target_chapter_id: chapter.id.clone(),
            provider: "gw-silent".into(),
            model: "model-silent".into(),
            prompt_versions: prompts,
            token_budget: Some(1024),
            context_token_budget: None,
            max_automatic_retries: 1,
            repair_budget: None,
            endpoint: format!("{base_url}/chat/completions"),
            allow_localhost_http: true,
            step_routes,
            credential_ref: Some("gw-silent#1".into()),
        })
        .await
        .unwrap();

    // 第一次推进：请求已送达，客户端超时。
    let first = runtime.worker.tick().await.unwrap();
    assert!(
        !matches!(first, TickOutcome::Idle),
        "第一次推进应当真正发起请求：{first:?}"
    );
    assert_eq!(
        hits.load(Ordering::SeqCst),
        1,
        "远端只应收到一次请求（超时不得自动重发）"
    );

    // 再做多次推进：恢复路径不允许擅自重发计费未知的请求。
    for _ in 0..5 {
        let _ = runtime.worker.tick().await.unwrap();
    }
    assert_eq!(
        hits.load(Ordering::SeqCst),
        1,
        "恢复/重试路径不得自动重发计费未知的请求"
    );

    // Run 必须停在可恢复状态，并把失败原因持久化。
    let detail = runtime
        .director
        .get_run_detail(&novel.id, &opened.run_id)
        .await
        .unwrap();
    assert!(
        detail.status == "paused" || detail.status == "failed",
        "计费未知必须进入可恢复停点，实际 {}",
        detail.status
    );
    assert!(
        !detail.legal_actions.is_empty(),
        "停点必须给出可执行的下一步"
    );

    let contract_json: String =
        sqlx::query_scalar("SELECT contract_json FROM workflow_run WHERE id = ?")
            .bind(&opened.run_id)
            .fetch_one(kernel.pool())
            .await
            .unwrap();
    assert!(!contract_json.contains("key-S"), "合同里不能出现密钥明文");
}

// ============================================================
// R5.2-G3/I：外部 SIGKILL 后重启，不擅自重发已送达的请求。
// 子进程复用本测试二进制（环境变量驱动），避免额外二进制目标。
// ============================================================

fn g3_env() -> Option<(String, String)> {
    let db = std::env::var("P166_G3_DB").ok()?;
    let endpoint = std::env::var("P166_G3_ENDPOINT").ok()?;
    Some((db, endpoint))
}

async fn build_runtime(
    kernel: Arc<SqliteKernel>,
    vault: Arc<TestVault>,
    endpoint: String,
    owner: &str,
    timeout_ms: u64,
) -> Runtime {
    let _ = endpoint;
    Runtime::new(RuntimeDeps {
        novels: kernel.clone(),
        runs: kernel.clone(),
        worlds: kernel.clone(),
        lookups: kernel.clone(),
        task_center: kernel.clone(),
        llm: Arc::new(OpenAiCompatibleProvider::with_policy(ProviderPolicy {
            timeout: Duration::from_millis(timeout_ms),
            max_attempts: 3,
            ..ProviderPolicy::default()
        })),
        clock: Arc::new(FakeClock::new(1_000)),
        ids: Arc::new(SequentialIds::new()),
        faults: Arc::new(novel_application::NoFault),
        secrets: Arc::new(TestSecrets(vault)),
        owner: owner.into(),
        lease_ms: 60_000,
        concurrency: 1,
    })
}

async fn seed_silent_provider(
    kernel: Arc<SqliteKernel>,
    vault: Arc<TestVault>,
    base_url: &str,
) -> novel_application::ProviderService {
    let service = ProviderService::new(
        kernel.clone() as Arc<dyn ProviderRegistry>,
        vault.clone() as Arc<dyn CredentialVault>,
        Arc::new(TestIntents),
    );
    service
        .upsert_provider(
            ProviderDraft {
                id: "gw-silent".into(),
                display_name: Some("静默网关".into()),
                protocol: ProviderProtocol::OpenAiCompatible,
                base_url: base_url.into(),
                allow_localhost_http: true,
                is_builtin: false,
                sort_order: 0,
            },
            CredentialAction::Replace("key-S".into()),
            None,
            1,
        )
        .await
        .unwrap();
    service
        .upsert_model(
            novel_domain::ModelProfile {
                provider_id: "gw-silent".into(),
                model_id: "model-silent".into(),
                display_name: Some("model-silent".into()),
                capabilities: Default::default(),
                source: novel_domain::ModelSource::User,
                overridden: Default::default(),
            },
            false,
            None,
            1,
        )
        .await
        .unwrap();
    kernel
        .set_binding_cas(
            &novel_domain::ModelBinding {
                task_type: "planner".into(),
                provider_id: "gw-silent".into(),
                model_id: "model-silent".into(),
                reasoning_effort: None,
            },
            None,
            1,
        )
        .await
        .unwrap();
    service
}

async fn open_silent_run(
    runtime: &Runtime,
    service: &novel_application::ProviderService,
    base_url: &str,
) -> String {
    let step_routes = service.frozen_step_routes().await.unwrap();
    let novel = runtime
        .director
        .create_novel("SIGKILL 书", "")
        .await
        .unwrap();
    let chapter = runtime
        .director
        .create_chapter(&novel.id, "第一章", 1)
        .await
        .unwrap();
    let mut prompts = std::collections::BTreeMap::new();
    prompts.insert("chapter_draft".into(), "chapter-draft-v1".into());
    let opened = runtime
        .director
        .open_run(OpenRunInput {
            command_id: format!("g3-open-{}", novel.id),
            novel_id: novel.id.clone(),
            target_chapter_id: chapter.id,
            provider: "gw-silent".into(),
            model: "model-silent".into(),
            prompt_versions: prompts,
            token_budget: Some(1024),
            context_token_budget: None,
            max_automatic_retries: 1,
            repair_budget: None,
            endpoint: format!("{base_url}/chat/completions"),
            allow_localhost_http: true,
            step_routes,
            credential_ref: Some("gw-silent#1".into()),
        })
        .await
        .unwrap();
    opened.run_id
}

/// 子进程主体：只有在环境变量存在时才真正执行。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn g3_child_worker() {
    let Some((db, endpoint)) = g3_env() else {
        return;
    };
    let kernel = Arc::new(SqliteKernel::open(std::path::Path::new(&db)).await.unwrap());
    let vault = Arc::new(TestVault::default());
    let service = seed_silent_provider(kernel.clone(), vault.clone(), &endpoint).await;
    let runtime = build_runtime(kernel, vault, endpoint.clone(), "g3-child", 60_000).await;
    let _run_id = open_silent_run(&runtime, &service, &endpoint).await;
    // 这一步会把请求发出去并一直等待响应；父进程在收到请求后 SIGKILL 本进程。
    let _ = runtime.worker.tick().await;
    tokio::time::sleep(Duration::from_secs(3600)).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sigkill_after_delivery_does_not_resend_on_restart() {
    let (base_url, _address, hits) = silent_server().await;
    let directory = tempfile::tempdir().unwrap();
    let db_path = directory.path().join("g3.sqlite");
    let db = db_path.to_string_lossy().to_string();

    // 1. 起子进程：建库、开 Run、发请求后阻塞。
    let exe = std::env::current_exe().unwrap();
    let mut child = std::process::Command::new(exe)
        .args(["--exact", "g3_child_worker", "--nocapture"])
        .env("P166_G3_DB", &db)
        .env("P166_G3_ENDPOINT", &base_url)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();

    // 2. 等远端真正收到请求。
    let mut waited = 0;
    while hits.load(Ordering::SeqCst) == 0 && waited < 300 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        waited += 1;
    }
    assert_eq!(hits.load(Ordering::SeqCst), 1, "子进程应当已发出一次请求");

    // 3. 外部 SIGKILL（真实进程级崩溃）。
    let _ = child.kill();
    let _ = child.wait();
    assert_eq!(hits.load(Ordering::SeqCst), 1, "SIGKILL 本身不会产生新请求");

    // 4. 重启：新的 Runtime 打开同一份数据库，接管同一个 Run。
    let kernel = Arc::new(SqliteKernel::open(&db_path).await.unwrap());
    let vault = Arc::new(TestVault::default());
    // 崩溃进程的密钥在真实环境里来自钥匙库/文件；这里用同一把测试密钥重建。
    vault
        .values
        .lock()
        .unwrap()
        .insert("gw-silent#1".into(), "key-S".into());
    let runtime = build_runtime(kernel.clone(), vault, base_url.clone(), "g3-restart", 800).await;

    for _ in 0..5 {
        let _ = runtime.worker.tick().await.unwrap();
    }
    assert_eq!(
        hits.load(Ordering::SeqCst),
        1,
        "重启后不得自动重发计费未知的请求（实际 {} 次）",
        hits.load(Ordering::SeqCst)
    );

    let runs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workflow_run")
        .fetch_one(kernel.pool())
        .await
        .unwrap();
    assert_eq!(runs, 1, "同一个 Run 不应被复制成多个");
}
