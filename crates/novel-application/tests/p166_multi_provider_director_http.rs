//! R4.1-C：真实 Director 四环节多 Provider（两个独立 Mock HTTP Server）。
//!
//! 断言依据是**实际到达两个服务器的 HTTP 请求**（路径、Authorization、model、body），
//! 不是 RunContract 字段。

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use novel_application::{
    CredentialAction, CredentialIntents, CredentialVault, ModelSecrets, OpenRunInput,
    PendingCredentialIntent, PortError, ProviderDraft, ProviderRegistry, ProviderService, Runtime,
    RuntimeDeps, TickOutcome,
};
use novel_domain::ProviderProtocol;
use novel_llm::OpenAiCompatibleProvider;
use novel_storage_sqlite::SqliteKernel;
use novel_testkit::{FakeClock, SequentialIds};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[derive(Debug, Clone)]
struct Record {
    method: String,
    path: String,
    authorization: Option<String>,
    body: String,
}

impl Record {
    fn model(&self) -> String {
        let value: serde_json::Value = serde_json::from_str(&self.body).unwrap_or_default();
        value
            .get("model")
            .and_then(|model| model.as_str())
            .unwrap_or_default()
            .to_string()
    }

}

/// 一个可编排的 Mock LLM Server：按 FIFO 返回剧本内容，并记录每次请求。
struct MockProvider {
    address: std::net::SocketAddr,
    records: Arc<Mutex<Vec<Record>>>,
}

impl MockProvider {
    async fn start(script: Vec<String>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let records = Arc::new(Mutex::new(Vec::new()));
        let records_for_task = records.clone();
        let script = Arc::new(Mutex::new(VecDeque::from(script)));
        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    return;
                };
                let records = records_for_task.clone();
                let script = script.clone();
                tokio::spawn(async move {
                    let mut buffer = vec![0_u8; 64 * 1024];
                    let read = socket.read(&mut buffer).await.unwrap_or(0);
                    let raw = String::from_utf8_lossy(&buffer[..read]).to_string();
                    let (head, body) = raw.split_once("\r\n\r\n").unwrap_or((raw.as_str(), ""));
                    let mut lines = head.lines();
                    let request_line = lines.next().unwrap_or_default();
                    let mut parts = request_line.split_whitespace();
                    let method = parts.next().unwrap_or_default().to_string();
                    let path = parts.next().unwrap_or_default().to_string();
                    let mut authorization = None;
                    for line in lines {
                        if line.to_ascii_lowercase().starts_with("authorization:") {
                            authorization = Some(
                                line.split_once(':')
                                    .map(|(_, v)| v.trim().to_string())
                                    .unwrap_or_default(),
                            );
                        }
                    }
                    records.lock().unwrap().push(Record {
                        method,
                        path,
                        authorization,
                        body: body.to_string(),
                    });
                    let content = script
                        .lock()
                        .unwrap()
                        .pop_front()
                        .unwrap_or_else(|| "{\"issues\":[]}".to_string());
                    let event =
                        serde_json::json!({ "choices": [{ "delta": { "content": content } }] })
                            .to_string();
                    let payload = format!("data: {event}\n\ndata: [DONE]\n\n");
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        payload.len(),
                        payload
                    );
                    let _ = socket.write_all(response.as_bytes()).await;
                    let _ = socket.flush().await;
                });
            }
        });
        Self { address, records }
    }

    fn base_url(&self) -> String {
        format!("http://{}/v1", self.address)
    }

    fn endpoint(&self) -> String {
        format!("http://{}/v1/chat/completions", self.address)
    }

    fn records(&self) -> Vec<Record> {
        self.records.lock().unwrap().clone()
    }
}

#[derive(Default)]
struct TestVault {
    values: Mutex<std::collections::HashMap<String, String>>,
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

fn story() -> String {
    serde_json::json!({
        "expansion": {"expanded_premise":"少年入山。","protagonist_core":"被逐少年","conflict_engine":"宗门秘密","conflict_layers":{"external":"外","internal":"内"},"mystery_box":"祖祠","emotional_line":"成长","setpiece_seeds":["夜战"],"tone_reference":"冷峻"},
        "decomposition": {"selling_point":"逆袭","core_conflict":"真相","main_hook":"血脉","progression_loop":"发现","growth_path":"承担","major_payoffs":["真相"],"ending_flavor":"新生"},
        "constraints":["不揭露终极秘密"],"issues":[]
    })
    .to_string()
}

fn cast() -> String {
    serde_json::json!({"members":[
        {"name":"林动","role":"主角","castRole":"protagonist","storyFunction":"承载真相"},
        {"name":"苏晚","role":"同伴","castRole":"ally","storyFunction":"协助"},
        {"name":"陆沉","role":"对手","castRole":"antagonist","storyFunction":"施压"}],
        "relations":[{"sourceName":"林动","targetName":"苏晚","surfaceRelation":"同门"},{"sourceName":"林动","targetName":"陆沉","surfaceRelation":"敌对"}]})
    .to_string()
}

fn volume() -> String {
    serde_json::json!({"volumes":[{"volumeOrder":1,"title":"山门卷","roleLabel":"开局","coreReward":"入门","planningMode":"hard"}]}).to_string()
}

fn chapters() -> String {
    serde_json::json!({"chapters":[{"chapterOrder":1,"title":"第一章","volumeTitle":"山门卷","summary":"入山","characterNames":["林动"]}]}).to_string()
}

fn issues() -> String {
    serde_json::json!({"issues":[{"type":"timeline","severity":"high","evidence":"时间线冲突","suggestion":"修正时间"}]}).to_string()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn four_steps_route_to_two_real_mock_providers() {
    // A 负责 planner + review；B 负责 writer + repair。
    let provider_a = MockProvider::start(vec![
        story(),
        cast(),
        volume(),
        chapters(),
        issues(),
        "{\"issues\":[]}".to_string(),
        "{\"issues\":[]}".to_string(),
    ])
    .await;
    let provider_b = MockProvider::start(vec![
        "候选正文：少年入山。".to_string(),
        "修复后的候选正文：少年入山，风雪扑面。".to_string(),
        "修复后的候选正文：覆盖。".to_string(),
    ])
    .await;

    let directory = tempfile::tempdir().unwrap();
    let kernel = Arc::new(
        SqliteKernel::open(&directory.path().join("multi.sqlite"))
            .await
            .unwrap(),
    );
    let vault = Arc::new(TestVault::default());
    let service = ProviderService::new(
        kernel.clone() as Arc<dyn ProviderRegistry>,
        vault.clone() as Arc<dyn CredentialVault>,
        Arc::new(TestIntents),
    );

    let draft_a = ProviderDraft {
        id: "gw-a".into(),
        display_name: Some("A 网关".into()),
        protocol: ProviderProtocol::OpenAiCompatible,
        base_url: provider_a.base_url(),
        allow_localhost_http: true,
        is_builtin: false,
        sort_order: 0,
    };
    let draft_b = ProviderDraft {
        id: "gw-b".into(),
        display_name: Some("B 网关".into()),
        protocol: ProviderProtocol::OpenAiCompatible,
        base_url: provider_b.base_url(),
        allow_localhost_http: true,
        is_builtin: false,
        sort_order: 1,
    };
    service
        .upsert_provider(draft_a, CredentialAction::Replace("key-A".into()), None, 1)
        .await
        .unwrap();
    service
        .upsert_provider(draft_b, CredentialAction::Replace("key-B".into()), None, 1)
        .await
        .unwrap();
    for (provider, model) in [
        ("gw-a", "model-plan"),
        ("gw-a", "model-review"),
        ("gw-b", "model-write"),
        ("gw-b", "model-repair"),
    ] {
        service
            .upsert_model(
                novel_domain::ModelProfile {
                    provider_id: provider.into(),
                    model_id: model.into(),
                    display_name: Some(model.into()),
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
    }
    for (task, provider, model) in [
        ("planner", "gw-a", "model-plan"),
        ("writer", "gw-b", "model-write"),
        ("review", "gw-a", "model-review"),
        ("repair", "gw-b", "model-repair"),
    ] {
        kernel
            .set_binding_cas(
                &novel_domain::ModelBinding {
                    task_type: task.into(),
                    provider_id: provider.into(),
                    model_id: model.into(),
                    reasoning_effort: None,
                },
                None,
                1,
            )
            .await
            .unwrap();
    }

    let step_routes = service.frozen_step_routes().await.unwrap();
    assert_eq!(step_routes.len(), 4, "四个环节都应冻结：{step_routes:?}");
    assert_eq!(step_routes["writer"].provider_id, "gw-b");
    assert_eq!(step_routes["review"].provider_id, "gw-a");

    let runtime = Runtime::new(RuntimeDeps {
        novels: kernel.clone(),
        runs: kernel.clone(),
        worlds: kernel.clone(),
        lookups: kernel.clone(),
        task_center: kernel.clone(),
        llm: Arc::new(OpenAiCompatibleProvider::new()),
        clock: Arc::new(FakeClock::new(1_000)),
        ids: Arc::new(SequentialIds::new()),
        faults: Arc::new(novel_application::NoFault),
        secrets: Arc::new(TestSecrets(vault.clone())),
        owner: "multi-worker".into(),
        lease_ms: 60_000,
        concurrency: 1,
    });
    let novel = runtime.director.create_novel("多模型书", "").await.unwrap();
    let chapter = runtime
        .director
        .create_chapter(&novel.id, "第一章", 1)
        .await
        .unwrap();
    let mut prompts = BTreeMap::new();
    prompts.insert("chapter_draft".to_string(), "chapter-draft-v1".to_string());
    prompts.insert(
        "consistency_review".to_string(),
        "consistency-review-v1".to_string(),
    );
    prompts.insert(
        "chapter_repair".to_string(),
        "chapter-repair-v1".to_string(),
    );
    let opened = runtime
        .director
        .open_run(OpenRunInput {
            command_id: "multi-open".into(),
            novel_id: novel.id.clone(),
            target_chapter_id: chapter.id.clone(),
            provider: "gw-a".into(),
            model: "model-plan".into(),
            prompt_versions: prompts,
            token_budget: Some(4096),
            context_token_budget: None,
            max_automatic_retries: 0,
            repair_budget: Some(1),
            endpoint: provider_a.endpoint(),
            allow_localhost_http: true,
            step_routes,
            credential_ref: Some("gw-a#1".into()),
        })
        .await
        .unwrap();

    // 驱动真实 Worker 到终态或人工确认点。
    let mut approvals = 0;
    for tick in 0..80 {
        match runtime.worker.tick().await.unwrap() {
            TickOutcome::Advanced { status, run_id } => {
                if status == "waiting_approval" {
                    runtime
                        .director
                        .approve(&format!("multi-approve-{tick}"), &run_id)
                        .await
                        .unwrap();
                    approvals += 1;
                }
                if status == "paused" {
                    let detail = runtime
                        .director
                        .get_run_detail(&novel.id, &run_id)
                        .await
                        .unwrap();
                    if detail
                        .legal_actions
                        .iter()
                        .any(|action| action == "approve")
                    {
                        runtime
                            .director
                            .approve(&format!("multi-approve-{tick}"), &run_id)
                            .await
                            .unwrap();
                        approvals += 1;
                        continue;
                    }
                    if detail.legal_actions.iter().any(|action| action == "resume") {
                        runtime
                            .director
                            .resume(&format!("multi-resume-{tick}"), &run_id, false)
                            .await
                            .unwrap();
                        approvals += 1;
                        continue;
                    }
                    break;
                }
                if status == "succeeded" {
                    break;
                }
            }
            TickOutcome::Idle => break,
            other => panic!("unexpected tick outcome: {other:?}"),
        }
    }

    let records_a = provider_a.records();
    let records_b = provider_b.records();
    assert!(!records_a.is_empty(), "A 必须收到 planner/review 请求");
    assert!(!records_b.is_empty(), "B 必须收到 writer/repair 请求");

    for record in &records_a {
        assert_eq!(record.method, "POST");
        assert_eq!(record.path, "/v1/chat/completions");
        assert_eq!(
            record.authorization.as_deref(),
            Some("Bearer key-A"),
            "A 的请求必须带 A 的密钥：{record:?}"
        );
        assert!(
            record.model() == "model-plan" || record.model() == "model-review",
            "A 只应收到 A 的模型，实际 {}",
            record.model()
        );
        assert!(!record.body.contains("key-A") && !record.body.contains("key-B"));
    }
    for record in &records_b {
        assert_eq!(record.method, "POST");
        assert_eq!(record.path, "/v1/chat/completions");
        assert_eq!(
            record.authorization.as_deref(),
            Some("Bearer key-B"),
            "B 的请求必须带 B 的密钥：{record:?}"
        );
        assert!(
            record.model() == "model-write" || record.model() == "model-repair",
            "B 只应收到 B 的模型，实际 {}",
            record.model()
        );
    }

    assert!(
        records_a
            .iter()
            .any(|record| record.model() == "model-plan"),
        "planner 必须打到 A"
    );
    assert!(
        records_b
            .iter()
            .any(|record| record.model() == "model-write"),
        "writer 必须打到 B"
    );
    assert!(
        records_a
            .iter()
            .any(|record| record.model() == "model-review"),
        "review 必须打到 A"
    );
    assert!(
        records_b
            .iter()
            .any(|record| record.model() == "model-repair"),
        "repair 必须被真实触发并打到 B（需要审校返回问题）"
    );

    // 合同里不能出现密钥明文。
    let contract_json: String =
        sqlx::query_scalar("SELECT contract_json FROM workflow_run WHERE id = ?")
            .bind(&opened.run_id)
            .fetch_one(kernel.pool())
            .await
            .unwrap();
    assert!(!contract_json.contains("key-A"));
    assert!(!contract_json.contains("key-B"));
    assert!(contract_json.contains("gw-b#1"), "冻结的密钥引用应在合同里");

    // SQLite 里有对应的工作流记录。
    let run_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workflow_run")
        .fetch_one(kernel.pool())
        .await
        .unwrap();
    assert_eq!(run_count, 1);
    assert!(approvals <= 80);
}

/// R4.2-D：Run 冻结旧路由与旧密钥；轮换后旧 Run 用旧配置，新 Run 用新配置。
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn paused_run_keeps_frozen_route_and_old_key_after_rotation() {
    // A：planner + review；B：writer + repair；B2：轮换后的新 Endpoint（必须对旧 Run 零请求）。
    let mut script_a = Vec::new();
    for _ in 0..2 {
        script_a.push(story());
        script_a.push(cast());
        script_a.push(volume());
        script_a.push(chapters());
        script_a.push(issues());
        script_a.push("{\"issues\":[]}".to_string());
        script_a.push("{\"issues\":[]}".to_string());
    }
    let provider_a = MockProvider::start(script_a).await;
    let provider_b = MockProvider::start(vec![
        "候选正文：旧端点产出。".to_string(),
        "修复后的候选正文：旧端点修复。".to_string(),
        "候选正文：旧端点产出二。".to_string(),
    ])
    .await;
    let provider_b2 = MockProvider::start(vec![
        "候选正文：新端点产出。".to_string(),
        "修复后的候选正文：新端点修复。".to_string(),
    ])
    .await;

    let directory = tempfile::tempdir().unwrap();
    let kernel = Arc::new(
        SqliteKernel::open(&directory.path().join("rotate.sqlite"))
            .await
            .unwrap(),
    );
    let vault = Arc::new(TestVault::default());
    let service = ProviderService::new(
        kernel.clone() as Arc<dyn ProviderRegistry>,
        vault.clone() as Arc<dyn CredentialVault>,
        Arc::new(TestIntents),
    );

    let draft = |id: &str, name: &str, base: String, order: i32| ProviderDraft {
        id: id.into(),
        display_name: Some(name.into()),
        protocol: ProviderProtocol::OpenAiCompatible,
        base_url: base,
        allow_localhost_http: true,
        is_builtin: false,
        sort_order: order,
    };
    service
        .upsert_provider(
            draft("gw-a", "A 网关", provider_a.base_url(), 0),
            CredentialAction::Replace("key-A".into()),
            None,
            1,
        )
        .await
        .unwrap();
    service
        .upsert_provider(
            draft("gw-b", "B 网关", provider_b.base_url(), 1),
            CredentialAction::Replace("key-B".into()),
            None,
            1,
        )
        .await
        .unwrap();
    for (provider, model) in [
        ("gw-a", "model-plan"),
        ("gw-a", "model-review"),
        ("gw-b", "model-write"),
        ("gw-b", "model-repair"),
        ("gw-b", "model-write2"),
    ] {
        service
            .upsert_model(
                novel_domain::ModelProfile {
                    provider_id: provider.into(),
                    model_id: model.into(),
                    display_name: Some(model.into()),
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
    }
    for (task, provider, model) in [
        ("planner", "gw-a", "model-plan"),
        ("writer", "gw-b", "model-write"),
        ("review", "gw-a", "model-review"),
        ("repair", "gw-b", "model-repair"),
    ] {
        kernel
            .set_binding_cas(
                &novel_domain::ModelBinding {
                    task_type: task.into(),
                    provider_id: provider.into(),
                    model_id: model.into(),
                    reasoning_effort: None,
                },
                None,
                1,
            )
            .await
            .unwrap();
    }

    let runtime = Runtime::new(RuntimeDeps {
        novels: kernel.clone(),
        runs: kernel.clone(),
        worlds: kernel.clone(),
        lookups: kernel.clone(),
        task_center: kernel.clone(),
        llm: Arc::new(OpenAiCompatibleProvider::new()),
        clock: Arc::new(FakeClock::new(1_000)),
        ids: Arc::new(SequentialIds::new()),
        faults: Arc::new(novel_application::NoFault),
        secrets: Arc::new(TestSecrets(vault.clone())),
        owner: "rotate-worker".into(),
        lease_ms: 60_000,
        concurrency: 1,
    });
    let novel = runtime.director.create_novel("轮换书", "").await.unwrap();
    let chapter = runtime
        .director
        .create_chapter(&novel.id, "第一章", 1)
        .await
        .unwrap();

    let open_input =
        |command: &str,
         chapter_id: String,
         endpoint: String,
         routes: std::collections::BTreeMap<String, novel_application::FrozenStepRoute>,
         model: &str| OpenRunInput {
            command_id: command.into(),
            novel_id: novel.id.clone(),
            target_chapter_id: chapter_id,
            provider: "gw-a".into(),
            model: model.into(),
            prompt_versions: {
                let mut prompts = BTreeMap::new();
                prompts.insert("chapter_draft".to_string(), "chapter-draft-v1".to_string());
                prompts.insert(
                    "consistency_review".to_string(),
                    "consistency-review-v1".to_string(),
                );
                prompts.insert(
                    "chapter_repair".to_string(),
                    "chapter-repair-v1".to_string(),
                );
                prompts
            },
            token_budget: Some(4096),
            context_token_budget: None,
            max_automatic_retries: 0,
            repair_budget: Some(1),
            endpoint,
            allow_localhost_http: true,
            step_routes: routes,
            credential_ref: Some("gw-a#1".into()),
        };

    // ---- Run-Old：冻结当前配置 ----
    let frozen_old = service.frozen_step_routes().await.unwrap();
    assert_eq!(frozen_old["writer"].endpoint, provider_b.base_url());
    let run_old = runtime
        .director
        .open_run(open_input(
            "rotate-open-old",
            chapter.id.clone(),
            provider_a.endpoint(),
            frozen_old,
            "model-plan",
        ))
        .await
        .unwrap();

    // 推进到第一个业务暂停点（此时已经用旧配置请求过 A）。
    let mut paused = false;
    for tick in 0..4 {
        if let TickOutcome::Advanced { status, run_id } = runtime.worker.tick().await.unwrap()
            && status == "waiting_approval"
        {
            {
                paused = true;
                let detail = runtime
                    .director
                    .get_run_detail(&novel.id, &run_id)
                    .await
                    .unwrap();
                let resumable = detail
                    .legal_actions
                    .iter()
                    .any(|action| action == "approve" || action == "confirm");
                assert!(
                    resumable,
                    "需要在可恢复的业务暂停点，实际动作：{:?}",
                    detail.legal_actions
                );
                let _ = tick;
                break;
            }
        }
    }
    assert!(paused, "Run-Old 应当停在可恢复的人工确认点");
    assert!(!provider_a.records().is_empty(), "旧 Run 已经请求过 A");

    // ---- 轮换：A/B 换密钥，B 换 Endpoint，writer 换模型 ----
    let views = service.list_providers().await.unwrap();
    let revision_a = views
        .iter()
        .find(|view| view.id == "gw-a")
        .unwrap()
        .revision;
    let revision_b = views
        .iter()
        .find(|view| view.id == "gw-b")
        .unwrap()
        .revision;
    service
        .upsert_provider(
            draft("gw-a", "A 网关", provider_a.base_url(), 0),
            CredentialAction::Replace("key-A2".into()),
            Some(revision_a),
            2,
        )
        .await
        .unwrap();
    // 换到另一个主机端口属于「明文 HTTP 主机变更」，产品要求显式确认；这里模拟用户确认。
    service
        .upsert_provider_with_confirmation(
            draft("gw-b", "B 网关", provider_b2.base_url(), 1),
            CredentialAction::Replace("key-B2".into()),
            Some(revision_b),
            2,
            true,
        )
        .await
        .unwrap();
    let revision_b2 = service
        .list_providers()
        .await
        .unwrap()
        .into_iter()
        .find(|view| view.id == "gw-b")
        .unwrap()
        .revision;
    kernel
        .set_binding_cas(
            &novel_domain::ModelBinding {
                task_type: "writer".into(),
                provider_id: "gw-b".into(),
                model_id: "model-write2".into(),
                reasoning_effort: None,
            },
            Some(revision_b2),
            2,
        )
        .await
        .unwrap();

    // 旧槽必须仍然存在（旧 Run 还要用）。
    assert_eq!(
        vault.values.lock().unwrap().get("gw-b#1").cloned(),
        Some("key-B".to_string()),
        "轮换不能删除旧密钥槽"
    );

    // ---- 轮换完成后再放行旧 Run（模拟作者在旧配置下继续） ----
    let pending = runtime
        .director
        .get_run_detail(&novel.id, &run_old.run_id)
        .await
        .unwrap();
    assert!(
        pending
            .legal_actions
            .iter()
            .any(|action| action == "approve" || action == "confirm"),
        "轮换后旧 Run 仍应停在可恢复点，实际动作：{:?}",
        pending.legal_actions
    );
    runtime
        .director
        .approve("rotate-resume-old", &run_old.run_id)
        .await
        .unwrap();

    // ---- 继续旧 Run 到终态 ----
    for tick in 0..40 {
        match runtime.worker.tick().await.unwrap() {
            TickOutcome::Advanced { status, run_id } => {
                if status == "waiting_approval" {
                    if let Ok(detail) = runtime.director.get_run_detail(&novel.id, &run_id).await
                        && detail
                            .legal_actions
                            .iter()
                            .any(|action| action == "approve" || action == "confirm")
                    {
                        runtime
                            .director
                            .approve(&format!("rotate-approve-{tick}"), &run_id)
                            .await
                            .unwrap();
                    }
                } else if status == "succeeded" || status == "paused" {
                    break;
                }
            }
            TickOutcome::Idle => break,
            other => panic!("unexpected tick outcome: {other:?}"),
        }
    }

    // ---- 旧 Run 的实际请求：旧密钥 + 旧端点 ----
    let old_a = provider_a.records();
    let old_b = provider_b.records();
    assert!(
        old_a.len() >= 3,
        "旧 Run 应当多次请求 A，实际 {}",
        old_a.len()
    );
    assert!(
        !old_b.is_empty(),
        "旧 Run 的 writer/repair 请求必须打到旧 B 端点"
    );
    for record in &old_a {
        assert_eq!(
            record.authorization.as_deref(),
            Some("Bearer key-A"),
            "旧 Run 必须继续用旧密钥：{record:?}"
        );
    }
    for record in &old_b {
        assert_eq!(
            record.authorization.as_deref(),
            Some("Bearer key-B"),
            "旧 Run 的 B 请求必须继续用旧密钥：{record:?}"
        );
        assert_ne!(record.model(), "model-write2", "旧 Run 不能改用新模型");
    }
    assert!(
        provider_b2.records().is_empty(),
        "轮换后，旧 Run 不得打到新 Endpoint（实际 {} 次）",
        provider_b2.records().len()
    );

    // ---- Run-New：使用新配置 ----
    let frozen_new = service.frozen_step_routes().await.unwrap();
    assert_eq!(frozen_new["writer"].endpoint, provider_b2.base_url());
    assert_eq!(frozen_new["writer"].model_id, "model-write2");
    assert_eq!(frozen_new["writer"].credential_ref, "gw-b#2");

    let chapter_two = runtime
        .director
        .create_chapter(&novel.id, "第二章", 2)
        .await
        .unwrap();
    let run_new = runtime
        .director
        .open_run(open_input(
            "rotate-open-new",
            chapter_two.id.clone(),
            provider_a.endpoint(),
            frozen_new,
            "model-plan",
        ))
        .await
        .unwrap();
    for tick in 0..30 {
        match runtime.worker.tick().await.unwrap() {
            TickOutcome::Advanced { status, run_id } => {
                if status == "waiting_approval" {
                    runtime
                        .director
                        .approve(&format!("rotate-new-approve-{tick}"), &run_id)
                        .await
                        .unwrap();
                }
                if status == "succeeded" || status == "paused" {
                    break;
                }
                if !provider_b2.records().is_empty() {
                    break;
                }
            }
            TickOutcome::Idle => break,
            other => panic!("unexpected tick outcome: {other:?}"),
        }
    }
    let new_b = provider_b2.records();
    assert!(!new_b.is_empty(), "新 Run 必须打到新 Endpoint");
    assert_eq!(
        new_b[0].authorization.as_deref(),
        Some("Bearer key-B2"),
        "新 Run 必须使用新密钥：{:?}",
        new_b[0]
    );
    assert_eq!(new_b[0].model(), "model-write2", "新 Run 应使用新绑定模型");

    // 旧 Run 的请求数在轮换后没有再打新端点。
    assert_ne!(run_old.run_id, run_new.run_id);
}
