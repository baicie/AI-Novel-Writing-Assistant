use novel_application::{
    BatchRepository, NativeTaskQuery, NativeTaskRepository, NewBatchRun, NewChapter, NewNovel,
    NovelRepository,
};
use novel_storage_sqlite::SqliteKernel;

#[tokio::test]
async fn native_task_projection_merges_persisted_domains_and_maps_actions() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = SqliteKernel::open(&dir.path().join("tasks.sqlite"))
        .await
        .unwrap();
    NovelRepository::create_novel(
        &kernel,
        NewNovel {
            id: "n1".into(),
            title: "任务".into(),
            description: String::new(),
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    NovelRepository::create_chapter(
        &kernel,
        NewChapter {
            id: "c1".into(),
            novel_id: "n1".into(),
            title: "第一章".into(),
            sort_order: 1,
            volume_id: None,
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    // Director run waiting approval.
    sqlx::query("INSERT INTO workflow_run(id, novel_id, plan_version, contract_json, created_at) VALUES ('r1','n1','p','{}',10)").execute(kernel.pool()).await.unwrap();
    sqlx::query("INSERT INTO workflow_control(run_id, novel_id, status, version, pause_json, gate_json, cursor_step_id, failure_reason) VALUES ('r1','n1','waiting_approval',1,NULL,NULL,'chapter_approval',NULL)").execute(kernel.pool()).await.unwrap();
    // Batch.
    kernel
        .create_batch(NewBatchRun {
            id: "b1".into(),
            novel_id: "n1".into(),
            command_id: "cmd-b".into(),
            request_hash: "h".into(),
            request_json: "{}".into(),
            items: vec![("i1".into(), "c1".into(), 0)],
            now_ms: 20,
        })
        .await
        .unwrap();
    // Index job failed.
    sqlx::query("INSERT INTO knowledge_index_job(id, command_id, novel_id, source_type, source_id, source_revision, index_version, status, attempt_count, max_attempts, claim_epoch, next_run_at, created_at, updated_at) VALUES ('j1','cmd-j','n1','document','d1','v1',1,'failed',1,3,0,0,30,30)").execute(kernel.pool()).await.unwrap();
    // Memory extraction job.
    sqlx::query("INSERT INTO memory_extraction_job(id, command_id, novel_id, chapter_id, chapter_version, source_content_hash, extraction_version, provider, model, endpoint, status, created_at, updated_at) VALUES ('m1','cmd-m','n1','c1',1,'hash','v1','mock','mock','endpoint','failed',40,40)").execute(kernel.pool()).await.unwrap();
    // Quality debt.
    sqlx::query("INSERT INTO quality_debt(id, novel_id, run_id, chapter_order, code, detail, created_at) VALUES ('q1','n1','r1',1,'timeline','时间线待处理',50)").execute(kernel.pool()).await.unwrap();

    let tasks = kernel
        .list_native_tasks(&NativeTaskQuery {
            novel_id: Some("n1".into()),
            ..Default::default()
        })
        .await
        .unwrap();
    let types: std::collections::HashSet<_> =
        tasks.iter().map(|task| task.task_type.as_str()).collect();
    assert!(types.contains("director_run"));
    assert!(types.contains("batch"));
    assert!(types.contains("index_job"));
    assert!(types.contains("memory_extraction"));
    assert!(types.contains("quality_debt"));
    let approval = tasks
        .iter()
        .find(|task| task.task_type == "director_run")
        .unwrap();
    assert_eq!(approval.status, "waiting_approval");
    assert!(approval.requires_user_action);
    assert!(
        approval
            .available_actions
            .contains(&"open_approval".to_string())
    );
    let retry = tasks
        .iter()
        .find(|task| task.task_type == "index_job")
        .unwrap();
    assert_eq!(retry.available_actions, vec!["open_detail".to_string()]);
    assert_eq!(retry.status, "failed");
    let wrong_novel = kernel
        .list_native_tasks(&NativeTaskQuery {
            novel_id: Some("other".into()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(wrong_novel.is_empty());
    let failed = kernel
        .list_native_tasks(&NativeTaskQuery {
            status: Some("failed".into()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(failed.iter().all(|task| task.status == "failed"));
}
