use novel_application::{
    IndexJobRepository, KnowledgeRepository, NewChapter, NewDocument, NewIndexJob, NewNovel,
    NovelRepository,
};
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
    let body = "索引来源正文。".to_string();
    KnowledgeRepository::index_document(
        &kernel,
        NewDocument {
            id: "ch1".into(),
            novel_id: "n".into(),
            owner_type: "document".into(),
            title: "ch1".into(),
            body: body.clone(),
            terms: novel_context::fts_query(&novel_context::tokenize(
                &body,
                &novel_context::Lexicon::default(),
            )),
            created_at: 1,
            source_revision: Some("rev@v1".into()),
            source_content_hash: Some("hash-1".into()),
            embedding: None,
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
        source_revision: "rev@v1".into(),
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
            .complete_index_job(&first.id, "worker-b", claimed.claim_epoch, 22)
            .await
            .is_err()
    );
    kernel
        .complete_index_job(&first.id, "worker-a", claimed.claim_epoch, 23)
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
        .fail_index_job(
            &queued.id,
            "worker-a",
            claimed.claim_epoch,
            "index_error",
            "boom",
            2_000,
            30,
        )
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
            claimed.claim_epoch,
            "index_error",
            "boom-2",
            10_000,
            8_000,
        )
        .await
        .unwrap();
    assert_eq!(exhausted.status, "failed");
    assert_eq!(exhausted.attempt_count, 2);
    let requeued = kernel
        .reset_index_job_for_retry(&queued.id, 11_000)
        .await
        .unwrap();
    assert_eq!(requeued.status, "pending");
    assert_eq!(requeued.attempt_count, 0);
    assert!(
        kernel
            .claim_index_job(&queued.id, "manual", 20_000, 12_000)
            .await
            .unwrap()
            .is_some()
    );
}

#[tokio::test]
async fn stale_epoch_and_stale_source_revision_cannot_complete() {
    let (_dir, kernel) = fixture().await;
    let queued = kernel
        .enqueue_index_job(job("j1", "cmd-1", 10))
        .await
        .unwrap();
    let first = kernel
        .claim_index_job(&queued.id, "worker-a", 1_000, 20)
        .await
        .unwrap()
        .unwrap();
    kernel.recover_expired_index_jobs(2_000).await.unwrap();
    let second = kernel
        .claim_index_job(&queued.id, "worker-b", 5_000, 3_000)
        .await
        .unwrap()
        .unwrap();
    assert!(second.claim_epoch > first.claim_epoch);
    assert!(
        kernel
            .complete_index_job(&queued.id, "worker-a", first.claim_epoch, 4_000)
            .await
            .is_err()
    );
    // Source revision changed after the job was claimed: the job is obsolete for completion.
    let newer = novel_application::NewDocument {
        id: "ch1".into(),
        novel_id: "n".into(),
        owner_type: "document".into(),
        title: "ch1".into(),
        body: "索引来源正文 v2。".into(),
        terms: novel_context::fts_query(&novel_context::tokenize(
            "索引来源正文 v2",
            &novel_context::Lexicon::default(),
        )),
        created_at: 2,
        source_revision: Some("rev@v2".into()),
        source_content_hash: Some("hash-2".into()),
        embedding: None,
    };
    KnowledgeRepository::index_document(&kernel, newer)
        .await
        .unwrap();
    assert!(
        kernel
            .complete_index_job(&queued.id, "worker-b", second.claim_epoch, 6_000)
            .await
            .is_err()
    );
    let obsolete = kernel
        .mark_index_job_obsolete(
            &queued.id,
            "worker-b",
            second.claim_epoch,
            "revision changed",
            7_000,
        )
        .await
        .unwrap();
    assert_eq!(obsolete.status, "obsolete");
}

#[tokio::test]
async fn chapter_save_records_automatic_succeeded_index_job() {
    let (_dir, kernel) = fixture().await;
    NovelRepository::create_chapter(
        &kernel,
        NewChapter {
            id: "chx".into(),
            novel_id: "n".into(),
            title: "自动索引章".into(),
            sort_order: 2,
            volume_id: None,
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    NovelRepository::save_user_chapter(
        &kernel,
        novel_application::SaveChapterRecord {
            novel_id: "n".into(),
            chapter_id: "chx".into(),
            expected_version: 0,
            title: "自动索引章".into(),
            content: "自动入队章节正文。".into(),
            revision_id: "rev-auto".into(),
            artifact_id: "artifact-auto".into(),
            content_sha256: "hash-auto".into(),
            now_ms: 5,
        },
    )
    .await
    .unwrap();
    let jobs = kernel.list_index_jobs("n").await.unwrap();
    let job = jobs
        .iter()
        .find(|job| job.source_id == "chapter:chx")
        .expect("automatic chapter index job");
    assert_eq!(job.status, "succeeded");
    assert_eq!(job.source_revision, "rev-auto@v1");
}
