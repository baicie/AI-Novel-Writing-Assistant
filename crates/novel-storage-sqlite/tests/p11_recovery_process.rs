#![cfg(feature = "test-harness")]
//! P11.1-H0: one real cross-process checkpoint after the repair candidate is persisted.
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use novel_application::{OpenRunInput, Runtime, RuntimeDeps, WorkflowRepository};
use novel_llm::UnconfiguredProvider;
use novel_storage_sqlite::SqliteKernel;
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

async fn start_server(hold_repair: bool) -> (String, Arc<AtomicUsize>, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let reviews = Arc::new(AtomicUsize::new(0));
    let repairs = Arc::new(AtomicUsize::new(0));
    let r = reviews.clone();
    let p = repairs.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let reviews = r.clone();
            let repairs = p.clone();
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
                    let call = reviews.fetch_add(1, Ordering::SeqCst);
                    if call == 0 {
                        issues()
                    } else {
                        serde_json::json!({"issues":[]}).to_string()
                    }
                } else if body.contains("STEP:chapter_repair") {
                    repairs.fetch_add(1, Ordering::SeqCst);
                    if hold_repair {
                        let _ = socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\ndata: {\"choices\":[{\"delta\":{\"content\":\"partial\"}}]}\n\n").await;
                        tokio::time::sleep(Duration::from_secs(120)).await;
                        return;
                    }
                    "跨进程修复候选正文".to_string()
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
        reviews,
        repairs,
    )
}

async fn seed(path: &std::path::Path, endpoint: &str) -> (String, String) {
    let kernel = Arc::new(SqliteKernel::open(path).await.unwrap());
    let runtime = Runtime::new(RuntimeDeps {
        novels: kernel.clone(),
        runs: kernel.clone(),
        worlds: kernel.clone(),
        lookups: kernel.clone(),
        task_center: kernel.clone(),
        llm: Arc::new(UnconfiguredProvider),
        clock: Arc::new(novel_application::SystemClock),
        ids: Arc::new(novel_application::UuidIds),
        faults: Arc::new(novel_application::NoFault),
        secrets: Arc::new(novel_application::NoModelSecrets),
        owner: "seed".into(),
        lease_ms: 60_000,
        concurrency: 1,
    });
    let novel = runtime
        .director
        .create_novel("跨进程修复书", "")
        .await
        .unwrap();
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
            command_id: "p11-process-open".into(),
            novel_id: novel.id.clone(),
            target_chapter_id: chapter.id.clone(),
            provider: "mock".into(),
            model: "mock".into(),
            prompt_versions: prompts,
            token_budget: Some(512),
            context_token_budget: Some(24_000),
            max_automatic_retries: 0,
            repair_budget: Some(1),
            endpoint: endpoint.into(),
            allow_localhost_http: true,
        })
        .await
        .unwrap();
    (novel.id, opened.run_id)
}

fn wait_for(path: &std::path::Path, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if path.exists() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    false
}

#[tokio::test(flavor = "multi_thread")]
async fn repair_candidate_checkpoint_survives_real_sigkill_and_does_not_replay() {
    let (endpoint, reviews, repairs) = start_server(false).await;
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("recovery.sqlite");
    let (novel_id, run_id) = seed(&db, &endpoint).await;
    let marker = dir.path().join("checkpoint.marker");
    let binary = std::env::var("CARGO_BIN_EXE_recovery_worker").expect("harness binary env");
    let mut child = Command::new(&binary)
        .env("P11_DB", &db)
        .env("P11_ENDPOINT", &endpoint)
        .env("P11_NOVEL", &novel_id)
        .env("P11_OWNER", "worker-a")
        .env("P11_MARKER", &marker)
        .env("P11_AUTO_APPROVE", "1")
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let pid = child.id();
    assert!(
        wait_for(&marker, Duration::from_secs(30)),
        "checkpoint marker not reached (pid {pid})"
    );
    let kernel = SqliteKernel::open(&db).await.unwrap();
    let candidates: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM chapter_candidate WHERE run_id = ?")
            .bind(&run_id)
            .fetch_one(kernel.pool())
            .await
            .unwrap();
    assert_eq!(
        candidates, 2,
        "original and repair candidates must both be persisted"
    );
    let attempt_phase: String = sqlx::query_scalar("SELECT phase FROM workflow_step_attempt WHERE run_id = ? ORDER BY created_at DESC, rowid DESC LIMIT 1").bind(&run_id).fetch_one(kernel.pool()).await.unwrap();
    assert_eq!(attempt_phase, "candidate_saved");
    let (repaired_content, repaired_hash): (String, String) = sqlx::query_as(
        "SELECT content, content_sha256 FROM chapter_candidate WHERE run_id = ? ORDER BY updated_at DESC, rowid DESC LIMIT 1",
    ).bind(&run_id).fetch_one(kernel.pool()).await.unwrap();
    assert_eq!(repaired_content, "跨进程修复候选正文");
    assert_eq!(
        repaired_hash,
        novel_application::sha256_hex(repaired_content.as_bytes())
    );
    let before_repairs = repairs.load(Ordering::SeqCst);
    let before_reviews = reviews.load(Ordering::SeqCst);
    assert_eq!(before_repairs, 1);

    // True SIGKILL, not a graceful stop/drop.
    child.kill().unwrap();
    let status = child.wait().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(
            status.signal(),
            Some(9),
            "worker must be killed by SIGKILL, got {status:?}"
        );
    }
    println!(
        "P11 child_pid={pid} sigkill_status={status:?} repairs={before_repairs} reviews={before_reviews}"
    );

    // New process: Worker must recover the CandidateSaved attempt without a repair replay.
    let mut recovery = Command::new(&binary)
        .env("P11_DB", &db)
        .env("P11_ENDPOINT", &endpoint)
        .env("P11_NOVEL", &novel_id)
        .env("P11_OWNER", "worker-a")
        .env("P11_AUTO_APPROVE", "0")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let start = Instant::now();
    let recovered = loop {
        if recovery.try_wait().unwrap().is_some() {
            break true;
        }
        if start.elapsed() > Duration::from_secs(30) {
            break false;
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    if !recovered {
        let _ = recovery.kill();
        panic!("recovery worker did not exit after safe resume");
    }
    let recovery_status = recovery.wait().unwrap();
    assert!(
        recovery_status.success(),
        "recovery worker failed: {recovery_status:?}"
    );
    assert_eq!(
        repairs.load(Ordering::SeqCst),
        before_repairs,
        "repair request was replayed after SIGKILL"
    );
    assert_eq!(
        reviews.load(Ordering::SeqCst),
        before_reviews + 1,
        "safe resume must run re-review exactly once"
    );
    let control = kernel.run_control(&run_id).await.unwrap();
    assert_eq!(control.status, novel_domain::RunStatus::WaitingApproval);
    assert_eq!(
        control.gate.as_ref().map(|gate| gate.id.as_str()),
        Some("approval")
    );
    let candidates_after: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM chapter_candidate WHERE run_id = ?")
            .bind(&run_id)
            .fetch_one(kernel.pool())
            .await
            .unwrap();
    assert_eq!(candidates_after, 2);
    let commits: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM workflow_artifact WHERE run_id = ? AND type = 'chapter_commit'",
    )
    .bind(&run_id)
    .fetch_one(kernel.pool())
    .await
    .unwrap();
    assert_eq!(commits, 0);
    let revisions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM chapter_revision WHERE chapter_id = (SELECT chapter_id FROM workflow_run WHERE id = ?)").bind(&run_id).fetch_one(kernel.pool()).await.unwrap();
    assert_eq!(revisions, 0);
    let debt: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM chapter_quality_debt WHERE run_id = ?")
            .bind(&run_id)
            .fetch_one(kernel.pool())
            .await
            .unwrap();
    assert!(debt > 0, "quality debt must survive recovery");
    let approvals: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM workflow_artifact WHERE run_id = ? AND type = 'chapter_approval'",
    )
    .bind(&run_id)
    .fetch_one(kernel.pool())
    .await
    .unwrap();
    assert_eq!(approvals, 1, "approval gate must be created exactly once");
    // Second recovery tick must be idempotent while waiting for the author.
    let mut again = Command::new(&binary)
        .env("P11_DB", &db)
        .env("P11_ENDPOINT", &endpoint)
        .env("P11_NOVEL", &novel_id)
        .env("P11_OWNER", "worker-a")
        .env("P11_AUTO_APPROVE", "0")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let again_status = again.wait().unwrap();
    assert!(again_status.success());
    assert_eq!(repairs.load(Ordering::SeqCst), before_repairs);
    assert_eq!(reviews.load(Ordering::SeqCst), before_reviews + 1);
    let approvals_again: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM workflow_artifact WHERE run_id = ? AND type = 'chapter_approval'",
    )
    .bind(&run_id)
    .fetch_one(kernel.pool())
    .await
    .unwrap();
    assert_eq!(approvals_again, 1);
    let candidate_after_again: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM chapter_candidate WHERE run_id = ?")
            .bind(&run_id)
            .fetch_one(kernel.pool())
            .await
            .unwrap();
    assert_eq!(candidate_after_again, 2);
    println!(
        "P11 recovery status={:?} repairs_after={} reviews_after={} candidates={} commits={} revisions={} debt={}",
        control.status,
        repairs.load(Ordering::SeqCst),
        reviews.load(Ordering::SeqCst),
        candidates_after,
        commits,
        revisions,
        debt
    );
    let _ = child.id();
}

#[tokio::test(flavor = "multi_thread")]
async fn remote_started_repair_is_not_replayed_and_stays_unknown() {
    let (endpoint, reviews, repairs) = start_server(true).await;
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("remote-started.sqlite");
    let (novel_id, run_id) = seed(&db, &endpoint).await;
    let binary = std::env::var("CARGO_BIN_EXE_recovery_worker").expect("harness binary env");
    let mut child = Command::new(&binary)
        .env("P11_DB", &db)
        .env("P11_ENDPOINT", &endpoint)
        .env("P11_NOVEL", &novel_id)
        .env("P11_OWNER", "worker-a")
        .env("P11_AUTO_APPROVE", "1")
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let pid = child.id();
    let start = Instant::now();
    while repairs.load(Ordering::SeqCst) == 0 {
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "repair request did not start (pid {pid})"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
    let kernel = SqliteKernel::open(&db).await.unwrap();
    let attempt_phase: String = sqlx::query_scalar("SELECT phase FROM workflow_step_attempt WHERE run_id = ? ORDER BY created_at DESC, rowid DESC LIMIT 1").bind(&run_id).fetch_one(kernel.pool()).await.unwrap();
    assert_eq!(attempt_phase, "remote_started");
    let candidates_before: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM chapter_candidate WHERE run_id = ?")
            .bind(&run_id)
            .fetch_one(kernel.pool())
            .await
            .unwrap();
    assert_eq!(candidates_before, 1);
    let request_id: Option<String> = sqlx::query_scalar("SELECT request_id FROM workflow_step_attempt WHERE run_id = ? ORDER BY created_at DESC, rowid DESC LIMIT 1").bind(&run_id).fetch_one(kernel.pool()).await.unwrap();
    assert!(request_id.is_some());
    let before_repairs = repairs.load(Ordering::SeqCst);
    let before_reviews = reviews.load(Ordering::SeqCst);

    child.kill().unwrap();
    let status = child.wait().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(status.signal(), Some(9));
    }
    println!("P11B child_pid={pid} sigkill_status={status:?} repair_requests={before_repairs}");

    let mut recovery = Command::new(&binary)
        .env("P11_DB", &db)
        .env("P11_ENDPOINT", &endpoint)
        .env("P11_NOVEL", &novel_id)
        .env("P11_OWNER", "worker-a")
        .env("P11_AUTO_APPROVE", "0")
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let exit = recovery.wait().unwrap();
    assert!(exit.success(), "recovery worker failed: {exit:?}");
    assert_eq!(
        repairs.load(Ordering::SeqCst),
        before_repairs,
        "unknown remote repair was replayed"
    );
    assert_eq!(
        reviews.load(Ordering::SeqCst),
        before_reviews,
        "unknown remote repair entered re-review"
    );
    let control = kernel.run_control(&run_id).await.unwrap();
    assert_eq!(control.status, novel_domain::RunStatus::Paused);
    let candidates_after: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM chapter_candidate WHERE run_id = ?")
            .bind(&run_id)
            .fetch_one(kernel.pool())
            .await
            .unwrap();
    assert_eq!(candidates_after, 1);
    let invocations: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM llm_invocation WHERE run_id = ? AND step_id = 'bounded_repair'",
    )
    .bind(&run_id)
    .fetch_one(kernel.pool())
    .await
    .unwrap();
    assert_eq!(
        invocations, 0,
        "unknown usage must not be recorded as a fake zero"
    );
    let commits: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM workflow_artifact WHERE run_id = ? AND type = 'chapter_commit'",
    )
    .bind(&run_id)
    .fetch_one(kernel.pool())
    .await
    .unwrap();
    assert_eq!(commits, 0);
    let revisions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM chapter_revision WHERE chapter_id = (SELECT chapter_id FROM workflow_run WHERE id = ?)").bind(&run_id).fetch_one(kernel.pool()).await.unwrap();
    assert_eq!(revisions, 0);
    let debt: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM chapter_quality_debt WHERE run_id = ?")
            .bind(&run_id)
            .fetch_one(kernel.pool())
            .await
            .unwrap();
    assert!(debt > 0);
    println!(
        "P11B recovery status={:?} repairs_after={} reviews_after={} candidates={} invocations={} commits={} revisions={} debt={}",
        control.status,
        repairs.load(Ordering::SeqCst),
        reviews.load(Ordering::SeqCst),
        candidates_after,
        invocations,
        commits,
        revisions,
        debt
    );
}
