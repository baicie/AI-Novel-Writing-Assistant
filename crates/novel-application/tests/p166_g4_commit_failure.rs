//! R6-G4：远端已成功返回，但本地提交阶段失败 —— 不得重新发送生成请求。
//!
//! 用 FaultInjector 在「候选稿已落库、步骤尚未标记完成」的位置中断，
//! 再继续 tick，断言不会再次调用模型。

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use novel_application::{
    CredentialAction, CredentialIntents, CredentialVault, FaultInjector, ModelSecrets,
    OpenRunInput, PendingCredentialIntent, PortError, ProviderDraft, ProviderRegistry,
    ProviderService, Runtime, RuntimeDeps, TickOutcome,
};
use novel_domain::ProviderProtocol;
use novel_llm::OpenAiCompatibleProvider;
use novel_storage_sqlite::SqliteKernel;
use novel_testkit::{FakeClock, SequentialIds};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

struct TestSecrets;

#[async_trait]
impl ModelSecrets for TestSecrets {
    async fn api_key(&self, _provider: &str) -> Result<Option<String>, PortError> {
        Ok(Some("key-G4".into()))
    }
}

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

/// 只在 draft 步骤之后注入一次中断。
#[derive(Default)]
struct DraftInterrupt {
    used: Mutex<bool>,
    triggered: AtomicUsize,
}

#[async_trait]
impl FaultInjector for DraftInterrupt {
    async fn fail_after_candidate(&self, step_id: &str) -> bool {
        if step_id != "draft" {
            return false;
        }
        let mut used = self.used.lock().unwrap();
        if *used {
            return false;
        }
        *used = true;
        self.triggered.fetch_add(1, Ordering::SeqCst);
        true
    }
}

fn story() -> String {
    serde_json::json!({
        "expansion": {"expanded_premise":"提交失败","protagonist_core":"主角","conflict_engine":"冲突","conflict_layers":{"external":"外","internal":"内"},"mystery_box":"未知","emotional_line":"情绪","setpiece_seeds":["场面"],"tone_reference":"气质"},
        "decomposition": {"selling_point":"卖点","core_conflict":"冲突","main_hook":"钩子","progression_loop":"循环","growth_path":"成长","major_payoffs":["回报"],"ending_flavor":"结局"},
        "constraints":["规则"],"issues":[]
    })
    .to_string()
}
fn cast() -> String {
    serde_json::json!({"members":[
        {"name":"林动","role":"主角","castRole":"protagonist","storyFunction":"承载"},
        {"name":"苏晚","role":"同伴","castRole":"ally","storyFunction":"协助"},
        {"name":"陆沉","role":"对手","castRole":"antagonist","storyFunction":"施压"}],
        "relations":[{"sourceName":"林动","targetName":"苏晚","surfaceRelation":"同门"},{"sourceName":"林动","targetName":"陆沉","surfaceRelation":"敌对"}]}).to_string()
}
fn volume() -> String {
    serde_json::json!({"volumes":[{"volumeOrder":1,"title":"山门卷","roleLabel":"开局","coreReward":"入门","planningMode":"hard"}]}).to_string()
}
fn chapters() -> String {
    serde_json::json!({"chapters":[{"chapterOrder":1,"title":"第一章","volumeTitle":"山门卷","summary":"入山","characterNames":["林动"]}]}).to_string()
}

/// 记录请求次数；响应按 FIFO 剧本返回。
async fn mock(script: Vec<String>) -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let hits = Arc::new(AtomicUsize::new(0));
    let hits_task = hits.clone();
    let script = Arc::new(Mutex::new(std::collections::VecDeque::from(script)));
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let hits = hits_task.clone();
            let script = script.clone();
            tokio::spawn(async move {
                let mut buffer = vec![0_u8; 32 * 1024];
                let _ = socket.read(&mut buffer).await;
                hits.fetch_add(1, Ordering::SeqCst);
                let content = script
                    .lock()
                    .unwrap()
                    .pop_front()
                    .unwrap_or_else(|| "补充输出".to_string());
                let event =
                    serde_json::json!({"choices":[{"delta":{"content": content}}]}).to_string();
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
    (format!("http://{address}/v1"), hits)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn commit_failure_after_successful_generation_does_not_resend() {
    let (base_url, hits) = mock(vec![
        story(),
        cast(),
        volume(),
        chapters(),
        "候选正文：远端已经成功生成。".to_string(),
        "不应该被用到的第二次草稿".to_string(),
    ])
    .await;

    let directory = tempfile::tempdir().unwrap();
    let kernel = Arc::new(
        SqliteKernel::open(&directory.path().join("g4.sqlite"))
            .await
            .unwrap(),
    );
    let service = ProviderService::new(
        kernel.clone() as Arc<dyn ProviderRegistry>,
        Arc::new(TestVault) as Arc<dyn CredentialVault>,
        Arc::new(TestIntents),
    );
    service
        .upsert_provider(
            ProviderDraft {
                id: "gw".into(),
                display_name: Some("G4 网关".into()),
                protocol: ProviderProtocol::OpenAiCompatible,
                base_url: base_url.clone(),
                allow_localhost_http: true,
                is_builtin: false,
                sort_order: 0,
            },
            CredentialAction::Replace("key-G4".into()),
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
    kernel
        .set_binding_cas(
            &novel_domain::ModelBinding {
                task_type: "writer".into(),
                provider_id: "gw".into(),
                model_id: "model".into(),
                reasoning_effort: None,
            },
            None,
            1,
        )
        .await
        .unwrap();

    let step_routes = service.frozen_step_routes().await.unwrap();
    let faults = Arc::new(DraftInterrupt::default());
    let runtime = Runtime::new(RuntimeDeps {
        novels: kernel.clone(),
        runs: kernel.clone(),
        worlds: kernel.clone(),
        lookups: kernel.clone(),
        task_center: kernel.clone(),
        llm: Arc::new(OpenAiCompatibleProvider::new()),
        clock: Arc::new(FakeClock::new(1_000)),
        ids: Arc::new(SequentialIds::new()),
        faults: faults.clone(),
        secrets: Arc::new(TestSecrets),
        owner: "g4-worker".into(),
        lease_ms: 60_000,
        concurrency: 1,
    });
    let novel = runtime
        .director
        .create_novel("提交失败书", "")
        .await
        .unwrap();
    let chapter = runtime
        .director
        .create_chapter(&novel.id, "第一章", 1)
        .await
        .unwrap();
    let mut prompts = std::collections::BTreeMap::new();
    prompts.insert("chapter_draft".into(), "chapter-draft-v1".into());
    prompts.insert("consistency_review".into(), "consistency-review-v1".into());
    let opened = runtime
        .director
        .open_run(OpenRunInput {
            command_id: "g4-open".into(),
            novel_id: novel.id.clone(),
            target_chapter_id: chapter.id.clone(),
            provider: "gw".into(),
            model: "model".into(),
            prompt_versions: prompts,
            token_budget: Some(4096),
            context_token_budget: None,
            max_automatic_retries: 1,
            repair_budget: Some(1),
            endpoint: format!("{base_url}/chat/completions"),
            allow_localhost_http: true,
            step_routes,
            credential_ref: Some("gw#1".into()),
        })
        .await
        .unwrap();

    let mut interrupted = false;
    for tick in 0..40 {
        match runtime.worker.tick().await.unwrap() {
            TickOutcome::Interrupted { .. } => {
                interrupted = true;
                break;
            }
            TickOutcome::Advanced { status, run_id } => {
                if status == "waiting_approval" {
                    runtime
                        .director
                        .approve(&format!("g4-approve-{tick}"), &run_id)
                        .await
                        .unwrap();
                } else if status == "succeeded" || status == "paused" {
                    break;
                }
            }
            TickOutcome::Idle => break,
            other => panic!("unexpected tick outcome: {other:?}"),
        }
    }
    assert!(interrupted, "应当命中 draft 之后的注入点");
    assert_eq!(
        faults.triggered.load(Ordering::SeqCst),
        1,
        "注入点只应触发一次"
    );

    let after_interrupt = hits.load(Ordering::SeqCst);
    assert_eq!(after_interrupt, 5, "草稿应当只请求一次：{after_interrupt}");

    // 继续推进：允许系统恢复/重试，但绝不能再向远端要一次草稿。
    for _ in 0..6 {
        let _ = runtime.worker.tick().await.unwrap();
    }
    assert_eq!(
        hits.load(Ordering::SeqCst),
        after_interrupt,
        "本地提交失败后不得重发生成请求（实际从 {after_interrupt} 增加到 {}）",
        hits.load(Ordering::SeqCst)
    );

    // 候选稿与章节正文不应被重复写入。
    let candidates: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM workflow_artifact WHERE type='chapter_draft_candidate'",
    )
    .fetch_one(kernel.pool())
    .await
    .unwrap();
    assert!(candidates <= 1, "候选稿不应重复：{candidates}");
    let _ = opened.run_id;
}
