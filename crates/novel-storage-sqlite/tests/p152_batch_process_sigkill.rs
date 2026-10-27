#![cfg(feature = "test-harness")]
use std::collections::BTreeMap;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use novel_application::{
    BatchRepository, BatchRunRequest, BatchService, Clock, NewChapter, NewNovel, NovelRepository,
    Runtime, RuntimeDeps, SystemClock, UuidIds,
};
use novel_llm::UnconfiguredProvider;
use novel_storage_sqlite::SqliteKernel;

fn runtime(kernel: Arc<SqliteKernel>) -> Runtime {
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
async fn real_process_sigkill_between_open_run_and_item_binding_recovers() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("batch.sqlite");
    let barrier = dir.path().join("barrier");
    std::fs::create_dir_all(&barrier).unwrap();
    let kernel = Arc::new(SqliteKernel::open(&db).await.unwrap());
    NovelRepository::create_novel(
        kernel.as_ref(),
        NewNovel {
            id: "n1".into(),
            title: "进程崩溃".into(),
            description: String::new(),
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    for (id, order) in [("c1", 1), ("c2", 2)] {
        NovelRepository::create_chapter(
            kernel.as_ref(),
            NewChapter {
                id: id.into(),
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
    let mut prompts = BTreeMap::new();
    prompts.insert("chapter_draft".into(), "chapter-draft-v1".into());
    let batch = BatchService::new(kernel.clone(), Arc::new(UuidIds))
        .create(
            "n1",
            &["c1".into(), "c2".into()],
            BatchRunRequest {
                provider: "mock".into(),
                model: "mock".into(),
                prompt_versions: prompts,
                token_budget: Some(512),
                context_token_budget: None,
                max_automatic_retries: 0,
                repair_budget: Some(0),
                endpoint: "https://api.example.com/v1/chat/completions".into(),
                allow_localhost_http: false,
            },
            "cmd-create",
            10,
        )
        .await
        .unwrap();
    kernel.pool().close().await;

    let mut child = Command::new(env!("CARGO_BIN_EXE_batch_crash_worker"))
        .arg(&db)
        .arg(&batch.id)
        .env("BIZ_NOVEL_TEST_BATCH_BARRIER_DIR", &barrier)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline && !std::fs::read_dir(&barrier).unwrap().next().is_some() {
        std::thread::sleep(Duration::from_millis(50));
    }
    let marker = std::fs::read_dir(&barrier)
        .unwrap()
        .next()
        .expect("barrier marker")
        .unwrap()
        .path();
    let payload: serde_json::Value =
        serde_json::from_slice(&std::fs::read(marker).unwrap()).unwrap();
    let run_id = payload["runId"].as_str().unwrap().to_string();
    let command_id = payload["commandId"].as_str().unwrap().to_string();
    child.kill().unwrap();
    let _ = child.wait();
    assert!(!child.id().to_string().is_empty());

    let reopened = Arc::new(SqliteKernel::open(&db).await.unwrap());
    // The killed worker's process-local lease is still in the future; simulate lease expiry.
    sqlx::query("UPDATE batch_run SET lease_until = 0 WHERE id = ?").bind(&batch.id).execute(reopened.pool()).await.unwrap();
    let before = BatchRepository::get_batch(reopened.as_ref(), &batch.id)
        .await
        .unwrap();
    assert!(
        before.items[0].run_id.is_none(),
        "binding must not have committed before SIGKILL"
    );
    let runtime = runtime(reopened.clone());
    BatchService::new(reopened.clone(), Arc::new(UuidIds))
        .tick(
            &runtime.director,
            "scheduler-b",
            SystemClock.now_ms() + 120_000,
        )
        .await
        .unwrap();
    let after = BatchRepository::get_batch(reopened.as_ref(), &batch.id)
        .await
        .unwrap();
    assert_eq!(after.items[0].run_id.as_deref(), Some(run_id.as_str()));
    assert_eq!(after.items[0].run_id.as_deref(), Some(run_id.as_str()));
    let runs = runtime.director.list_runs("n1").await.unwrap();
    assert_eq!(runs.len(), 1, "recovery must reuse the existing Run");
    assert_eq!(runs[0].id, run_id);
    assert_eq!(
        format!("batch:{}:item:{}:open", batch.id, after.items[0].id),
        command_id
    );
}
