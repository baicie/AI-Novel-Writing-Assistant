use std::process::Command;
use std::time::{Duration, Instant};

use novel_application::{
    IndexJobRepository, KnowledgeRepository, NewDocument, NewIndexJob, NewNovel, NovelRepository,
};
use novel_storage_sqlite::SqliteKernel;

#[tokio::test]
async fn killed_index_worker_is_recovered_by_fencing_and_reindexed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("worker.sqlite");
    let kernel = SqliteKernel::open(&path).await.unwrap();
    NovelRepository::create_novel(
        &kernel,
        NewNovel {
            id: "n".into(),
            title: "worker".into(),
            description: String::new(),
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    let body = "worker 来源正文。".to_string();
    KnowledgeRepository::index_document(
        &kernel,
        NewDocument {
            id: "doc1".into(),
            novel_id: "n".into(),
            owner_type: "document".into(),
            title: "doc1".into(),
            body: body.clone(),
            terms: novel_context::fts_query(&novel_context::tokenize(
                &body,
                &novel_context::Lexicon::default(),
            )),
            created_at: 1,
            source_revision: Some("rev-1".into()),
            source_content_hash: Some("hash-1".into()),
            embedding: None,
        },
    )
    .await
    .unwrap();
    sqlx::query("UPDATE knowledge_document SET index_status = 'building' WHERE id = 'doc1'")
        .execute(kernel.pool())
        .await
        .unwrap();
    let job = kernel
        .enqueue_index_job(NewIndexJob {
            id: "job-crash".into(),
            command_id: "cmd-crash".into(),
            novel_id: "n".into(),
            source_type: "document".into(),
            source_id: "doc1".into(),
            source_revision: "rev-1".into(),
            index_version: 1,
            max_attempts: 3,
            now_ms: 10,
        })
        .await
        .unwrap();
    let marker = dir.path().join("worker.pid");
    let mut child = Command::new(env!("CARGO_BIN_EXE_index_worker"))
        .env("INDEX_WORKER_DB", &path)
        .env("INDEX_WORKER_JOB", &job.id)
        .env("INDEX_WORKER_OWNER", "worker-killed")
        .env("INDEX_WORKER_MARKER", &marker)
        .env("INDEX_WORKER_LEASE_MS", "300")
        .spawn()
        .unwrap();
    let start = Instant::now();
    while !marker.exists() && start.elapsed() < Duration::from_secs(10) {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(marker.exists(), "worker did not claim job");
    let _ = child.kill();
    let status = child.wait().unwrap();
    assert!(!status.success(), "worker should have been SIGKILLed");
    tokio::time::sleep(Duration::from_millis(500)).await;

    let recovered = kernel
        .recover_expired_index_jobs(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64,
        )
        .await
        .unwrap();
    assert_eq!(recovered.len(), 1);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    let claimed = kernel
        .claim_index_job(&job.id, "worker-new", now + 5_000, now)
        .await
        .unwrap()
        .unwrap();
    assert!(claimed.claim_epoch > 1);
    assert!(
        KnowledgeRepository::retry_index_document(&kernel, "doc1")
            .await
            .unwrap()
    );
    kernel
        .complete_index_job(&job.id, "worker-new", claimed.claim_epoch, now + 1)
        .await
        .unwrap();
    let jobs = kernel.list_index_jobs("n").await.unwrap();
    assert_eq!(jobs[0].status, "succeeded");
    let chunks: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM knowledge_chunk WHERE document_id = 'doc1'")
            .fetch_one(kernel.pool())
            .await
            .unwrap();
    assert_eq!(chunks, 1, "recovery must not duplicate chunks");
    let revision: String = sqlx::query_scalar(
        "SELECT source_revision FROM knowledge_chunk WHERE document_id = 'doc1'",
    )
    .fetch_one(kernel.pool())
    .await
    .unwrap();
    assert_eq!(revision, "rev-1");
}
