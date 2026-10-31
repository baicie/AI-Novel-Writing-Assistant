//! R8.3-I2：两个独立进程竞争接管同一个 Run（Lease Fencing + 不重复计费）。
//!
//! 进程 A：拿到租约后发出一次生成请求并卡在等待响应上，被外部 SIGKILL。
//! 进程 B（本测试进程）：租约过期后接管，必须能继续，但不得重发计费未知的请求。

use std::collections::BTreeMap;
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

#[derive(Default)]
struct TestVault;

#[async_trait]
impl CredentialVault for TestVault {
    async fn write(&self, _reference: &str, _secret: &str) -> Result<(), PortError> {
        Ok(())
    }
    async fn delete(&self, _reference: &str) -> Result<(), PortError> {
        Ok(())
    }
    async fn present(&self, _reference: &str) -> Result<bool, PortError> {
        Ok(true)
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

struct TestSecrets;

#[async_trait]
impl ModelSecrets for TestSecrets {
    async fn api_key(&self, _provider: &str) -> Result<Option<String>, PortError> {
        Ok(Some("key-T".into()))
    }
}

/// 收到请求但永不响应：计费状态未知。
async fn silent_server() -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let hits = Arc::new(AtomicUsize::new(0));
    let hits_task = hits.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let hits = hits_task.clone();
            tokio::spawn(async move {
                let mut buffer = vec![0_u8; 16 * 1024];
                let _ = socket.read(&mut buffer).await;
                hits.fetch_add(1, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_secs(600)).await;
                drop(socket);
            });
        }
    });
    (format!("http://{address}/v1"), hits)
}

async fn seed(kernel: Arc<SqliteKernel>, base_url: &str) -> (ProviderService, Runtime) {
    let vault = Arc::new(TestVault);
    let service = ProviderService::new(
        kernel.clone() as Arc<dyn ProviderRegistry>,
        vault as Arc<dyn CredentialVault>,
        Arc::new(TestIntents),
    );
    service
        .upsert_provider(
            ProviderDraft {
                id: "gw".into(),
                display_name: Some("T 网关".into()),
                protocol: ProviderProtocol::OpenAiCompatible,
                base_url: base_url.to_string(),
                allow_localhost_http: true,
                is_builtin: false,
                sort_order: 0,
            },
            CredentialAction::Replace("key-T".into()),
            None,
            1,
        )
        .await
        .unwrap();
    service
        .upsert_model(
            novel_domain::ModelProfile {
                provider_id: "gw".into(),
                model_id: "model".into(),
                display_name: Some("model".into()),
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
                provider_id: "gw".into(),
                model_id: "model".into(),
                reasoning_effort: None,
            },
            None,
            1,
        )
        .await
        .unwrap();
    let runtime = Runtime::new(RuntimeDeps {
        novels: kernel.clone(),
        runs: kernel.clone(),
        worlds: kernel.clone(),
        lookups: kernel.clone(),
        task_center: kernel.clone(),
        llm: Arc::new(OpenAiCompatibleProvider::with_policy(ProviderPolicy {
            timeout: Duration::from_millis(700),
            max_attempts: 3,
            ..ProviderPolicy::default()
        })),
        clock: Arc::new(FakeClock::new(1_000)),
        ids: Arc::new(SequentialIds::new()),
        faults: Arc::new(novel_application::NoFault),
        secrets: Arc::new(TestSecrets),
        owner: "worker".into(),
        lease_ms: 400,
        concurrency: 1,
    });
    (service, runtime)
}

async fn open(
    kernel: Arc<SqliteKernel>,
    service: &ProviderService,
    runtime: &Runtime,
    base_url: &str,
) -> (String, String, String) {
    let routes = service.frozen_step_routes().await.unwrap();
    let novel = runtime.director.create_novel("接管书", "").await.unwrap();
    let chapter = runtime
        .director
        .create_chapter(&novel.id, "第一章", 1)
        .await
        .unwrap();
    let mut prompts = BTreeMap::new();
    prompts.insert("chapter_draft".to_string(), "chapter-draft-v1".to_string());
    let opened = runtime
        .director
        .open_run(OpenRunInput {
            command_id: "t-open".into(),
            novel_id: novel.id.clone(),
            target_chapter_id: chapter.id.clone(),
            provider: "gw".into(),
            model: "model".into(),
            prompt_versions: prompts,
            token_budget: Some(1024),
            context_token_budget: None,
            max_automatic_retries: 0,
            repair_budget: None,
            endpoint: format!("{base_url}/chat/completions"),
            allow_localhost_http: true,
            step_routes: routes,
            credential_ref: Some("gw#1".into()),
        })
        .await
        .unwrap();
    let _ = kernel;
    (opened.run_id, novel.id, chapter.id)
}

fn env(key: &str) -> Option<String> {
    std::env::var(key).ok()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn takeover_child_process() {
    let (Some(db), Some(endpoint), Some(marker)) = (
        env("P166_T_DB"),
        env("P166_T_ENDPOINT"),
        env("P166_T_MARKER"),
    ) else {
        return;
    };
    let kernel = Arc::new(SqliteKernel::open(std::path::Path::new(&db)).await.unwrap());
    let (service, runtime) = seed(
        kernel.clone(),
        endpoint.trim_end_matches("/chat/completions"),
    )
    .await;
    let (run_id, novel_id, chapter_id) = open(
        kernel.clone(),
        &service,
        &runtime,
        endpoint.trim_end_matches("/chat/completions"),
    )
    .await;
    std::fs::write(
        &marker,
        serde_json::json!({"runId": run_id, "novelId": novel_id, "chapterId": chapter_id})
            .to_string(),
    )
    .unwrap();
    // 发出生成请求并卡住：请求已送达，响应未知。
    let _ = runtime.worker.tick().await;
    tokio::time::sleep(Duration::from_secs(600)).await;
}

fn read_marker(path: &std::path::Path) -> Option<serde_json::Value> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn second_worker_takes_over_without_resending_unknown_request() {
    let (base_url, hits) = silent_server().await;
    let directory = tempfile::tempdir().unwrap();
    let db_path = directory.path().join("takeover.sqlite");
    let db = db_path.to_string_lossy().to_string();
    let marker = directory.path().join("marker.json");

    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "takeover_child_process", "--nocapture"])
        .env("P166_T_DB", &db)
        .env("P166_T_ENDPOINT", format!("{base_url}/chat/completions"))
        .env("P166_T_MARKER", marker.to_string_lossy().to_string())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();

    let mut waited = 0;
    while read_marker(&marker).is_none() || hits.load(Ordering::SeqCst) == 0 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        waited += 1;
        assert!(waited < 600, "子进程没有发出请求");
    }
    let info = read_marker(&marker).unwrap();
    let run_id = info["runId"].as_str().unwrap().to_string();
    let novel_id = info["novelId"].as_str().unwrap().to_string();
    let chapter_id = info["chapterId"].as_str().unwrap().to_string();
    assert_eq!(hits.load(Ordering::SeqCst), 1, "子进程应只发出一次请求");

    // 外部 SIGKILL：进程 A 消失，但它的租约仍在有效期内。
    let _ = child.kill();
    let _ = child.wait();

    let kernel = Arc::new(SqliteKernel::open(&db_path).await.unwrap());
    let vault = Arc::new(TestVault);
    let service = ProviderService::new(
        kernel.clone() as Arc<dyn ProviderRegistry>,
        vault as Arc<dyn CredentialVault>,
        Arc::new(TestIntents),
    );
    let make_runtime = |now_ms: i64, owner: &str| {
        Runtime::new(RuntimeDeps {
            novels: kernel.clone(),
            runs: kernel.clone(),
            worlds: kernel.clone(),
            lookups: kernel.clone(),
            task_center: kernel.clone(),
            llm: Arc::new(OpenAiCompatibleProvider::with_policy(ProviderPolicy {
                timeout: Duration::from_millis(700),
                max_attempts: 3,
                ..ProviderPolicy::default()
            })),
            clock: Arc::new(FakeClock::new(now_ms)),
            ids: Arc::new(SequentialIds::new()),
            faults: Arc::new(novel_application::NoFault),
            secrets: Arc::new(TestSecrets),
            owner: owner.into(),
            lease_ms: 400,
            concurrency: 1,
        })
    };

    // 租约还没过期时（A 的时钟在 1_000ms 附近），B 不能抢到同一个 Run。
    let runtime = make_runtime(1_100, "worker-b");
    let early = runtime.worker.tick().await.unwrap();
    assert!(
        !matches!(early, TickOutcome::Advanced { .. }),
        "租约有效期内不应被第二个进程接管：{early:?}"
    );
    assert_eq!(hits.load(Ordering::SeqCst), 1, "抢锁失败不得触发新请求");

    // 时间推进到租约过期之后，另一个进程接管同一个 Run。
    tokio::time::sleep(Duration::from_millis(200)).await;
    let runtime = make_runtime(5_000, "worker-b2");
    for _ in 0..5 {
        let _ = runtime.worker.tick().await.unwrap();
    }
    assert_eq!(
        hits.load(Ordering::SeqCst),
        1,
        "接管后不得重发计费未知的请求（实际 {} 次）",
        hits.load(Ordering::SeqCst)
    );
    let _ = service;

    let detail = runtime
        .director
        .get_run_detail(&novel_id, &run_id)
        .await
        .unwrap();
    assert!(
        detail.status == "paused" || detail.status == "failed",
        "接管后应停在安全的可恢复状态：{}",
        detail.status
    );
    let chapter = runtime
        .director
        .get_chapter(&novel_id, &chapter_id)
        .await
        .unwrap();
    assert_eq!(chapter.content, "", "计费未知时不得写入正式正文");
    assert_eq!(chapter.official_version, 0);

    let integrity: String = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(kernel.pool())
        .await
        .unwrap();
    assert_eq!(integrity, "ok");
}
