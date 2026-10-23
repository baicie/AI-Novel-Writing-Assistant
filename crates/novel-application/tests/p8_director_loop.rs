use std::sync::Arc;

use novel_application::{NovelRepository, OpenRunInput, Runtime, RuntimeDeps, TickOutcome};
use novel_storage_sqlite::SqliteKernel;
use novel_testkit::{FakeClock, MockLlm, SequentialIds};
use sqlx::Row;

fn prompts() -> std::collections::BTreeMap<String, String> {
    let mut prompts = std::collections::BTreeMap::new();
    prompts.insert("chapter_draft".into(), "chapter-draft-v1".into());
    prompts
}

#[tokio::test]
async fn mock_director_loop_reaches_committed_chapter_and_survives_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("novel.sqlite");
    let kernel = Arc::new(SqliteKernel::open(&path).await.unwrap());
    let llm = Arc::new(MockLlm::new("导演流程生成的候选正文"));
    let runtime = Runtime::new(RuntimeDeps {
        novels: kernel.clone(),
        runs: kernel.clone(),
        worlds: kernel.clone(),
        lookups: kernel.clone(),
        task_center: kernel.clone(),
        llm,
        clock: Arc::new(FakeClock::new(1_000)),
        ids: Arc::new(SequentialIds::new()),
        faults: Arc::new(novel_application::NoFault),
        secrets: Arc::new(novel_application::NoModelSecrets),
        owner: "p8-worker".into(),
        lease_ms: 60_000,
        concurrency: 1,
    });
    let novel = runtime.director.create_novel("导演闭环", "").await.unwrap();
    let chapter = runtime
        .director
        .create_chapter(&novel.id, "第一章", 1)
        .await
        .unwrap();
    let opened = runtime
        .director
        .open_run(OpenRunInput {
            command_id: "p8-open".into(),
            novel_id: novel.id.clone(),
            target_chapter_id: chapter.id.clone(),
            provider: "mock".into(),
            model: "mock-model".into(),
            prompt_versions: prompts(),
            token_budget: Some(512),
            max_automatic_retries: 0,
            endpoint: "https://api.example.com/v1/chat/completions".into(),
            allow_localhost_http: false,
        })
        .await
        .unwrap();
    assert_eq!(opened.status, "queued");

    let mut approvals = 0;
    let mut completed = false;
    for tick in 0..40 {
        match runtime.worker.tick().await.unwrap() {
            TickOutcome::Advanced { status, run_id } => {
                if status == "waiting_approval" {
                    runtime
                        .director
                        .approve(&format!("p8-approve-{tick}"), &run_id)
                        .await
                        .unwrap();
                    approvals += 1;
                }
                if status == "succeeded" {
                    completed = true;
                    break;
                }
            }
            TickOutcome::Idle => break,
            TickOutcome::Interrupted { .. } => panic!("mock loop must not be interrupted"),
            TickOutcome::LostLease { .. } | TickOutcome::Saturated | TickOutcome::Stopped => {
                panic!("unexpected worker outcome")
            }
        }
    }
    assert!(completed, "director run must reach succeeded");
    assert!(
        approvals >= 3,
        "book and chapter gates must require approval"
    );

    let chapter_after = runtime
        .director
        .get_chapter(&novel.id, &chapter.id)
        .await
        .unwrap();
    assert_eq!(chapter_after.content, "导演流程生成的候选正文");
    assert_eq!(chapter_after.official_version, 1);
    assert_eq!(chapter_after.protection, "none");
    let status: String = sqlx::query("SELECT status FROM workflow_control WHERE run_id = ?")
        .bind(&opened.run_id)
        .fetch_one(kernel.pool())
        .await
        .unwrap()
        .try_get("status")
        .unwrap();
    assert_eq!(status, "succeeded");

    let reopened = SqliteKernel::open(&path).await.unwrap();
    let reopened_chapter = NovelRepository::get_chapter(&reopened, &novel.id, &chapter.id)
        .await
        .unwrap();
    assert_eq!(reopened_chapter.content, "导演流程生成的候选正文");
    let revisions: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM chapter_revision WHERE chapter_id = ?")
            .bind(&chapter.id)
            .fetch_one(reopened.pool())
            .await
            .unwrap();
    assert_eq!(revisions, 1);
}
