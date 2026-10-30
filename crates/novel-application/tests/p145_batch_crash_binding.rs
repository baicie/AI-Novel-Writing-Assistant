use std::collections::BTreeMap;
use std::sync::Arc;

use novel_application::{
    BatchRepository, BatchRunRequest, BatchService, NewChapter, NewNovel, OpenRunInput, Runtime,
    RuntimeDeps, UuidIds,
};
use novel_storage_sqlite::SqliteKernel;
use novel_testkit::{FakeClock, ScriptedLlm};

fn prompts() -> BTreeMap<String, String> {
    let mut prompts = BTreeMap::new();
    prompts.insert("chapter_draft".into(), "chapter-draft-v1".into());
    prompts
}

fn build_runtime(kernel: Arc<SqliteKernel>) -> Runtime {
    Runtime::new(RuntimeDeps {
        novels: kernel.clone(),
        runs: kernel.clone(),
        worlds: kernel.clone(),
        lookups: kernel.clone(),
        task_center: kernel.clone(),
        llm: Arc::new(ScriptedLlm::new(vec![])),
        clock: Arc::new(FakeClock::new(1_000)),
        ids: Arc::new(UuidIds),
        faults: Arc::new(novel_application::NoFault),
        secrets: Arc::new(novel_application::NoModelSecrets),
        owner: "batch-crash-worker".into(),
        lease_ms: 60_000,
        concurrency: 1,
    })
}

#[tokio::test]
async fn committed_open_run_before_binding_is_replayed_after_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("batch-crash.sqlite");
    let kernel = Arc::new(SqliteKernel::open(&path).await.unwrap());
    let runtime = build_runtime(kernel.clone());
    novel_application::NovelRepository::create_novel(
        kernel.as_ref(),
        NewNovel {
            id: "n1".into(),
            title: "崩溃绑定".into(),
            description: String::new(),
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    for (index, id) in ["c1", "c2"].iter().enumerate() {
        novel_application::NovelRepository::create_chapter(
            kernel.as_ref(),
            NewChapter {
                id: id.to_string(),
                novel_id: "n1".into(),
                title: format!("第{}章", index + 1),
                sort_order: index as i32 + 1,
                volume_id: None,
                now_ms: 1,
            },
        )
        .await
        .unwrap();
    }
    let batch = BatchService::new(kernel.clone(), Arc::new(UuidIds))
        .create(
            "n1",
            &["c1".into(), "c2".into()],
            BatchRunRequest {
                provider: "mock".into(),
                model: "mock".into(),
                prompt_versions: prompts(),
                token_budget: Some(512),
                context_token_budget: None,
                max_automatic_retries: 0,
                repair_budget: Some(0),
                endpoint: "https://api.example.com/v1/chat/completions".into(),
                allow_localhost_http: false,
            },
            "create-crash",
            10,
        )
        .await
        .unwrap();
    let claimed = kernel
        .claim_batch(&batch.id, "scheduler-a", 100_000, 20)
        .await
        .unwrap()
        .unwrap();
    let item = &claimed.items[0];
    let outcome = runtime
        .director
        .open_run(OpenRunInput {
            command_id: format!("batch:{}:item:{}:open", batch.id, item.id),
            novel_id: "n1".into(),
            target_chapter_id: "c1".into(),
            provider: "mock".into(),
            model: "mock".into(),
            prompt_versions: prompts(),
            token_budget: Some(512),
            context_token_budget: None,
            max_automatic_retries: 0,
            repair_budget: Some(0),
            endpoint: "https://api.example.com/v1/chat/completions".into(),
            allow_localhost_http: false,
        })
        .await
        .unwrap();
    // Simulate the crash exactly between open_run commit and Batch Item binding.
    drop(runtime);
    drop(kernel);

    let reopened = Arc::new(SqliteKernel::open(&path).await.unwrap());
    let after = BatchRepository::get_batch(reopened.as_ref(), &batch.id)
        .await
        .unwrap();
    assert!(
        after.items[0].run_id.is_none(),
        "precondition: binding must not exist"
    );
    let runtime2 = build_runtime(reopened.clone());
    let service = BatchService::new(reopened.clone(), Arc::new(UuidIds));
    service
        .tick(&runtime2.director, "scheduler-b", 200_000)
        .await
        .unwrap();
    let repaired = BatchRepository::get_batch(reopened.as_ref(), &batch.id)
        .await
        .unwrap();
    assert_eq!(
        repaired.items[0].run_id.as_deref(),
        Some(outcome.run_id.as_str())
    );
    assert_eq!(repaired.status, "running");
    let runs = runtime2.director.list_runs("n1").await.unwrap();
    assert_eq!(runs.len(), 1, "recovery must not open a second Run");
    assert_eq!(runs[0].id, outcome.run_id);
}
