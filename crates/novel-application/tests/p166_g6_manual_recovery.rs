//! R8.4-G6：计费未知时的人工恢复状态机（真实 SQLite + 真实 Worker + 本地 Mock）。

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use novel_application::{
    CredentialAction, CredentialIntents, CredentialVault, ModelSecrets, OpenRunInput,
    PendingCredentialIntent, PortError, ProviderDraft, ProviderRegistry, ProviderService, Runtime,
    RuntimeDeps,
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
        Ok(Some("key-G6".into()))
    }
}

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

struct Harness {
    // 必须持有临时目录：它一旦被删，SQLite 会在下一次操作时打不开文件。
    _dir: tempfile::TempDir,
    #[allow(dead_code)]
    kernel: Arc<SqliteKernel>,
    runtime: Runtime,
    novel_id: String,
    chapter_id: String,
    run_id: String,
}

async fn harness(base_url: &str, now_ms: i64) -> Harness {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("g6.sqlite");
    let kernel = Arc::new(SqliteKernel::open(&path).await.unwrap());
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
                display_name: Some("G6 网关".into()),
                protocol: ProviderProtocol::OpenAiCompatible,
                base_url: base_url.to_string(),
                allow_localhost_http: true,
                is_builtin: false,
                sort_order: 0,
            },
            CredentialAction::Replace("key-G6".into()),
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
    let routes = service.frozen_step_routes().await.unwrap();
    let runtime = Runtime::new(RuntimeDeps {
        novels: kernel.clone(),
        runs: kernel.clone(),
        worlds: kernel.clone(),
        lookups: kernel.clone(),
        task_center: kernel.clone(),
        llm: Arc::new(OpenAiCompatibleProvider::with_policy(ProviderPolicy {
            timeout: Duration::from_millis(600),
            max_attempts: 3,
            ..ProviderPolicy::default()
        })),
        clock: Arc::new(FakeClock::new(now_ms)),
        ids: Arc::new(SequentialIds::new()),
        faults: Arc::new(novel_application::NoFault),
        secrets: Arc::new(TestSecrets),
        owner: "g6-worker".into(),
        lease_ms: 60_000,
        concurrency: 1,
    });
    let novel = runtime.director.create_novel("G6 书", "").await.unwrap();
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
            command_id: "g6-open".into(),
            novel_id: novel.id.clone(),
            target_chapter_id: chapter.id.clone(),
            provider: "gw".into(),
            model: "model".into(),
            prompt_versions: prompts,
            token_budget: Some(1024),
            context_token_budget: None,
            max_automatic_retries: 1,
            repair_budget: None,
            endpoint: format!("{base_url}/chat/completions"),
            allow_localhost_http: true,
            step_routes: routes,
            credential_ref: Some("gw#1".into()),
        })
        .await
        .unwrap();
    Harness {
        _dir: directory,
        kernel,
        runtime,
        novel_id: novel.id,
        chapter_id: chapter.id,
        run_id: opened.run_id,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn unknown_billing_pauses_and_only_explicit_resume_retries() {
    let (base_url, hits) = silent_server().await;
    let h = harness(&base_url, 1_000).await;

    // 1. 请求已送达、响应未知。
    let _ = h.runtime.worker.tick().await.unwrap();
    assert_eq!(hits.load(Ordering::SeqCst), 1, "应当只发出一次请求");

    // 2. 故障后系统必须识别为需要人工决策的停点。
    let detail = h
        .runtime
        .director
        .get_run_detail(&h.novel_id, &h.run_id)
        .await
        .unwrap();
    assert_eq!(detail.status, "paused", "计费未知必须进入停点");
    let reason = detail
        .pause_reason
        .clone()
        .or_else(|| detail.failure_reason.clone())
        .unwrap_or_default();
    assert!(!reason.is_empty(), "必须给出可读原因：{reason}");
    assert!(
        !detail.legal_actions.is_empty(),
        "必须给出用户可执行的动作：{:?}",
        detail.legal_actions
    );

    // 3. 没有任何新的授权前，不得自动重发。
    for _ in 0..4 {
        let _ = h.runtime.worker.tick().await.unwrap();
    }
    assert_eq!(hits.load(Ordering::SeqCst), 1, "不得自动重发计费未知请求");

    // 4. 人工取消：保留记录、不写正文。
    h.runtime
        .director
        .cancel("g6-cancel-1", &h.run_id)
        .await
        .unwrap();
    let cancelled = h
        .runtime
        .director
        .get_run_detail(&h.novel_id, &h.run_id)
        .await
        .unwrap();
    assert_eq!(cancelled.status, "cancelled");
    let chapter = h
        .runtime
        .director
        .get_chapter(&h.novel_id, &h.chapter_id)
        .await
        .unwrap();
    assert_eq!(chapter.content, "", "取消不得写入正文");
    assert_eq!(chapter.official_version, 0);

    // 5. 取消命令重放：幂等，不产生新请求。
    let replay = h.runtime.director.cancel("g6-cancel-1", &h.run_id).await;
    assert!(replay.is_ok(), "取消重放必须幂等：{replay:?}");
    assert_eq!(hits.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn explicit_regeneration_uses_a_new_attempt_and_replays_idempotently() {
    let (base_url, hits) = silent_server().await;
    let h = harness(&base_url, 1_000).await;

    let _ = h.runtime.worker.tick().await.unwrap();
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    let paused = h
        .runtime
        .director
        .get_run_detail(&h.novel_id, &h.run_id)
        .await
        .unwrap();
    assert_eq!(paused.status, "paused");
    let calls_before = paused.usage.calls;

    // 用户明确选择重新生成：这是新的一次尝试，允许再发一次请求。
    h.runtime
        .director
        .resume("g6-resume-1", &h.run_id, false)
        .await
        .unwrap();
    for _ in 0..4 {
        let _ = h.runtime.worker.tick().await.unwrap();
    }
    assert!(
        hits.load(Ordering::SeqCst) > 1,
        "人工重新生成应产生新的远端尝试（实际 {} 次）",
        hits.load(Ordering::SeqCst)
    );
    let after_resume = h
        .runtime
        .director
        .get_run_detail(&h.novel_id, &h.run_id)
        .await
        .unwrap();
    assert!(
        after_resume.usage.calls >= calls_before,
        "用量统计必须可持续追踪：{} -> {}",
        calls_before,
        after_resume.usage.calls
    );

    // 同一恢复命令重放：幂等，不再新增请求。
    let hits_after = hits.load(Ordering::SeqCst);
    let replay = h
        .runtime
        .director
        .resume("g6-resume-1", &h.run_id, false)
        .await;
    assert!(replay.is_ok(), "恢复命令重放必须幂等：{replay:?}");
    for _ in 0..2 {
        let _ = h.runtime.worker.tick().await.unwrap();
    }
    assert_eq!(
        hits.load(Ordering::SeqCst),
        hits_after,
        "重放恢复命令不得再发请求"
    );

    // 计费未知的 Run 不得凭空写入正式正文。
    let chapter = h
        .runtime
        .director
        .get_chapter(&h.novel_id, &h.chapter_id)
        .await
        .unwrap();
    assert_eq!(chapter.official_version, 0);
}
