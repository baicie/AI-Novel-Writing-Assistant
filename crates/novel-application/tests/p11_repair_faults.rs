use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use novel_application::{
    ModelSecrets, OpenRunInput, PortError, Runtime, RuntimeDeps, TickOutcome, WorkflowRepository,
};
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

fn story() -> String {
    serde_json::json!({"expansion":{"expanded_premise":"前提","protagonist_core":"核心","conflict_engine":"冲突","conflict_layers":{"external":"外"},"mystery_box":"未知","emotional_line":"成长","setpiece_seeds":["场面"],"tone_reference":"冷峻"},"decomposition":{"selling_point":"卖点","core_conflict":"冲突","main_hook":"钩子","progression_loop":"循环","growth_path":"成长","major_payoffs":["回报"],"ending_flavor":"结局"},"constraints":["规则"],"issues":[]}).to_string()
}
fn cast() -> String {
    serde_json::json!({"members":[{"name":"林动","role":"主角","castRole":"protagonist","storyFunction":"作用"},{"name":"苏晚","role":"同伴","castRole":"ally","storyFunction":"作用"},{"name":"陆沉","role":"对手","castRole":"antagonist","storyFunction":"作用"}],"relations":[{"sourceName":"林动","targetName":"苏晚","surfaceRelation":"同门"},{"sourceName":"林动","targetName":"陆沉","surfaceRelation":"敌对"}]}).to_string()
}
fn volume() -> String {
    serde_json::json!({"volumes":[{"volumeOrder":1,"title":"卷一","roleLabel":"开局","coreReward":"进入","planningMode":"hard"}]}).to_string()
}
fn chapters() -> String {
    serde_json::json!({"chapters":[{"chapterOrder":1,"title":"第一章","volumeTitle":"卷一","summary":"入山","characterNames":["林动"]}]}).to_string()
}
fn issues() -> String {
    serde_json::json!({"issues":[{"type":"timeline","severity":"high","evidence":"时间线冲突","suggestion":"修正时间"}]}).to_string()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RepairMode {
    Success,
    Eof,
    Cancel,
}

struct ServerState {
    repair_mode: RepairMode,
    repair_started: tokio::sync::Notify,
    release_repair: tokio::sync::Notify,
    reviews: AtomicUsize,
    bodies: Mutex<Vec<String>>,
}

async fn start_server(mode: RepairMode) -> (std::net::SocketAddr, Arc<ServerState>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let state = Arc::new(ServerState {
        repair_mode: mode,
        repair_started: tokio::sync::Notify::new(),
        release_repair: tokio::sync::Notify::new(),
        reviews: AtomicUsize::new(0),
        bodies: Mutex::new(Vec::new()),
    });
    let shared = state.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let state = shared.clone();
            tokio::spawn(async move {
                let mut data = Vec::new();
                let mut buf = [0u8; 32 * 1024];
                loop {
                    let n = socket.read(&mut buf).await.unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    data.extend_from_slice(&buf[..n]);
                    let text = String::from_utf8_lossy(&data);
                    if let Some(header_end) = text.find("\r\n\r\n") {
                        let len = text[..header_end]
                            .lines()
                            .find_map(|line| {
                                line.strip_prefix("Content-Length: ")
                                    .and_then(|v| v.trim().parse::<usize>().ok())
                            })
                            .unwrap_or(0);
                        if data.len() >= header_end + 4 + len {
                            break;
                        }
                    }
                }
                let body = String::from_utf8_lossy(&data).to_string();
                state.bodies.lock().unwrap().push(body.clone());
                let output = if body.contains("STEP:story_macro") {
                    story()
                } else if body.contains("STEP:character_cast") {
                    cast()
                } else if body.contains("STEP:volume_strategy") {
                    volume()
                } else if body.contains("STEP:chapter_list") {
                    chapters()
                } else if body.contains("STEP:consistency_review") {
                    let call = state.reviews.fetch_add(1, Ordering::SeqCst);
                    if call == 0 {
                        issues()
                    } else {
                        serde_json::json!({"issues":[]}).to_string()
                    }
                } else if body.contains("STEP:chapter_repair") {
                    state.repair_started.notify_waiters();
                    match state.repair_mode {
                        RepairMode::Success => "修复后的候选正文".to_string(),
                        RepairMode::Eof => {
                            let response = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\ndata: {\"choices\":[{\"delta\":{\"content\":\"半\"}}]}\n\n";
                            let _ = socket.write_all(response.as_bytes()).await;
                            return;
                        }
                        RepairMode::Cancel => {
                            let response = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\ndata: {\"choices\":[{\"delta\":{\"content\":\"迟到\"}}]}\n\n";
                            let _ = socket.write_all(response.as_bytes()).await;
                            state.release_repair.notified().await;
                            return;
                        }
                    }
                } else {
                    "候选正文".to_string()
                };
                let event =
                    serde_json::json!({"choices":[{"delta":{"content":output}}]}).to_string();
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
    (address, state)
}

async fn runtime_for(
    address: std::net::SocketAddr,
    manager: Arc<dyn novel_llm::LlmProvider>,
) -> (Arc<SqliteKernel>, Runtime, String, String) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.keep().join("fault.sqlite");
    let kernel = Arc::new(SqliteKernel::open(&path).await.unwrap());
    let runtime = Runtime::new(RuntimeDeps {
        novels: kernel.clone(),
        runs: kernel.clone(),
        worlds: kernel.clone(),
        lookups: kernel.clone(),
        task_center: kernel.clone(),
        llm: manager,
        clock: Arc::new(FakeClock::new(1_000)),
        ids: Arc::new(SequentialIds::new()),
        faults: Arc::new(novel_application::NoFault),
        secrets: Arc::new(TestSecrets),
        owner: "p11-worker".into(),
        lease_ms: 60_000,
        concurrency: 1,
    });
    let novel = runtime.director.create_novel("故障书", "").await.unwrap();
    let chapter = runtime
        .director
        .create_chapter(&novel.id, "第一章", 1)
        .await
        .unwrap();
    let mut prompts = std::collections::BTreeMap::new();
    prompts.insert("chapter_draft".into(), "chapter-draft-v2".into());
    let opened = runtime
        .director
        .open_run(OpenRunInput {
            command_id: "p11-open".into(),
            novel_id: novel.id.clone(),
            target_chapter_id: chapter.id.clone(),
            provider: "mock".into(),
            credential_ref: Some("mock".into()),
            model: "mock".into(),
            prompt_versions: prompts,
            token_budget: Some(512),
            context_token_budget: Some(24_000),
            max_automatic_retries: 0,
            repair_budget: Some(1),
            endpoint: format!("http://{address}/v1/chat/completions"),
            allow_localhost_http: true,
            step_routes: Default::default(),
        })
        .await
        .unwrap();
    (kernel, runtime, novel.id, opened.run_id)
}

#[tokio::test]
async fn repair_sse_eof_pauses_without_candidate_or_commit() {
    let (address, state) = start_server(RepairMode::Eof).await;
    let llm: Arc<dyn novel_llm::LlmProvider> = Arc::new(OpenAiCompatibleProvider::new());
    let (kernel, runtime, novel_id, run_id) = runtime_for(address, llm).await;
    // Drive until the first chapter gate, then let the worker reach repair naturally.
    for _ in 0..40 {
        match runtime.worker.tick().await.unwrap() {
            TickOutcome::Advanced {
                status,
                run_id: current,
            } if status == "waiting_approval" => {
                let detail = runtime
                    .director
                    .get_run_detail(&novel_id, &current)
                    .await
                    .unwrap();
                runtime
                    .director
                    .approve(
                        &format!("p11-fault-approve-{}", detail.control_version),
                        &current,
                    )
                    .await
                    .unwrap();
            }
            TickOutcome::Advanced { status, .. } if status == "paused" => break,
            TickOutcome::Idle => break,
            _ => {}
        }
    }
    assert!(
        state
            .bodies
            .lock()
            .unwrap()
            .iter()
            .any(|body| body.contains("STEP:chapter_repair"))
    );
    let control = WorkflowRepository::run_control(kernel.as_ref(), &run_id)
        .await
        .unwrap();
    assert_eq!(control.status, novel_domain::RunStatus::Paused);
    let artifacts = WorkflowRepository::list_artifacts(kernel.as_ref(), &novel_id)
        .await
        .unwrap();
    assert!(
        !artifacts
            .iter()
            .any(|artifact| artifact.artifact_type == "repair_notes")
    );
    assert!(
        !artifacts
            .iter()
            .any(|artifact| artifact.artifact_type == "chapter_commit")
    );
    let candidates: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM chapter_candidate WHERE run_id = ?")
            .bind(&run_id)
            .fetch_one(kernel.as_ref().pool())
            .await
            .unwrap();
    assert_eq!(
        candidates, 1,
        "only the original draft candidate should exist"
    );
}

#[tokio::test]
async fn repair_success_via_real_http_records_second_review_and_commit() {
    let (address, state) = start_server(RepairMode::Success).await;
    let llm: Arc<dyn novel_llm::LlmProvider> = Arc::new(OpenAiCompatibleProvider::new());
    let (kernel, runtime, novel_id, run_id) = runtime_for(address, llm).await;
    let mut completed = false;
    for _ in 0..50 {
        match runtime.worker.tick().await.unwrap() {
            TickOutcome::Advanced {
                status,
                run_id: current,
            } if status == "waiting_approval" => {
                let detail = runtime
                    .director
                    .get_run_detail(&novel_id, &current)
                    .await
                    .unwrap();
                runtime
                    .director
                    .approve(
                        &format!("p11-http-approve-{}", detail.control_version),
                        &current,
                    )
                    .await
                    .unwrap();
            }
            TickOutcome::Advanced { status, .. } if status == "succeeded" => {
                completed = true;
                break;
            }
            TickOutcome::Advanced { .. } => {}
            TickOutcome::Idle => break,
            _ => {}
        }
    }
    assert!(completed);
    assert!(state.reviews.load(Ordering::SeqCst) >= 2);
    assert!(
        state
            .bodies
            .lock()
            .unwrap()
            .iter()
            .any(|body| body.contains("STEP:chapter_repair"))
    );
    let chapter: String = sqlx::query_scalar("SELECT r.content FROM chapter c JOIN chapter_revision r ON r.id=c.official_revision_id WHERE c.id=(SELECT chapter_id FROM workflow_run WHERE id=?)").bind(&run_id).fetch_one(kernel.as_ref().pool()).await.unwrap_or_default();
    assert!(chapter.contains("修复后的候选正文"));
}

#[tokio::test]
async fn cancel_during_repair_sse_does_not_commit_late_tokens() {
    let (address, state) = start_server(RepairMode::Cancel).await;
    let llm: Arc<dyn novel_llm::LlmProvider> = Arc::new(OpenAiCompatibleProvider::new());
    let (kernel, runtime, novel_id, run_id) = runtime_for(address, llm).await;
    // Drive through gates and draft/review, stopping before the repair tick.
    let mut ready = false;
    for _ in 0..30 {
        let artifacts = WorkflowRepository::list_artifacts(kernel.as_ref(), &novel_id)
            .await
            .unwrap();
        let control = WorkflowRepository::run_control(kernel.as_ref(), &run_id)
            .await
            .unwrap();
        if control.status == novel_domain::RunStatus::Running
            && artifacts
                .iter()
                .any(|artifact| artifact.artifact_type == "consistency_review")
        {
            ready = true;
            break;
        }
        match runtime.worker.tick().await.unwrap() {
            TickOutcome::Advanced {
                status,
                run_id: current,
            } if status == "waiting_approval" => {
                let detail = runtime
                    .director
                    .get_run_detail(&novel_id, &current)
                    .await
                    .unwrap();
                runtime
                    .director
                    .approve(
                        &format!("p11-cancel-approve-{}", detail.control_version),
                        &current,
                    )
                    .await
                    .unwrap();
            }
            TickOutcome::Advanced { status, .. } if status == "paused" => break,
            TickOutcome::Idle => break,
            _ => {}
        }
    }
    assert!(ready, "review must complete before cancelling repair");
    let worker = runtime.worker.clone();
    let tick = tokio::spawn(async move { worker.tick().await });
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        state.repair_started.notified(),
    )
    .await
    .expect("repair request must start");
    runtime.worker.cancel_generation(&run_id);
    state.release_repair.notify_waiters();
    let _ = tick.await;
    let control = WorkflowRepository::run_control(kernel.as_ref(), &run_id)
        .await
        .unwrap();
    assert_eq!(control.status, novel_domain::RunStatus::Cancelled);
    let artifacts = WorkflowRepository::list_artifacts(kernel.as_ref(), &novel_id)
        .await
        .unwrap();
    assert!(
        !artifacts
            .iter()
            .any(|artifact| artifact.artifact_type == "chapter_commit")
    );
    let revision: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM chapter_revision WHERE chapter_id=(SELECT chapter_id FROM workflow_run WHERE id=?)").bind(&run_id).fetch_one(kernel.as_ref().pool()).await.unwrap();
    assert_eq!(revision, 0);
}
