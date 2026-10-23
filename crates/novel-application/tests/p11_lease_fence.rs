use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use novel_application::{
    ModelSecrets, OpenRunInput, PortError, Runtime, RuntimeDeps, TickOutcome, WorkflowRepository,
};
use novel_llm::OpenAiCompatibleProvider;
use novel_storage_sqlite::SqliteKernel;
use novel_testkit::{FakeClock, SequentialIds};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

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

async fn blocked_repair_server() -> (
    String,
    Arc<tokio::sync::Notify>,
    Arc<tokio::sync::Notify>,
    Arc<AtomicUsize>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let repair_started = Arc::new(tokio::sync::Notify::new());
    let release_repair = Arc::new(tokio::sync::Notify::new());
    let repairs = Arc::new(AtomicUsize::new(0));
    let started = repair_started.clone();
    let release = release_repair.clone();
    let count = repairs.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let started = started.clone();
            let release = release.clone();
            let count = count.clone();
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
                let output = if body.contains("STEP:story_macro") {
                    story()
                } else if body.contains("STEP:character_cast") {
                    cast()
                } else if body.contains("STEP:volume_strategy") {
                    volume()
                } else if body.contains("STEP:chapter_list") {
                    chapters()
                } else if body.contains("STEP:consistency_review") {
                    issues()
                } else if body.contains("STEP:chapter_repair") {
                    count.fetch_add(1, Ordering::SeqCst);
                    started.notify_waiters();
                    release.notified().await;
                    "迟到修复候选正文".to_string()
                } else {
                    "原始候选正文".to_string()
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
    (
        format!("http://{address}/v1/chat/completions"),
        repair_started,
        release_repair,
        repairs,
    )
}

struct TestSecrets;
#[async_trait]
impl ModelSecrets for TestSecrets {
    async fn api_key(&self, _provider: &str) -> Result<Option<String>, PortError> {
        Ok(Some("local-test-key".into()))
    }
}

async fn runtime(
    kernel: Arc<SqliteKernel>,
    clock: Arc<FakeClock>,
    owner: &str,
    lease_ms: i64,
) -> Runtime {
    Runtime::new(RuntimeDeps {
        novels: kernel.clone(),
        runs: kernel.clone(),
        worlds: kernel.clone(),
        lookups: kernel.clone(),
        task_center: kernel.clone(),
        llm: Arc::new(OpenAiCompatibleProvider::new()),
        clock,
        ids: Arc::new(SequentialIds::new()),
        faults: Arc::new(novel_application::NoFault),
        secrets: Arc::new(TestSecrets),
        owner: owner.into(),
        lease_ms,
        concurrency: 1,
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn stale_worker_cannot_write_after_lease_takeover() {
    let (endpoint, repair_started, release_repair, repairs) = blocked_repair_server().await;
    let dir = tempfile::tempdir().unwrap();
    let kernel = Arc::new(
        SqliteKernel::open(&dir.path().join("lease-fence.sqlite"))
            .await
            .unwrap(),
    );
    let clock = Arc::new(FakeClock::new(1_000));
    let a = runtime(kernel.clone(), clock.clone(), "worker-a", 50).await;
    let b = runtime(kernel.clone(), clock.clone(), "worker-b", 50).await;
    let novel = a.director.create_novel("租约竞争书", "").await.unwrap();
    let chapter = a
        .director
        .create_chapter(&novel.id, "第一章", 1)
        .await
        .unwrap();
    let mut prompts = std::collections::BTreeMap::new();
    prompts.insert("chapter_draft".into(), "chapter-draft-v2".into());
    let opened = a
        .director
        .open_run(OpenRunInput {
            command_id: "lease-open".into(),
            novel_id: novel.id.clone(),
            target_chapter_id: chapter.id.clone(),
            provider: "mock".into(),
            model: "mock".into(),
            prompt_versions: prompts,
            token_budget: Some(512),
            context_token_budget: Some(24_000),
            max_automatic_retries: 0,
            repair_budget: Some(1),
            endpoint,
            allow_localhost_http: true,
        })
        .await
        .unwrap();
    let run_id = opened.run_id;
    // Drive to the review boundary; the repair tick is the one that blocks.
    let mut at_review = false;
    for _ in 0..30 {
        let artifacts = WorkflowRepository::list_artifacts(kernel.as_ref(), &novel.id)
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
            at_review = true;
            break;
        }
        match a.worker.tick().await.unwrap() {
            TickOutcome::Advanced {
                status,
                run_id: current,
            } if status == "waiting_approval" => {
                let detail = a
                    .director
                    .get_run_detail(&novel.id, &current)
                    .await
                    .unwrap();
                a.director
                    .approve(
                        &format!("lease-approve-{}", detail.control_version),
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
    assert!(at_review);
    let worker_a = a.worker.clone();
    let late = tokio::spawn(async move { worker_a.tick().await });
    tokio::time::timeout(Duration::from_secs(5), repair_started.notified())
        .await
        .expect("repair request must start");
    assert_eq!(repairs.load(Ordering::SeqCst), 1);
    // Deterministic lease expiry: T1.
    clock.advance(100);
    // T2: B legally takes over and must hold the unknown remote result.
    let outcome = b.worker.tick().await.unwrap();
    assert!(
        matches!(outcome, TickOutcome::Advanced { ref status, .. } if status == "paused"),
        "B must pause: {outcome:?}"
    );
    let control = WorkflowRepository::run_control(kernel.as_ref(), &run_id)
        .await
        .unwrap();
    assert_eq!(control.status, novel_domain::RunStatus::Paused);
    let attempts_before: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM workflow_step_attempt WHERE run_id = ?")
            .bind(&run_id)
            .fetch_one(kernel.as_ref().pool())
            .await
            .unwrap();
    // T3: release A's late complete response; A's fenced writes must be rejected.
    release_repair.notify_waiters();
    let late_result = late.await.unwrap();
    assert!(
        late_result.is_err(),
        "stale worker write must fail, got {late_result:?}"
    );
    assert_eq!(
        repairs.load(Ordering::SeqCst),
        1,
        "no unbounded repair retry"
    );
    let candidates: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM chapter_candidate WHERE run_id = ?")
            .bind(&run_id)
            .fetch_one(kernel.as_ref().pool())
            .await
            .unwrap();
    assert_eq!(candidates, 1, "A must not insert a repair candidate");
    let repair_notes: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM workflow_artifact WHERE run_id = ? AND type = 'repair_notes'",
    )
    .bind(&run_id)
    .fetch_one(kernel.as_ref().pool())
    .await
    .unwrap();
    assert_eq!(repair_notes, 0);
    let approvals: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM workflow_artifact WHERE run_id = ? AND type = 'chapter_approval'",
    )
    .bind(&run_id)
    .fetch_one(kernel.as_ref().pool())
    .await
    .unwrap();
    assert_eq!(approvals, 0);
    let commits: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM workflow_artifact WHERE run_id = ? AND type = 'chapter_commit'",
    )
    .bind(&run_id)
    .fetch_one(kernel.as_ref().pool())
    .await
    .unwrap();
    assert_eq!(commits, 0);
    let revisions: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM chapter_revision WHERE chapter_id = ?")
            .bind(&chapter.id)
            .fetch_one(kernel.as_ref().pool())
            .await
            .unwrap();
    assert_eq!(revisions, 0);
    let attempts_after: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM workflow_step_attempt WHERE run_id = ?")
            .bind(&run_id)
            .fetch_one(kernel.as_ref().pool())
            .await
            .unwrap();
    assert_eq!(attempts_after, attempts_before);
    let phase: String = sqlx::query_scalar("SELECT phase FROM workflow_step_attempt WHERE run_id = ? AND step_id = 'bounded_repair' ORDER BY created_at DESC LIMIT 1").bind(&run_id).fetch_one(kernel.as_ref().pool()).await.unwrap();
    assert_eq!(phase, "remote_started");
    let debt: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM chapter_quality_debt WHERE run_id = ?")
            .bind(&run_id)
            .fetch_one(kernel.as_ref().pool())
            .await
            .unwrap();
    assert!(debt > 0);
    println!(
        "P11 lease timeline: A blocked repair=1, clock 1000->1100, B paused, A late write rejected, candidates=1 repair_notes=0 attempts={attempts_after} debt={debt}"
    );
}
