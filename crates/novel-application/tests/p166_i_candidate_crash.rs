//! R8.2-I1：候选稿确认过程中的进程级崩溃（真实 SIGKILL + 真实 SQLite）。
//!
//! 子进程复用本测试二进制，通过环境变量驱动：
//!   P166_I_DB / P166_I_ENDPOINT / P166_I_MARKER / P166_I_MODE
//! 父进程只负责起 Mock、发信号、重启后断言持久化事实。

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use novel_application::{
    CredentialAction, CredentialIntents, CredentialVault, ModelSecrets, OpenRunInput,
    PendingCredentialIntent, PortError, ProviderDraft, ProviderRegistry, ProviderService, Runtime,
    RuntimeDeps,
};
use novel_domain::ProviderProtocol;
use novel_llm::OpenAiCompatibleProvider;
use novel_storage_sqlite::SqliteKernel;
use novel_testkit::{FakeClock, SequentialIds};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

// ---------- 最小端口实现 ----------

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
        Ok(Some("key-I".into()))
    }
}

// ---------- Mock LLM ----------

fn story() -> String {
    serde_json::json!({
        "expansion": {"expanded_premise":"崩溃","protagonist_core":"主角","conflict_engine":"冲突","conflict_layers":{"external":"外","internal":"内"},"mystery_box":"未知","emotional_line":"情绪","setpiece_seeds":["场面"],"tone_reference":"气质"},
        "decomposition": {"selling_point":"卖点","core_conflict":"冲突","main_hook":"钩子","progression_loop":"循环","growth_path":"成长","major_payoffs":["回报"],"ending_flavor":"结局"},
        "constraints":["规则"],"issues":[]
    })
    .to_string()
}

fn script() -> Vec<String> {
    vec![
        story(),
        serde_json::json!({"members":[
            {"name":"林动","role":"主角","castRole":"protagonist","storyFunction":"承载"},
            {"name":"苏晚","role":"同伴","castRole":"ally","storyFunction":"协助"},
            {"name":"陆沉","role":"对手","castRole":"antagonist","storyFunction":"施压"}],
            "relations":[{"sourceName":"林动","targetName":"苏晚","surfaceRelation":"同门"},{"sourceName":"林动","targetName":"陆沉","surfaceRelation":"敌对"}]})
        .to_string(),
        serde_json::json!({"volumes":[{"volumeOrder":1,"title":"山门卷","roleLabel":"开局","coreReward":"入门","planningMode":"hard"}]}).to_string(),
        serde_json::json!({"chapters":[{"chapterOrder":1,"title":"第一章","volumeTitle":"山门卷","summary":"入山","characterNames":["林动"]}]}).to_string(),
        "候选正文：崩溃测试第一版。".to_string(),
        serde_json::json!({"issues":[{"type":"timeline","severity":"high","evidence":"时间线冲突","suggestion":"修正"}]}).to_string(),
        "崩溃测试正式正文：少年入山，风雪扑面。".to_string(),
        "{\"issues\":[]}".to_string(),
    ]
}

async fn mock() -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let hits = Arc::new(AtomicUsize::new(0));
    let hits_task = hits.clone();
    let queue = Arc::new(Mutex::new(std::collections::VecDeque::from(script())));
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let hits = hits_task.clone();
            let queue = queue.clone();
            tokio::spawn(async move {
                let mut buffer = vec![0_u8; 32 * 1024];
                let _ = socket.read(&mut buffer).await;
                hits.fetch_add(1, Ordering::SeqCst);
                let content = queue
                    .lock()
                    .unwrap()
                    .pop_front()
                    .unwrap_or_else(|| "{}".to_string());
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

// ---------- 子进程主体 ----------

fn env(key: &str) -> Option<String> {
    std::env::var(key).ok()
}

/// 建库 + 建 Run + 推进到候选稿门点；返回 (run_id, novel_id, chapter_id, candidate_id, sha, content, version)。
async fn prepare(
    db: &str,
    endpoint: &str,
) -> (String, String, String, String, String, String, i64) {
    let kernel = Arc::new(SqliteKernel::open(std::path::Path::new(db)).await.unwrap());
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
                display_name: Some("I 网关".into()),
                protocol: ProviderProtocol::OpenAiCompatible,
                base_url: endpoint.trim_end_matches("/chat/completions").to_string(),
                allow_localhost_http: true,
                is_builtin: false,
                sort_order: 0,
            },
            CredentialAction::Replace("key-I".into()),
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
    for task in ["planner", "writer", "review", "repair"] {
        kernel
            .set_binding_cas(
                &novel_domain::ModelBinding {
                    task_type: task.into(),
                    provider_id: "gw".into(),
                    model_id: "model".into(),
                    reasoning_effort: None,
                },
                None,
                1,
            )
            .await
            .unwrap();
    }
    let routes = service.frozen_step_routes().await.unwrap();
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
        secrets: Arc::new(TestSecrets),
        owner: "i-child".into(),
        lease_ms: 60_000,
        concurrency: 1,
    });
    let novel = runtime.director.create_novel("I 崩溃书", "").await.unwrap();
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
            command_id: "i-open".into(),
            novel_id: novel.id.clone(),
            target_chapter_id: chapter.id.clone(),
            provider: "gw".into(),
            model: "model".into(),
            prompt_versions: prompts,
            token_budget: Some(4096),
            context_token_budget: None,
            max_automatic_retries: 1,
            repair_budget: Some(1),
            endpoint: endpoint.to_string(),
            allow_localhost_http: true,
            step_routes: routes,
            credential_ref: Some("gw#1".into()),
        })
        .await
        .unwrap();

    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        assert!(tokio::time::Instant::now() < deadline, "等不到候选稿门点");
        let detail = runtime
            .director
            .get_run_detail(&novel.id, &opened.run_id)
            .await
            .unwrap();
        let at_approval = detail
            .gate
            .as_ref()
            .map(|gate| gate.id == "approval")
            .unwrap_or(false);
        if at_approval
            && let Some(candidate) = detail.latest_candidate.clone()
            && !candidate.id.is_empty()
        {
            return (
                opened.run_id,
                novel.id,
                chapter.id,
                candidate.id,
                candidate.content_sha256,
                candidate.content,
                detail.control_version,
            );
        }
        if detail.status == "waiting_approval"
            && detail
                .legal_actions
                .iter()
                .any(|action| action == "approve" || action == "confirm")
        {
            let _ = runtime
                .director
                .approve(
                    &format!("i-approve-{}", detail.control_version),
                    &opened.run_id,
                )
                .await;
        }
        let _ = runtime.worker.tick().await.unwrap();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn i_child_process() {
    let (Some(db), Some(endpoint), Some(marker), Some(mode)) = (
        env("P166_I_DB"),
        env("P166_I_ENDPOINT"),
        env("P166_I_MARKER"),
        env("P166_I_MODE"),
    ) else {
        return;
    };

    let (run_id, novel_id, chapter_id, candidate_id, sha, content, version) =
        prepare(&db, &endpoint).await;
    std::fs::write(
        &marker,
        serde_json::json!({
            "runId": run_id,
            "novelId": novel_id,
            "chapterId": chapter_id,
            "candidateId": candidate_id,
            "sha": sha,
            "content": content,
            "version": version,
        })
        .to_string(),
    )
    .unwrap();

    let kernel = Arc::new(SqliteKernel::open(std::path::Path::new(&db)).await.unwrap());
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
        secrets: Arc::new(TestSecrets),
        owner: "i-child".into(),
        lease_ms: 60_000,
        concurrency: 1,
    });

    if mode == "commit-then-hang" {
        let _ = runtime
            .director
            .confirm_candidate_after_edit(
                "i-confirm-1",
                &run_id,
                Some(version),
                &candidate_id,
                &sha,
                &content,
            )
            .await;
        // 确认之后 worker 才会把候选稿写成正式正文；这里一直推进到成功。
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(60);
        loop {
            assert!(
                tokio::time::Instant::now() < deadline,
                "子进程推进不到成功状态"
            );
            let detail = runtime
                .director
                .get_run_detail(&novel_id, &run_id)
                .await
                .unwrap();
            if detail.status == "succeeded" {
                break;
            }
            if detail.status == "waiting_approval"
                && detail
                    .legal_actions
                    .iter()
                    .any(|action| action == "approve" || action == "confirm")
            {
                let _ = runtime
                    .director
                    .approve(&format!("i-post-{}", detail.control_version), &run_id)
                    .await;
            }
            let _ = runtime.worker.tick().await;
        }
        // 持久化已经完成，客户端还没有机会把响应送出去。
        std::fs::write(format!("{marker}.committed"), "ok").unwrap();
    }

    // 无论是提交前阻塞还是提交后挂起，都停在这里等父进程 SIGKILL。
    tokio::time::sleep(std::time::Duration::from_secs(600)).await;
}

fn read_marker(path: &std::path::Path) -> Option<serde_json::Value> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
}

async fn spawn_child(
    db: &str,
    endpoint: &str,
    marker: &std::path::Path,
    mode: &str,
) -> std::process::Child {
    std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "i_child_process", "--nocapture"])
        .env("P166_I_DB", db)
        .env("P166_I_ENDPOINT", endpoint)
        .env("P166_I_MARKER", marker.to_string_lossy().to_string())
        .env("P166_I_MODE", mode)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap()
}

async fn wait_for(path: &std::path::Path) -> serde_json::Value {
    let mut waited = 0;
    while read_marker(path).is_none() {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        waited += 1;
        assert!(waited < 600, "等不到子进程标记文件");
    }
    read_marker(path).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn crash_after_commit_keeps_content_and_replay_is_idempotent() {
    let (endpoint, hits) = mock().await;
    let directory = tempfile::tempdir().unwrap();
    let db_path = directory.path().join("i1.sqlite");
    let db = db_path.to_string_lossy().to_string();
    let marker = directory.path().join("marker.json");

    let mut child = spawn_child(&db, &endpoint, &marker, "commit-then-hang").await;
    let info = wait_for(&marker).await;
    // 等子进程真正完成确认。
    let mut waited = 0;
    while !std::path::Path::new(&format!("{}.committed", marker.to_string_lossy())).exists() {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        waited += 1;
        assert!(waited < 600, "子进程没有完成确认");
    }
    let _ = child.kill();
    let _ = child.wait();

    let hits_before_restart = hits.load(Ordering::SeqCst);
    let content = info["content"].as_str().unwrap().to_string();
    let version = info["version"].as_i64().unwrap();

    // 重启：直接读数据库事实。
    let kernel = Arc::new(SqliteKernel::open(&db_path).await.unwrap());
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
        secrets: Arc::new(TestSecrets),
        owner: "i-restart".into(),
        lease_ms: 60_000,
        concurrency: 1,
    });
    // 提交后崩溃：正文必须已经落库（客户端没收到响应不代表没成功）。
    // 这里不再驱动 worker：崩溃进程的租约还没过期，接管会按预期返回 Conflict。
    let chapter = runtime
        .director
        .get_chapter(
            info["novelId"].as_str().unwrap(),
            info["chapterId"].as_str().unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(chapter.content, content, "已提交的正式正文必须保留");
    let version_after = chapter.official_version;
    assert!(version_after > 0, "正式版本必须已递增");

    // 重放同一 command_id：必须幂等，不得二次提交。
    let replay = runtime
        .director
        .confirm_candidate_after_edit(
            "i-confirm-1",
            info["runId"].as_str().unwrap(),
            Some(version),
            info["candidateId"].as_str().unwrap(),
            info["sha"].as_str().unwrap(),
            &content,
        )
        .await;
    assert!(replay.is_ok(), "同一命令重放必须幂等：{replay:?}");
    let chapter_replay = runtime
        .director
        .get_chapter(
            info["novelId"].as_str().unwrap(),
            info["chapterId"].as_str().unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(chapter_replay.content, content, "重放不得改变已确认正文");
    assert_eq!(
        chapter_replay.official_version, version_after,
        "重放不得二次递增正式版本"
    );
    assert_eq!(
        hits.load(Ordering::SeqCst),
        hits_before_restart,
        "重启与重放都不得再次请求模型"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn crash_before_commit_leaves_content_untouched_and_recoverable() {
    let (endpoint, hits) = mock().await;
    let directory = tempfile::tempdir().unwrap();
    let db_path = directory.path().join("i1a.sqlite");
    let db = db_path.to_string_lossy().to_string();
    let marker = directory.path().join("marker-i1a.json");

    // 子进程只推进到候选稿门点，不做确认；随后被外部 SIGKILL。
    let mut child = spawn_child(&db, &endpoint, &marker, "hang").await;
    let info = wait_for(&marker).await;
    let _ = child.kill();
    let _ = child.wait();

    let hits_before = hits.load(Ordering::SeqCst);
    let kernel = Arc::new(SqliteKernel::open(&db_path).await.unwrap());
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
        secrets: Arc::new(TestSecrets),
        owner: "i-restart-a".into(),
        lease_ms: 60_000,
        concurrency: 1,
    });
    let novel_id = info["novelId"].as_str().unwrap().to_string();
    let chapter_id = info["chapterId"].as_str().unwrap().to_string();
    let run_id = info["runId"].as_str().unwrap().to_string();

    // 提交前崩溃：正式正文必须还是空的。
    let chapter = runtime
        .director
        .get_chapter(&novel_id, &chapter_id)
        .await
        .unwrap();
    assert_eq!(chapter.content, "", "提交前崩溃不得写入正式正文");
    assert_eq!(chapter.official_version, 0, "提交前崩溃不得递增正式版本");

    // 门点与候选稿仍然可恢复。
    let detail = runtime
        .director
        .get_run_detail(&novel_id, &run_id)
        .await
        .unwrap();
    assert!(
        detail.status == "waiting_approval" || detail.status == "paused",
        "空闲后的 Run 必须停在可恢复状态：{}",
        detail.status
    );
    let candidate = detail.latest_candidate.clone().expect("候选稿必须还在");
    assert_eq!(candidate.id, info["candidateId"].as_str().unwrap());

    // 用户重新确认同一候选：正常成功并落库。
    let content = info["content"].as_str().unwrap().to_string();
    let confirmed = runtime
        .director
        .confirm_candidate_after_edit(
            "i-confirm-after-crash",
            &run_id,
            None,
            &candidate.id,
            info["sha"].as_str().unwrap(),
            &content,
        )
        .await;
    assert!(confirmed.is_ok(), "崩溃后重新确认应当成功：{confirmed:?}");
    for _ in 0..40 {
        let detail = runtime
            .director
            .get_run_detail(&novel_id, &run_id)
            .await
            .unwrap();
        if detail.status == "succeeded" || detail.status == "failed" {
            break;
        }
        let _ = runtime.worker.tick().await;
    }
    let chapter_after = runtime
        .director
        .get_chapter(&novel_id, &chapter_id)
        .await
        .unwrap();
    assert_eq!(chapter_after.content, content, "重新确认后正文必须落库");
    assert!(chapter_after.official_version > 0);
    assert_eq!(
        hits.load(Ordering::SeqCst),
        hits_before,
        "崩溃恢复不得重新请求模型"
    );
}
