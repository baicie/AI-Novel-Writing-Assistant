use std::sync::Arc;

use novel_application::{
    ExtractMemoryInput, MemoryService, NewChapter, NewCharacter, NewNovel, NewTimelineEvent,
    NovelRepository,
};
use novel_storage_sqlite::SqliteKernel;
use novel_testkit::{FakeClock, ScriptedLlm, SequentialIds};

fn extraction_json() -> String {
    serde_json::json!({"facts":[
        {"factType":"character_state","truthfulness":"narrator_fact","entityName":"林动","value":"林动成为掌门","evidenceQuote":"林动成为掌门","confidence":0.9},
        {"factType":"timeline_event","truthfulness":"rumor","entityName":"陆沉","value":"传闻陆沉已死","evidenceQuote":"传闻陆沉已死","confidence":0.4},
        {"factType":"plot_thread","truthfulness":"narrator_fact","entityName":"祖祠封印","value":"祖祠封印尚未解除","evidenceQuote":"祖祠封印尚未解除","confidence":0.8},
        {"factType":"character_state","truthfulness":"narrator_fact","entityName":"无名","value":"无名人物出现","evidenceQuote":"无名人物出现","confidence":0.5}
    ]}).to_string()
}

async fn fixture(output: String) -> (Arc<SqliteKernel>, MemoryService, String, String) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.keep().join("memory.sqlite");
    let kernel = Arc::new(SqliteKernel::open(&path).await.unwrap());
    NovelRepository::create_novel(
        kernel.as_ref(),
        NewNovel {
            id: "n".into(),
            title: "记忆书".into(),
            description: String::new(),
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    NovelRepository::create_character(
        kernel.as_ref(),
        NewCharacter {
            id: "char-lin".into(),
            novel_id: "n".into(),
            name: "林动".into(),
            role: "主角".into(),
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    NovelRepository::create_character(
        kernel.as_ref(),
        NewCharacter {
            id: "char-lu".into(),
            novel_id: "n".into(),
            name: "陆沉".into(),
            role: "对手".into(),
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    NovelRepository::create_chapter(
        kernel.as_ref(),
        NewChapter {
            id: "ch1".into(),
            novel_id: "n".into(),
            title: "第一章".into(),
            sort_order: 1,
            volume_id: None,
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    NovelRepository::save_user_chapter(
        kernel.as_ref(),
        novel_application::SaveChapterRecord {
            novel_id: "n".into(),
            chapter_id: "ch1".into(),
            expected_version: 0,
            title: "第一章".into(),
            content: "林动成为掌门。林动离开山门。传闻陆沉已死。祖祠封印尚未解除。无名人物出现。"
                .into(),
            revision_id: "rev1".into(),
            artifact_id: "art1".into(),
            content_sha256: "hash1".into(),
            now_ms: 2,
        },
    )
    .await
    .unwrap();
    let llm = Arc::new(ScriptedLlm::new(vec![("STEP:memory_extraction", output)]));
    let service = MemoryService::new(
        kernel.clone(),
        kernel.clone(),
        llm,
        Arc::new(FakeClock::new(100)),
        Arc::new(SequentialIds::new()),
        Arc::new(novel_application::NoModelSecrets),
    );
    (kernel, service, "n".into(), "ch1".into())
}

async fn extract(service: &MemoryService) -> novel_application::MemoryExtractionJobView {
    service
        .extract(ExtractMemoryInput {
            command_id: "extract-1".into(),
            novel_id: "n".into(),
            chapter_id: "ch1".into(),
            provider: "mock".into(),
            model: "mock".into(),
            endpoint: "https://api.example.com/v1/chat/completions".into(),
            allow_localhost_http: false,
            token_budget: Some(512),
        })
        .await
        .unwrap()
}

#[tokio::test]
async fn extraction_is_candidate_only_and_confirm_reject_are_versioned() {
    let (kernel, service, novel, chapter) = fixture(extraction_json()).await;
    let job = extract(&service).await;
    assert_eq!(job.status, "succeeded");
    let candidates = service.list_candidates(&novel, &chapter).await.unwrap();
    assert_eq!(candidates.len(), 4);
    assert!(
        candidates
            .iter()
            .all(|candidate| candidate.status == "proposed")
    );
    let state = candidates
        .iter()
        .find(|candidate| candidate.fact_type == "character_state" && candidate.entity_id.is_some())
        .unwrap();
    let ambiguous = candidates
        .iter()
        .find(|candidate| candidate.ambiguous)
        .unwrap();
    let rumor = candidates
        .iter()
        .find(|candidate| candidate.truthfulness == "rumor")
        .unwrap();
    let plot = candidates
        .iter()
        .find(|candidate| candidate.fact_type == "plot_thread")
        .unwrap();

    let confirmed = service.confirm(&state.id, state.version).await.unwrap();
    assert_eq!(confirmed.status, "confirmed");
    let facts = NovelRepository::list_story_facts(kernel.as_ref(), &novel, 2)
        .await
        .unwrap();
    assert!(
        facts
            .iter()
            .any(|fact| fact.body == "林动成为掌门" && fact.status == "confirmed")
    );

    // A rumor is preserved as disputed evidence, never as a confirmed fact.
    service.confirm(&rumor.id, rumor.version).await.unwrap();
    let facts = NovelRepository::list_story_facts(kernel.as_ref(), &novel, 2)
        .await
        .unwrap();
    assert!(!facts.iter().any(|fact| fact.body == "传闻陆沉已死"));

    assert!(
        service
            .confirm(&ambiguous.id, ambiguous.version)
            .await
            .is_err()
    );
    let rejected = service
        .reject(&plot.id, plot.version, "人工判断不是伏笔")
        .await
        .unwrap();
    assert_eq!(rejected.status, "rejected");

    // Repeated extraction by the same command is idempotent.
    let again = extract(&service).await;
    assert_eq!(again.id, job.id);
    let after = service.list_candidates(&novel, &chapter).await.unwrap();
    assert_eq!(after.len(), 4);
}

#[tokio::test]
async fn stale_chapter_revision_blocks_confirm() {
    let (kernel, service, novel, chapter) = fixture(extraction_json()).await;
    let job = extract(&service).await;
    assert_eq!(job.status, "succeeded");
    let candidates = service.list_candidates(&novel, &chapter).await.unwrap();
    let state = candidates
        .iter()
        .find(|candidate| candidate.fact_type == "character_state" && candidate.entity_id.is_some())
        .unwrap();
    NovelRepository::save_user_chapter(
        kernel.as_ref(),
        novel_application::SaveChapterRecord {
            novel_id: novel.clone(),
            chapter_id: chapter.clone(),
            expected_version: 1,
            title: "第一章".into(),
            content: "作者修改后的正文。".into(),
            revision_id: "rev2".into(),
            artifact_id: "art2".into(),
            content_sha256: "hash2".into(),
            now_ms: 3,
        },
    )
    .await
    .unwrap();
    assert!(service.confirm(&state.id, state.version).await.is_err());
    let candidates = service.list_candidates(&novel, &chapter).await.unwrap();
    assert!(
        candidates
            .iter()
            .any(|candidate| candidate.status == "proposed")
    );
}

#[tokio::test]
async fn invalid_model_output_fails_without_candidates() {
    let (kernel, service, novel, chapter) = fixture("not json".into()).await;
    assert!(
        service
            .extract(ExtractMemoryInput {
                command_id: "bad".into(),
                novel_id: novel.clone(),
                chapter_id: chapter.clone(),
                provider: "mock".into(),
                model: "mock".into(),
                endpoint: "https://api.example.com/v1/chat/completions".into(),
                allow_localhost_http: false,
                token_budget: Some(512),
            })
            .await
            .is_err()
    );
    assert!(
        service
            .list_candidates(&novel, &chapter)
            .await
            .unwrap()
            .is_empty()
    );
    let failed: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM memory_extraction_job WHERE command_id='bad' AND status='failed'",
    )
    .fetch_one(kernel.pool())
    .await
    .unwrap();
    assert_eq!(failed, 1);
}

#[tokio::test]
async fn same_character_same_chapter_conflict_requires_manual_resolution() {
    let conflict_output = serde_json::json!({"facts":[
        {"factType":"character_state","truthfulness":"narrator_fact","entityName":"林动","value":"林动成为掌门","evidenceQuote":"林动成为掌门","confidence":0.9},
        {"factType":"character_state","truthfulness":"narrator_fact","entityName":"林动","value":"林动离开山门","evidenceQuote":"林动离开山门","confidence":0.8}
    ]}).to_string();
    let (_kernel, service, novel, chapter) = fixture(conflict_output).await;
    assert!(
        service
            .extract(ExtractMemoryInput {
                command_id: "conflict".into(),
                novel_id: novel.clone(),
                chapter_id: chapter.clone(),
                provider: "mock".into(),
                model: "mock".into(),
                endpoint: "https://api.example.com/v1/chat/completions".into(),
                allow_localhost_http: false,
                token_budget: Some(512),
            })
            .await
            .is_ok()
    );
    let candidates = service.list_candidates(&novel, &chapter).await.unwrap();
    let first = candidates
        .iter()
        .find(|candidate| candidate.value == "林动成为掌门")
        .unwrap();
    let second = candidates
        .iter()
        .find(|candidate| candidate.value == "林动离开山门")
        .unwrap();
    service.confirm(&first.id, first.version).await.unwrap();
    let conflict = service.confirm(&second.id, second.version).await;
    assert!(
        conflict.is_err(),
        "incompatible same-chapter character state must require manual resolution"
    );
    let remaining = service.list_candidates(&novel, &chapter).await.unwrap();
    assert!(
        remaining
            .iter()
            .any(|candidate| candidate.id == second.id && candidate.status == "proposed")
    );
}

#[tokio::test]
async fn conflicts_are_detected_and_resolved_with_command_idempotency() {
    let output = serde_json::json!({"facts":[
        {"factType":"character_state","truthfulness":"narrator_fact","entityName":"林动","value":"林动成为掌门","evidenceQuote":"林动成为掌门","confidence":0.9},
        {"factType":"character_state","truthfulness":"narrator_fact","entityName":"林动","value":"林动离开山门","evidenceQuote":"林动离开山门","confidence":0.8}
    ]}).to_string();
    let (kernel, service, novel, chapter) = fixture(output).await;
    assert!(
        service
            .extract(ExtractMemoryInput {
                command_id: "conflict-extract".into(),
                novel_id: novel.clone(),
                chapter_id: chapter.clone(),
                provider: "mock".into(),
                model: "mock".into(),
                endpoint: "https://api.example.com/v1/chat/completions".into(),
                allow_localhost_http: false,
                token_budget: Some(512),
            })
            .await
            .is_ok()
    );
    let candidates = service.list_candidates(&novel, &chapter).await.unwrap();
    assert_eq!(candidates.len(), 2);
    let conflicts = service.sync_conflicts(&novel).await.unwrap();
    assert_eq!(conflicts.len(), 1);
    let conflict = &conflicts[0];
    assert_eq!(conflict.status, "open");
    assert_eq!(conflict.candidate_ids.len(), 2);
    let chosen = candidates
        .iter()
        .find(|candidate| candidate.value == "林动成为掌门")
        .unwrap();
    let resolved = service
        .resolve_conflict(
            &conflict.id,
            conflict.version,
            "resolve-1",
            "confirm_new",
            Some(&chosen.id),
            "作者确认",
            "author",
        )
        .await
        .unwrap();
    assert_eq!(resolved.status, "resolved");
    let facts = NovelRepository::list_story_facts(kernel.as_ref(), &novel, 2)
        .await
        .unwrap();
    assert!(facts.iter().any(|fact| fact.body == "林动成为掌门"));
    // Same command returns the persisted resolution.
    let again = service
        .resolve_conflict(
            &conflict.id,
            conflict.version,
            "resolve-1",
            "confirm_new",
            Some(&chosen.id),
            "作者确认",
            "author",
        )
        .await
        .unwrap();
    assert_eq!(again.status, "resolved");
    // An old conflict version cannot be resolved again with a new command.
    let stale = service
        .resolve_conflict(
            &conflict.id,
            conflict.version,
            "resolve-2",
            "keep_existing",
            Some(&chosen.id),
            "重复裁决",
            "author",
        )
        .await;
    assert!(stale.is_err());
}

async fn conflict_fixture() -> (
    Arc<SqliteKernel>,
    MemoryService,
    String,
    String,
    Vec<novel_application::MemoryConflictView>,
) {
    let output = serde_json::json!({"facts":[
        {"factType":"timeline_event","truthfulness":"narrator_fact","entityName":"林动","value":"林动在祖祠","evidenceQuote":"林动成为掌门","confidence":0.9},
        {"factType":"timeline_event","truthfulness":"narrator_fact","entityName":"林动","value":"林动在码头","evidenceQuote":"林动离开山门","confidence":0.8}
    ]}).to_string();
    let (kernel, service, novel, chapter) = fixture(output).await;
    service
        .extract(ExtractMemoryInput {
            command_id: "atomic-extract".into(),
            novel_id: novel.clone(),
            chapter_id: chapter.clone(),
            provider: "mock".into(),
            model: "mock".into(),
            endpoint: "https://api.example.com/v1/chat/completions".into(),
            allow_localhost_http: false,
            token_budget: Some(512),
        })
        .await
        .unwrap();
    let conflicts = service.sync_conflicts(&novel).await.unwrap();
    (kernel, service, novel, chapter, conflicts)
}

#[tokio::test]
async fn atomic_resolution_rejects_stale_candidate_and_keep_existing_rejects_all() {
    let (kernel, service, novel, chapter, conflicts) = conflict_fixture().await;
    let conflict = conflicts
        .into_iter()
        .find(|conflict| conflict.status == "open")
        .unwrap();
    NovelRepository::save_user_chapter(
        kernel.as_ref(),
        novel_application::SaveChapterRecord {
            novel_id: novel.clone(),
            chapter_id: chapter.clone(),
            expected_version: 1,
            title: "第一章".into(),
            content: "作者修改后的正文。".into(),
            revision_id: "rev2".into(),
            artifact_id: "art2".into(),
            content_sha256: "hash2".into(),
            now_ms: 5,
        },
    )
    .await
    .unwrap();
    let chosen = conflict.candidate_ids[0].clone();
    let result = service
        .resolve_conflict(
            &conflict.id,
            conflict.version,
            "atomic-stale",
            "confirm_new",
            Some(&chosen),
            "过期候选不应写入",
            "author",
        )
        .await;
    assert!(result.is_err());
    let facts = NovelRepository::list_story_facts(kernel.as_ref(), &novel, 2)
        .await
        .unwrap();
    assert!(facts.is_empty());
    let decisions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM memory_conflict_decision")
        .fetch_one(kernel.pool())
        .await
        .unwrap();
    assert_eq!(
        decisions, 0,
        "failed atomic resolution must not record a decision"
    );
}

#[tokio::test]
async fn keep_existing_rejects_every_conflicting_candidate() {
    let (kernel, service, novel, _chapter, conflicts) = conflict_fixture().await;
    let conflict = conflicts
        .into_iter()
        .find(|conflict| conflict.status == "open")
        .unwrap();
    let resolved = service
        .resolve_conflict(
            &conflict.id,
            conflict.version,
            "keep-all",
            "keep_existing",
            None,
            "保留旧事实",
            "author",
        )
        .await
        .unwrap();
    assert_eq!(resolved.status, "resolved");
    let candidates = service
        .list_candidates(&novel, &conflict.chapter_id)
        .await
        .unwrap();
    assert!(
        candidates
            .iter()
            .all(|candidate| candidate.status == "rejected")
    );
    assert!(
        NovelRepository::list_story_facts(kernel.as_ref(), &novel, 2)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn supersede_removes_old_fact_from_confirmed_story_facts() {
    let (kernel, service, novel, _chapter, conflicts) = conflict_fixture().await;
    let conflict = conflicts
        .into_iter()
        .find(|conflict| conflict.status == "open")
        .unwrap();
    NovelRepository::upsert_timeline_event(
        kernel.as_ref(),
        NewTimelineEvent {
            id: "old-fact".into(),
            novel_id: novel.clone(),
            chapter_order: 1,
            body: "旧事实：林动在旧地点".into(),
            source_chapter_id: "ch1".into(),
            source_revision: "rev-1".into(),
            status: "confirmed".into(),
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    sqlx::query("UPDATE memory_conflict SET fact_ids_json = ? WHERE id = ?")
        .bind(serde_json::json!(["old-fact"]).to_string())
        .bind(&conflict.id)
        .execute(kernel.pool())
        .await
        .unwrap();
    let chosen = conflict.candidate_ids[0].clone();
    service
        .resolve_conflict(
            &conflict.id,
            conflict.version,
            "supersede-old",
            "supersede_with_evidence",
            Some(&chosen),
            "新证据替代旧事实",
            "author",
        )
        .await
        .unwrap();
    let facts = NovelRepository::list_story_facts(kernel.as_ref(), &novel, 2)
        .await
        .unwrap();
    assert!(
        !facts.iter().any(|fact| fact.id == "old-fact"),
        "superseded fact must not enter Context facts"
    );
}

#[tokio::test]
async fn concurrent_resolution_writes_once_and_hash_guards_command_reuse() {
    let (_kernel, service, _novel, _chapter, conflicts) = conflict_fixture().await;
    let conflict = conflicts
        .into_iter()
        .find(|conflict| conflict.status == "open")
        .unwrap();
    let chosen = conflict.candidate_ids[0].clone();
    let first = service.resolve_conflict(
        &conflict.id,
        conflict.version,
        "race",
        "confirm_new",
        Some(&chosen),
        "并发一",
        "author",
    );
    let second = service.resolve_conflict(
        &conflict.id,
        conflict.version,
        "race",
        "keep_existing",
        Some(&chosen),
        "并发二",
        "author",
    );
    let (left, right) = tokio::join!(first, second);
    assert!(
        left.is_ok() ^ right.is_ok(),
        "exactly one concurrent decision must win"
    );
    let (winning_decision, winning_reason, losing_decision, losing_reason) = if left.is_ok() {
        ("confirm_new", "并发一", "keep_existing", "并发二")
    } else {
        ("keep_existing", "并发二", "confirm_new", "并发一")
    };
    let replay = service
        .resolve_conflict(
            &conflict.id,
            conflict.version,
            "race",
            winning_decision,
            Some(&chosen),
            winning_reason,
            "author",
        )
        .await;
    assert!(replay.is_ok(), "identical command replay must be accepted");
    let reused = service
        .resolve_conflict(
            &conflict.id,
            conflict.version,
            "race",
            losing_decision,
            Some(&chosen),
            losing_reason,
            "author",
        )
        .await;
    assert!(
        reused.is_err(),
        "same commandId with different decision must conflict"
    );
}

#[tokio::test]
async fn decision_write_failure_rolls_back_confirmed_fact() {
    let (kernel, service, novel, _chapter, conflicts) = conflict_fixture().await;
    let conflict = conflicts
        .into_iter()
        .find(|conflict| conflict.status == "open")
        .unwrap();
    let chosen = conflict.candidate_ids[0].clone();
    sqlx::query(
        "CREATE TRIGGER fail_conflict_decision BEFORE INSERT ON memory_conflict_decision
         BEGIN SELECT RAISE(ABORT, 'injected decision failure'); END",
    )
    .execute(kernel.pool())
    .await
    .unwrap();
    let failed = service
        .resolve_conflict(
            &conflict.id,
            conflict.version,
            "atomic-fail",
            "confirm_new",
            Some(&chosen),
            "注入失败",
            "author",
        )
        .await;
    assert!(failed.is_err());
    let facts = NovelRepository::list_story_facts(kernel.as_ref(), &novel, 2)
        .await
        .unwrap();
    assert!(
        facts.is_empty(),
        "fact write must roll back when decision insert fails"
    );
    let decisions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM memory_conflict_decision")
        .fetch_one(kernel.pool())
        .await
        .unwrap();
    assert_eq!(decisions, 0);
    let current = service.list_conflicts(&novel).await.unwrap();
    assert_eq!(
        current
            .iter()
            .find(|item| item.id == conflict.id)
            .unwrap()
            .status,
        "open"
    );
    sqlx::query("DROP TRIGGER fail_conflict_decision")
        .execute(kernel.pool())
        .await
        .unwrap();
    let retried = service
        .resolve_conflict(
            &conflict.id,
            conflict.version,
            "atomic-fail",
            "confirm_new",
            Some(&chosen),
            "注入失败",
            "author",
        )
        .await
        .unwrap();
    assert_eq!(retried.status, "resolved");
    assert_eq!(
        NovelRepository::list_story_facts(kernel.as_ref(), &novel, 2)
            .await
            .unwrap()
            .len(),
        1
    );
}
