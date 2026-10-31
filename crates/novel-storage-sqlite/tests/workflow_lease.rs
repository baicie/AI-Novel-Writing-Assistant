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
        context_token_budget: 24_000,
        max_automatic_retries: 0,
        repair_budget: 0,
        endpoint: "https://api.example.com/v1/chat/completions".into(),
        allow_localhost_http: false,
        reasoning_effort: None,
        step_routes: Default::default(),
        protocol: None,
        credential_ref: None,
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

    // A's lease has expired even before B claims: fenced writes are rejected.
    let expired = WorkflowRepository::start_attempt_fenced(
        &kernel,
        &first.fence,
        novel_application::NewAttempt {
            id: "attempt-expired".into(),
            run_id: first.run_id.clone(),
            step_id: "bounded_repair".into(),
            control_version: first.control.version,
            created_at: 1_011,
        },
        1_011,
    )
    .await;
    assert!(matches!(expired, Err(PortError::Conflict(_))));
    let third = WorkflowRepository::claim_next(&kernel, "worker-b", 1_011, 10)
        .await
        .unwrap()
        .expect("expired lease can be reclaimed");
    assert_eq!(third.control.version, first.control.version + 1);

    let stale = WorkflowRepository::apply_control(
        &kernel,
        ControlWrite {
            fence: None,
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

#[tokio::test]
async fn stale_worker_is_fenced_from_every_workflow_write() {
    let dir = TempDir::new().unwrap();
    let kernel = SqliteKernel::open(&dir.path().join("fencing.sqlite"))
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
    NovelRepository::create_chapter(
        &kernel,
        novel_application::NewChapter {
            id: "chapter-lease".into(),
            novel_id: "novel-lease".into(),
            title: "第一章".into(),
            sort_order: 1,
            volume_id: None,
            now_ms: 2,
        },
    )
    .await
    .unwrap();
    let contract = contract();
    WorkflowRepository::open_run(
        &kernel,
        OpenRunRecord {
            command_id: "open-fence".into(),
            request_hash: "open-fence-hash".into(),
            run_id: contract.run_id.clone(),
            novel_id: contract.novel_id.clone(),
            contract_json: serde_json::to_string(&contract).unwrap(),
            plan_version: contract.plan_version.clone(),
            seed_artifact_id: "seed-fence".into(),
            seed_payload: "{}".into(),
            seed_hash: "seed-hash".into(),
            now_ms: 10,
        },
    )
    .await
    .unwrap();
    let first = WorkflowRepository::claim_next(&kernel, "worker-a", 1_000, 10)
        .await
        .unwrap()
        .unwrap();
    // A repeated claim by the same owner is a new execution right; the old fence
    // must not remain valid even though the owner string is unchanged.
    let duplicate = WorkflowRepository::claim_next(&kernel, "worker-a", 1_005, 10)
        .await
        .unwrap()
        .expect("same owner can re-claim");
    assert!(duplicate.fence.control_version > first.fence.control_version);
    let old_fence_write = WorkflowRepository::start_attempt_fenced(
        &kernel,
        &first.fence,
        novel_application::NewAttempt {
            id: "attempt-old-claim".into(),
            run_id: first.run_id.clone(),
            step_id: "bounded_repair".into(),
            control_version: first.control.version,
            created_at: 1_005,
        },
        1_005,
    )
    .await;
    assert!(matches!(old_fence_write, Err(PortError::Conflict(_))));
    let second = WorkflowRepository::claim_next(&kernel, "worker-b", 1_020, 10)
        .await
        .unwrap()
        .unwrap();
    assert!(second.fence.control_version > duplicate.fence.control_version);

    let attempt = novel_application::NewAttempt {
        id: "attempt-a".into(),
        run_id: first.run_id.clone(),
        step_id: "bounded_repair".into(),
        control_version: first.control.version,
        created_at: 1_012,
    };
    assert!(matches!(
        WorkflowRepository::start_attempt_fenced(&kernel, &first.fence, attempt.clone(), 1_012)
            .await,
        Err(PortError::Conflict(_))
    ));
    assert!(matches!(
        WorkflowRepository::mark_remote_started_fenced(
            &kernel,
            &first.fence,
            &first.run_id,
            "attempt-a",
            "req-a",
            1_012
        )
        .await,
        Err(PortError::Conflict(_))
    ));
    assert!(matches!(
        WorkflowRepository::save_candidate_fenced(
            &kernel,
            &first.fence,
            novel_application::NewCandidate {
                id: "candidate-a".into(),
                chapter_id: "chapter-lease".into(),
                run_id: first.run_id.clone(),
                step_id: "bounded_repair".into(),
                content: "stale".into(),
                content_sha256: "hash".into(),
                now_ms: 1_012,
            },
            1_012
        )
        .await,
        Err(PortError::Conflict(_))
    ));
    assert!(matches!(
        WorkflowRepository::record_invocation_fenced(
            &kernel,
            &first.fence,
            novel_application::NewInvocation {
                id: "inv-a".into(),
                request_id: "req-a".into(),
                run_id: first.run_id.clone(),
                step_id: "bounded_repair".into(),
                provider: "mock".into(),
                model: "mock".into(),
                input_summary: "x".into(),
                started_at: 1_000,
                finished_at: 1_012,
                prompt_tokens: None,
                completion_tokens: None,
                status: "unknown".into(),
            },
            1_012
        )
        .await,
        Err(PortError::Conflict(_))
    ));
    assert!(matches!(
        WorkflowRepository::record_quality_debt_fenced(
            &kernel,
            &first.fence,
            &[novel_application::NewQualityDebt {
                id: "debt-a".into(),
                novel_id: "novel-lease".into(),
                chapter_id: "chapter-lease".into(),
                run_id: first.run_id.clone(),
                issue_id: "issue-a".into(),
                issue_type: "timeline".into(),
                severity: "high".into(),
                description: "stale".into(),
                evidence: "stale".into(),
                suggestion: "none".into(),
                source_ref: None,
                review_version: 1,
                now_ms: 1_012,
            }],
            1_012
        )
        .await,
        Err(PortError::Conflict(_))
    ));
    assert!(matches!(
        WorkflowRepository::apply_control(
            &kernel,
            ControlWrite {
                fence: Some(first.fence.clone()),
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
            }
        )
        .await,
        Err(PortError::Conflict(_))
    ));
    let counts: (i64, i64, i64, i64) = (
        sqlx::query_scalar("SELECT COUNT(*) FROM workflow_step_attempt WHERE run_id = ?")
            .bind(&first.run_id)
            .fetch_one(kernel.pool())
            .await
            .unwrap(),
        sqlx::query_scalar("SELECT COUNT(*) FROM chapter_candidate WHERE run_id = ?")
            .bind(&first.run_id)
            .fetch_one(kernel.pool())
            .await
            .unwrap(),
        sqlx::query_scalar("SELECT COUNT(*) FROM llm_invocation WHERE run_id = ?")
            .bind(&first.run_id)
            .fetch_one(kernel.pool())
            .await
            .unwrap(),
        sqlx::query_scalar("SELECT COUNT(*) FROM chapter_quality_debt WHERE run_id = ?")
            .bind(&first.run_id)
            .fetch_one(kernel.pool())
            .await
            .unwrap(),
    );
    assert_eq!(counts, (0, 0, 0, 0));

    // The valid owner can still write after taking the lease.
    WorkflowRepository::start_attempt_fenced(&kernel, &second.fence, attempt, 1_012)
        .await
        .unwrap();
    WorkflowRepository::mark_remote_started_fenced(
        &kernel,
        &second.fence,
        &second.run_id,
        "attempt-a",
        "req-b",
        1_012,
    )
    .await
    .unwrap();
    WorkflowRepository::save_candidate_fenced(
        &kernel,
        &second.fence,
        novel_application::NewCandidate {
            id: "candidate-b".into(),
            chapter_id: "chapter-lease".into(),
            run_id: second.run_id.clone(),
            step_id: "bounded_repair".into(),
            content: "valid".into(),
            content_sha256: "hash".into(),
            now_ms: 1_012,
        },
        1_012,
    )
    .await
    .unwrap();
    let valid_attempt: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM workflow_step_attempt WHERE run_id = ? AND id = 'attempt-a'",
    )
    .bind(&second.run_id)
    .fetch_one(kernel.pool())
    .await
    .unwrap();
    assert_eq!(valid_attempt, 1);

    // Chapter commit is fenced too: stale A cannot promote, current B can.
    let stale_promote = NovelRepository::promote_candidate_fenced(
        &kernel,
        &first.fence,
        &first.run_id,
        "chapter-lease",
        "candidate-b",
        "revision-stale",
        1_012,
    )
    .await;
    assert!(matches!(stale_promote, Err(PortError::Conflict(_))));
    let promoted = NovelRepository::promote_candidate_fenced(
        &kernel,
        &second.fence,
        &second.run_id,
        "chapter-lease",
        "candidate-b",
        "revision-valid",
        1_012,
    )
    .await
    .unwrap();
    assert!(matches!(
        promoted,
        novel_application::PromoteOutcome::Promoted { version: 1 }
    ));
    let revisions: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM chapter_revision WHERE chapter_id = 'chapter-lease'",
    )
    .fetch_one(kernel.pool())
    .await
    .unwrap();
    assert_eq!(revisions, 1);
}

#[tokio::test]
async fn candidate_checkpoint_rolls_back_when_attempt_is_not_saveable() {
    let dir = TempDir::new().unwrap();
    let kernel = SqliteKernel::open(&dir.path().join("atomic.sqlite"))
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
    NovelRepository::create_chapter(
        &kernel,
        novel_application::NewChapter {
            id: "chapter-lease".into(),
            novel_id: "novel-lease".into(),
            title: "第一章".into(),
            sort_order: 1,
            volume_id: None,
            now_ms: 2,
        },
    )
    .await
    .unwrap();
    let contract = contract();
    WorkflowRepository::open_run(
        &kernel,
        OpenRunRecord {
            command_id: "open-atomic".into(),
            request_hash: "open-atomic-hash".into(),
            run_id: contract.run_id.clone(),
            novel_id: contract.novel_id.clone(),
            contract_json: serde_json::to_string(&contract).unwrap(),
            plan_version: contract.plan_version.clone(),
            seed_artifact_id: "seed-atomic".into(),
            seed_payload: "{}".into(),
            seed_hash: "seed-hash".into(),
            now_ms: 10,
        },
    )
    .await
    .unwrap();
    let owner = WorkflowRepository::claim_next(&kernel, "worker-a", 1_000, 100)
        .await
        .unwrap()
        .unwrap();
    let result = WorkflowRepository::save_candidate_checkpoint(
        &kernel,
        &owner.fence,
        novel_application::NewCandidate {
            id: "candidate-atomic".into(),
            chapter_id: "chapter-lease".into(),
            run_id: owner.run_id.clone(),
            step_id: "bounded_repair".into(),
            content: "atomic".into(),
            content_sha256: "hash".into(),
            now_ms: 1_001,
        },
        "missing-attempt",
        novel_application::NewInvocation {
            id: "inv-atomic".into(),
            request_id: "req-atomic".into(),
            run_id: owner.run_id.clone(),
            step_id: "bounded_repair".into(),
            provider: "mock".into(),
            model: "mock".into(),
            input_summary: "x".into(),
            started_at: 1_000,
            finished_at: 1_001,
            prompt_tokens: None,
            completion_tokens: None,
            status: "unknown".into(),
        },
        1_001,
    )
    .await;
    assert!(matches!(result, Err(PortError::Conflict(_))));
    let candidates: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM chapter_candidate WHERE run_id = ?")
            .bind(&owner.run_id)
            .fetch_one(kernel.pool())
            .await
            .unwrap();
    let invocations: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM llm_invocation WHERE run_id = ?")
            .bind(&owner.run_id)
            .fetch_one(kernel.pool())
            .await
            .unwrap();
    let events: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM workflow_event WHERE run_id = ? AND kind = 'checkpoint_saved'",
    )
    .bind(&owner.run_id)
    .fetch_one(kernel.pool())
    .await
    .unwrap();
    assert_eq!((candidates, invocations, events), (0, 0, 0));
}

#[tokio::test]
async fn quality_debt_accept_is_versioned_and_preserves_history() {
    let dir = TempDir::new().unwrap();
    let kernel = SqliteKernel::open(&dir.path().join("debt.sqlite"))
        .await
        .unwrap();
    WorkflowRepository::record_quality_debt(
        &kernel,
        &[novel_application::NewQualityDebt {
            id: "debt-1".into(),
            novel_id: "n".into(),
            chapter_id: "c".into(),
            run_id: "r".into(),
            issue_id: "stable-issue".into(),
            issue_type: "timeline".into(),
            severity: "high".into(),
            description: "时间线冲突".into(),
            evidence: "证据".into(),
            suggestion: "修正".into(),
            source_ref: Some("chapter-1".into()),
            review_version: 1,
            now_ms: 10,
        }],
    )
    .await
    .unwrap();
    let open = WorkflowRepository::list_quality_debt(&kernel, "n", "c")
        .await
        .unwrap();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].decision_version, 0);
    let accepted =
        WorkflowRepository::accept_quality_debt(&kernel, "r", "debt-1", 0, "作者确认保留", 20)
            .await
            .unwrap();
    assert_eq!(accepted.status, "accepted");
    assert_eq!(accepted.decision_version, 1);
    assert_eq!(accepted.accepted_reason.as_deref(), Some("作者确认保留"));
    assert!(matches!(
        WorkflowRepository::accept_quality_debt(&kernel, "r", "debt-1", 0, "旧版本", 21).await,
        Err(PortError::Conflict(_))
    ));
    let after = WorkflowRepository::list_quality_debt(&kernel, "n", "c")
        .await
        .unwrap();
    assert_eq!(after[0].status, "accepted");
    assert_eq!(
        after[0].resolution_evidence.as_deref(),
        Some("作者确认保留")
    );
    // Accepted is not resolved; the original debt row remains.
    let resolved: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM chapter_quality_debt WHERE id='debt-1' AND status='accepted'",
    )
    .fetch_one(kernel.pool())
    .await
    .unwrap();
    assert_eq!(resolved, 1);
}
