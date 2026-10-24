use novel_application::{IndexJobRepository, NewIndexJob, NewNovel, NovelRepository};
use novel_storage_sqlite::SqliteKernel;

async fn fixture() -> (tempfile::TempDir, SqliteKernel) {
    let dir = tempfile::tempdir().unwrap();
    let kernel = SqliteKernel::open(&dir.path().join("jobs.sqlite"))
        .await
        .unwrap();
    NovelRepository::create_novel(
        &kernel,
        NewNovel {
            id: "n".into(),
            title: "索引任务".into(),
            description: String::new(),
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    (dir, kernel)
}

fn job(id: &str, command: &str, now: i64) -> NewIndexJob {
    NewIndexJob {
        id: id.into(),
        command_id: command.into(),
        novel_id: "n".into(),
        source_type: "chapter".into(),
        source_id: "ch1".into(),
        source_revision: "rev-1".into(),
        index_version: 1,
        max_attempts: 2,
        now_ms: now,
    }
}

#[tokio::test]
async fn enqueue_is_idempotent_and_claim_has_execution_lease() {
    let (_dir, kernel) = fixture().await;
    let first = kernel
        .enqueue_index_job(job("j1", "cmd-1", 10))
        .await
        .unwrap();
    let again = kernel
        .enqueue_index_job(job("j2", "cmd-1", 11))
        .await
        .unwrap();
    assert_eq!(first.id, again.id);
    assert_eq!(kernel.list_index_jobs("n").await.unwrap().len(), 1);

    let claimed = kernel
        .claim_index_job(&first.id, "worker-a", 5_000, 20)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(claimed.status, "running");
    assert!(
        kernel
            .claim_index_job(&first.id, "worker-b", 8_000, 21)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        kernel
            .complete_index_job(&first.id, "worker-b", 22)
            .await
            .is_err()
    );
    kernel
        .complete_index_job(&first.id, "worker-a", 23)
        .await
        .unwrap();
    let jobs = kernel.list_index_jobs("n").await.unwrap();
    assert_eq!(jobs[0].status, "succeeded");
}

#[tokio::test]
async fn failure_backoff_recovery_and_max_attempts_are_persisted() {
    let (_dir, kernel) = fixture().await;
    let queued = kernel
        .enqueue_index_job(job("j1", "cmd-1", 10))
        .await
        .unwrap();
    let claimed = kernel
        .claim_index_job(&queued.id, "worker-a", 1_000, 20)
        .await
        .unwrap()
        .unwrap();
    let failed = kernel
        .fail_index_job(&queued.id, "worker-a", "index_error", "boom", 2_000, 30)
        .await
        .unwrap();
    assert_eq!(failed.status, "retry_wait");
    assert_eq!(failed.attempt_count, 1);
    assert_eq!(failed.next_run_at, 2_000);

    // A killed worker's lease is recoverable, not permanently lost.
    let claimed = kernel
        .claim_index_job(&queued.id, "worker-b", 5_000, 3_000)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(claimed.attempt_count, 1);
    let recovered = kernel.recover_expired_index_jobs(6_000).await.unwrap();
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].status, "retry_wait");

    let claimed = kernel
        .claim_index_job(&queued.id, "worker-c", 9_000, 7_000)
        .await
        .unwrap()
        .unwrap();
    let exhausted = kernel
        .fail_index_job(
            &queued.id,
            "worker-c",
            "index_error",
            "boom-2",
            10_000,
            8_000,
        )
        .await
        .unwrap();
    assert_eq!(exhausted.status, "failed");
    assert_eq!(exhausted.attempt_count, 2);
}
