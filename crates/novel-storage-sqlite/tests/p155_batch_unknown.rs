#![cfg(feature = "test-harness")]
use std::collections::BTreeMap;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use novel_application::{
    BatchRepository, BatchRunRequest, BatchService, Clock, NewChapter, NewNovel, NovelRepository,
    OpenRunInput, Runtime, RuntimeDeps, SystemClock, UuidIds, WorkflowRepository,
};
use novel_llm::UnconfiguredProvider;
use novel_storage_sqlite::SqliteKernel;
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;

fn make_runtime(kernel: Arc<SqliteKernel>) -> Runtime {
    Runtime::new(RuntimeDeps {
        novels: kernel.clone(),
        runs: kernel.clone(),
        worlds: kernel.clone(),
        lookups: kernel.clone(),
        task_center: kernel,
        llm: Arc::new(UnconfiguredProvider),
        clock: Arc::new(SystemClock),
        ids: Arc::new(UuidIds),
        faults: Arc::new(novel_application::NoFault),
        secrets: Arc::new(novel_application::NoModelSecrets),
        owner: "parent".into(),
        lease_ms: 60_000,
        concurrency: 1,
    })
}

#[tokio::test]
async fn unknown_remote_is_not_resent_after_worker_sigkill() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("unknown.sqlite");
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!(
        "http://{}/v1/chat/completions",
        listener.local_addr().unwrap()
    );
    let received = Arc::new(AtomicUsize::new(0));
    let count = received.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            count.fetch_add(1, Ordering::SeqCst);
            tokio::spawn(async move {
                let mut buffer = vec![0u8; 16 * 1024];
                let _ = socket.read(&mut buffer).await;
                // Deliberately never send a response; the worker hangs in the HTTP call.
                tokio::time::sleep(Duration::from_secs(90)).await;
            });
        }
    });

    let kernel = Arc::new(SqliteKernel::open(&db).await.unwrap());
    NovelRepository::create_novel(
        kernel.as_ref(),
        NewNovel {
            id: "n1".into(),
            title: "远端未知".into(),
            description: String::new(),
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    for order in 1..=3 {
        NovelRepository::create_chapter(
            kernel.as_ref(),
            NewChapter {
                id: format!("c{order}"),
                novel_id: "n1".into(),
                title: format!("第{order}章"),
                sort_order: order,
                volume_id: None,
                now_ms: 1,
            },
        )
        .await
        .unwrap();
    }
    let first = kernel.get_chapter("n1", "c1").await.unwrap();
    NovelRepository::save_user_chapter(
        kernel.as_ref(),
        novel_application::SaveChapterRecord {
            novel_id: "n1".into(),
            chapter_id: "c1".into(),
            expected_version: first.official_version,
            title: "第1章".into(),
            content: "第一章正式正文".into(),
            revision_id: "rev-c1".into(),
            artifact_id: "artifact-c1".into(),
            content_sha256: "hash-c1".into(),
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    let mut prompts = BTreeMap::new();
    prompts.insert("chapter_draft".into(), "chapter-draft-v1".into());
    let batch = BatchService::new(kernel.clone(), Arc::new(UuidIds))
        .create(
            "n1",
            &["c1".into(), "c2".into(), "c3".into()],
            BatchRunRequest {
                provider: "mock".into(),
                model: "mock".into(),
                prompt_versions: prompts.clone(),
                token_budget: Some(512),
                context_token_budget: None,
                max_automatic_retries: 0,
                repair_budget: Some(0),
                endpoint: endpoint.clone(),
                allow_localhost_http: true,
            },
            "cmd-create-unknown",
            10,
        )
        .await
        .unwrap();
    sqlx::query("UPDATE batch_run_item SET status='succeeded' WHERE batch_id=? AND ordinal=0")
        .bind(&batch.id)
        .execute(kernel.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE batch_run SET cursor=1, status='running' WHERE id=?")
        .bind(&batch.id)
        .execute(kernel.pool())
        .await
        .unwrap();
    let claimed = kernel
        .claim_batch(
            &batch.id,
            "setup",
            SystemClock.now_ms() + 60_000,
            SystemClock.now_ms(),
        )
        .await
        .unwrap()
        .unwrap();
    let item = &claimed.items[1];
    let runtime = make_runtime(kernel.clone());
    let outcome = runtime
        .director
        .open_run(OpenRunInput {
            command_id: format!("batch:{}:item:{}:open", batch.id, item.id),
            novel_id: "n1".into(),
            target_chapter_id: "c2".into(),
            provider: "mock".into(),
            model: "mock".into(),
            prompt_versions: prompts,
            token_budget: Some(512),
            context_token_budget: None,
            max_automatic_retries: 0,
            repair_budget: Some(0),
            endpoint: endpoint.clone(),
            allow_localhost_http: true,
            step_routes: Default::default(),
        })
        .await
        .unwrap();
    kernel
        .bind_item_run(
            &novel_application::BatchFence {
                owner: "setup".into(),
                claim_epoch: claimed.claim_epoch,
                version: claimed.version,
            },
            &item.id,
            &outcome.run_id,
            SystemClock.now_ms(),
        )
        .await
        .unwrap();
    let run_id = outcome.run_id.clone();

    let err_path = dir.path().join("worker.err");
    let err_file = std::fs::File::create(&err_path).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_unknown_worker"))
        .arg(&db)
        .stdout(Stdio::null())
        .stderr(Stdio::from(err_file))
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut remote_started = false;
    while Instant::now() < deadline {
        remote_started = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM workflow_step_attempt WHERE run_id=? AND phase='remote_started'",
        )
        .bind(&run_id)
        .fetch_one(kernel.pool())
        .await
        .unwrap_or(0)
            > 0;
        if remote_started && received.load(Ordering::SeqCst) >= 1 {
            break;
        }
        if let Some(status) = child.try_wait().unwrap() {
            let err = std::fs::read_to_string(&err_path).unwrap_or_default();
            panic!("worker exited early: {status}; stderr: {err}");
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    if !remote_started {
        let err = std::fs::read_to_string(&err_path).unwrap_or_default();
        panic!("RemoteStarted must be persisted before kill; worker stderr: {err}");
    }
    assert!(
        received.load(Ordering::SeqCst) >= 1,
        "provider must receive the request"
    );
    let requests_before = received.load(Ordering::SeqCst);
    assert_eq!(
        requests_before, 1,
        "provider must receive exactly one request"
    );
    child.kill().unwrap();
    let _ = child.wait();
    tokio::time::sleep(Duration::from_millis(300)).await;

    let reopened = Arc::new(SqliteKernel::open(&db).await.unwrap());
    let now = SystemClock.now_ms();
    sqlx::query("UPDATE workflow_control SET lease_owner=NULL, lease_until=0 WHERE run_id=?")
        .bind(&run_id)
        .execute(reopened.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE batch_run SET lease_owner=NULL, lease_until=0 WHERE id=?")
        .bind(&batch.id)
        .execute(reopened.pool())
        .await
        .unwrap();
    let runtime2 = make_runtime(reopened.clone());
    let _ = runtime2.worker.tick().await;
    let _ = BatchService::new(reopened.clone(), Arc::new(UuidIds))
        .tick(&runtime2.director, "recovery-scheduler", now + 60_000)
        .await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        received.load(Ordering::SeqCst),
        requests_before,
        "unknown remote must not be resent"
    );
    let control = WorkflowRepository::run_control(reopened.as_ref(), &run_id)
        .await
        .unwrap();
    assert_eq!(control.status, novel_domain::RunStatus::Paused);
    let attempts: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM workflow_step_attempt WHERE run_id=?")
            .bind(&run_id)
            .fetch_one(reopened.pool())
            .await
            .unwrap();
    assert!(attempts >= 1);
    let candidates: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM chapter_candidate WHERE chapter_id='c2'")
            .fetch_one(reopened.pool())
            .await
            .unwrap();
    assert_eq!(candidates, 0);
    let after = BatchRepository::get_batch(reopened.as_ref(), &batch.id)
        .await
        .unwrap();
    assert!(after.items[2].run_id.is_none(), "chapter 3 must not start");
    assert_eq!(after.items[1].run_id.as_deref(), Some(run_id.as_str()));
}
