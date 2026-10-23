use std::collections::BTreeMap;

use novel_application::{
    ControlWrite, NovelRepository, OpenRunRecord, PortError, RunContract, WorkflowRepository,
};
use novel_domain::ControlEvent;
use novel_storage_sqlite::SqliteKernel;
use tempfile::TempDir;

fn contract() -> RunContract {
    RunContract {
        run_id: "run-lease".into(),
        novel_id: "novel-lease".into(),
        plan_version: "plan-v1".into(),
        chapter_plan_version: "chapter-v1".into(),
        target_chapter_id: "chapter-lease".into(),
        provider: "mock".into(),
        model: "mock".into(),
        prompt_versions: BTreeMap::new(),
        token_budget: Some(100),
        max_automatic_retries: 0,
        endpoint: "https://api.example.com/v1/chat/completions".into(),
        allow_localhost_http: false,
    }
}

#[tokio::test]
async fn expired_lease_can_be_reclaimed_and_stale_worker_cannot_commit() {
    let dir = TempDir::new().unwrap();
    let kernel = SqliteKernel::open(&dir.path().join("workflow.sqlite"))
        .await
        .unwrap();
    NovelRepository::create_novel(
        &kernel,
        novel_application::NewNovel {
            id: "novel-lease".into(),
            title: "租约测试".into(),
            description: String::new(),
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    let contract = contract();
    let opened = WorkflowRepository::open_run(
        &kernel,
        OpenRunRecord {
            command_id: "open-lease".into(),
            request_hash: "open-lease-hash".into(),
            run_id: contract.run_id.clone(),
            novel_id: contract.novel_id.clone(),
            contract_json: serde_json::to_string(&contract).unwrap(),
            plan_version: contract.plan_version.clone(),
            seed_artifact_id: "seed-lease".into(),
            seed_payload: "{}".into(),
            seed_hash: "seed-hash".into(),
            now_ms: 10,
        },
    )
    .await
    .unwrap();
    assert_eq!(opened.status, "queued");

    let first = WorkflowRepository::claim_next(&kernel, "worker-a", 1_000, 10)
        .await
        .unwrap()
        .expect("first worker claims");
    let second = WorkflowRepository::claim_next(&kernel, "worker-b", 1_005, 10)
        .await
        .unwrap();
    assert!(second.is_none(), "active lease must not be stolen");

    let third = WorkflowRepository::claim_next(&kernel, "worker-b", 1_011, 10)
        .await
        .unwrap()
        .expect("expired lease can be reclaimed");
    assert_eq!(third.control.version, first.control.version + 1);

    let stale = WorkflowRepository::apply_control(
        &kernel,
        ControlWrite {
            run_id: first.run_id.clone(),
            expected_version: first.control.version,
            event: ControlEvent::StepSucceeded,
            artifact: None,
            event_kind: "step_succeeded".into(),
            event_step_id: None,
            event_payload: "{}".into(),
            occurred_at: 1_012,
            invocation: None,
            attempt_id: None,
            attempt_phase: None,
            clear_lease: false,
        },
    )
    .await;
    assert!(matches!(stale, Err(PortError::Conflict(_))));
}
