use novel_application::{
    BatchFence, BatchRepository, NewBatchRun, NewChapter, NewNovel, NovelRepository,
};
use novel_storage_sqlite::SqliteKernel;

async fn fixture() -> (tempfile::TempDir, SqliteKernel) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("batch.sqlite");
    let kernel = SqliteKernel::open(&path).await.unwrap();
    NovelRepository::create_novel(
        &kernel,
        NewNovel {
            id: "n1".into(),
            title: "批量".into(),
            description: String::new(),
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    for (index, id) in ["c1", "c2", "c3"].iter().enumerate() {
        NovelRepository::create_chapter(
            &kernel,
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
    (dir, kernel)
}

fn record(command_id: &str, hash: &str) -> NewBatchRun {
    NewBatchRun {
        id: "b1".into(),
        novel_id: "n1".into(),
        command_id: command_id.into(),
        request_hash: hash.into(),
        request_json: "{}".into(),
        items: vec![
            ("i1".into(), "c1".into(), 0),
            ("i2".into(), "c2".into(), 1),
            ("i3".into(), "c3".into(), 2),
        ],
        now_ms: 10,
    }
}

#[tokio::test]
async fn batch_creation_is_idempotent_and_durable() {
    let (dir, kernel) = fixture().await;
    let created = kernel
        .create_batch(record("cmd-1", "hash-1"))
        .await
        .unwrap();
    assert_eq!(created.status, "created");
    assert_eq!(created.items.len(), 3);
    let replay = kernel
        .create_batch(record("cmd-1", "hash-1"))
        .await
        .unwrap();
    assert_eq!(replay.id, created.id);
    assert_eq!(replay.items.len(), 3);
    let conflict = kernel.create_batch(record("cmd-1", "hash-2")).await;
    assert!(
        conflict.is_err(),
        "same command with different request must conflict"
    );
    drop(kernel);
    let reopened = SqliteKernel::open(&dir.path().join("batch.sqlite"))
        .await
        .unwrap();
    let loaded = reopened.get_batch("b1").await.unwrap();
    assert_eq!(loaded.items.len(), 3);
    assert_eq!(loaded.items[2].chapter_id, "c3");
}

#[tokio::test]
async fn batch_claim_and_state_update_are_fenced() {
    let (_dir, kernel) = fixture().await;
    kernel
        .create_batch(record("cmd-2", "hash-1"))
        .await
        .unwrap();
    let claimed = kernel
        .claim_batch("b1", "scheduler-a", 100_000, 20)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(claimed.claim_epoch, 1);
    let stale = BatchFence {
        owner: "scheduler-b".into(),
        claim_epoch: 1,
        version: claimed.version,
    };
    assert!(
        kernel
            .update_batch(&stale, "running", 0, None, 30)
            .await
            .is_err()
    );
    let fence = BatchFence {
        owner: "scheduler-a".into(),
        claim_epoch: claimed.claim_epoch,
        version: claimed.version,
    };
    let updated = kernel
        .update_batch(&fence, "running", 0, None, 30)
        .await
        .unwrap();
    assert_eq!(updated.status, "running");
    assert_eq!(updated.version, claimed.version + 1);
}
