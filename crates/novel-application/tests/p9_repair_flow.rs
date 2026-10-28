use std::sync::Arc;

use novel_application::{
    NovelRepository, OpenRunInput, Runtime, RuntimeDeps, TickOutcome, WorkflowRepository,
};
use novel_storage_sqlite::SqliteKernel;
use novel_testkit::{FakeClock, ScriptedLlm, SequentialIds};

fn prompts() -> std::collections::BTreeMap<String, String> {
    let mut prompts = std::collections::BTreeMap::new();
    prompts.insert("chapter_draft".into(), "chapter-draft-v2".into());
    prompts
}

fn story() -> String {
    serde_json::json!({
        "expansion": {"expanded_premise":"少年入山。","protagonist_core":"被逐少年","conflict_engine":"宗门秘密","conflict_layers":{"external":"外","internal":"内"},"mystery_box":"祖祠","emotional_line":"成长","setpiece_seeds":["夜战"],"tone_reference":"冷峻"},
        "decomposition": {"selling_point":"逆袭","core_conflict":"真相","main_hook":"血脉","progression_loop":"发现","growth_path":"承担","major_payoffs":["真相"],"ending_flavor":"新生"},
        "constraints":["不揭露终极秘密"],"issues":[]
    }).to_string()
}
fn cast() -> String {
    serde_json::json!({"members":[
        {"name":"林动","role":"主角","castRole":"protagonist","storyFunction":"承载真相"},
        {"name":"苏晚","role":"同伴","castRole":"ally","storyFunction":"协助"},
        {"name":"陆沉","role":"对手","castRole":"antagonist","storyFunction":"施压"}],
        "relations":[{"sourceName":"林动","targetName":"苏晚","surfaceRelation":"同门"},{"sourceName":"林动","targetName":"陆沉","surfaceRelation":"敌对"}]}).to_string()
}
fn volume() -> String {
    serde_json::json!({"volumes":[{"volumeOrder":1,"title":"山门卷","roleLabel":"开局","coreReward":"入门","planningMode":"hard"}]}).to_string()
}
fn chapters() -> String {
    serde_json::json!({"chapters":[{"chapterOrder":1,"title":"第一章","volumeTitle":"山门卷","summary":"入山","characterNames":["林动"]}]}).to_string()
}
fn issues() -> String {
    serde_json::json!({"issues":[
        {"type":"timeline","severity":"high","evidence":"时间线冲突","suggestion":"修正时间"},
        {"type":"character","severity":"medium","evidence":"人物状态冲突","suggestion":"修正状态"}]}).to_string()
}

async fn run_with(
    outputs: Vec<(&'static str, String)>,
    max_retries: u8,
    repair_budget: Option<u8>,
) -> (Arc<SqliteKernel>, String, String, Arc<ScriptedLlm>) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.keep().join("repair.sqlite");
    let kernel = Arc::new(SqliteKernel::open(&path).await.unwrap());
    let llm = Arc::new(ScriptedLlm::new(outputs));
    let runtime = Runtime::new(RuntimeDeps {
        novels: kernel.clone(),
        runs: kernel.clone(),
        worlds: kernel.clone(),
        lookups: kernel.clone(),
        task_center: kernel.clone(),
        llm: llm.clone(),
        clock: Arc::new(FakeClock::new(1_000)),
        ids: Arc::new(SequentialIds::new()),
        faults: Arc::new(novel_application::NoFault),
        secrets: Arc::new(novel_application::NoModelSecrets),
        owner: "p9-repair-worker".into(),
        lease_ms: 60_000,
        concurrency: 1,
    });
    let novel = runtime.director.create_novel("修复书", "").await.unwrap();
    let chapter = runtime
        .director
        .create_chapter(&novel.id, "第一章", 1)
        .await
        .unwrap();
    runtime
        .director
        .open_run(OpenRunInput {
            command_id: "p9-repair-open".into(),
            novel_id: novel.id.clone(),
            target_chapter_id: chapter.id.clone(),
            provider: "mock".into(),
            model: "mock-model".into(),
            prompt_versions: prompts(),
            token_budget: Some(512),
            context_token_budget: Some(24_000),
            max_automatic_retries: max_retries,
            repair_budget,
            endpoint: "https://api.example.com/v1/chat/completions".into(),
            allow_localhost_http: false,
        })
        .await
        .unwrap();
    let mut approvals = 0;
    let mut status = "queued".to_string();
    for tick in 0..60 {
        match runtime.worker.tick().await.unwrap() {
            TickOutcome::Advanced {
                status: next,
                run_id,
            } => {
                status = next;
                if status == "waiting_approval" {
                    runtime
                        .director
                        .approve(&format!("p9-repair-approve-{tick}"), &run_id)
                        .await
                        .unwrap();
                    approvals += 1;
                }
                if status == "succeeded" || status == "paused" {
                    break;
                }
            }
            TickOutcome::Idle => break,
            other => panic!("unexpected outcome: {other:?}"),
        }
    }
    (kernel, novel.id, chapter.id, llm)
}

#[tokio::test]
async fn issues_are_repair_planned_re_reviewed_and_committed_only_after_clean_review() {
    let (kernel, novel_id, chapter_id, llm) = run_with(
        vec![
            ("STEP:story_macro", story()),
            ("STEP:character_cast", cast()),
            ("STEP:volume_strategy", volume()),
            ("STEP:chapter_list", chapters()),
            ("STEP:consistency_review", issues()),
            (
                "STEP:chapter_repair",
                "修复后的候选正文：林动按时间线在夜战后才进入祖祠。".into(),
            ),
            (
                "STEP:consistency_review",
                serde_json::json!({"issues":[]}).to_string(),
            ),
            (
                "STEP:chapter_draft",
                "原始候选正文：林动在夜战前进入祖祠。".into(),
            ),
        ],
        1,
        Some(1),
    )
    .await;
    let saved = NovelRepository::get_chapter(kernel.as_ref(), &novel_id, &chapter_id)
        .await
        .unwrap();
    assert_eq!(
        saved.content,
        "修复后的候选正文：林动按时间线在夜战后才进入祖祠。"
    );
    assert_eq!(saved.official_version, 1);
    let artifacts = WorkflowRepository::list_artifacts(kernel.as_ref(), &novel_id)
        .await
        .unwrap();
    let reviews: Vec<_> = artifacts
        .iter()
        .filter(|a| a.artifact_type == "consistency_review")
        .collect();
    assert!(reviews.len() >= 2, "repair must trigger a second review");
    assert!(artifacts.iter().any(|a| a.artifact_type == "repair_notes"));
    let candidates: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM chapter_candidate WHERE chapter_id = ?")
            .bind(&chapter_id)
            .fetch_one(kernel.as_ref().pool())
            .await
            .unwrap();
    assert!(
        candidates >= 2,
        "original and repaired candidates must both be preserved"
    );
    let debt = WorkflowRepository::list_quality_debt(kernel.as_ref(), &novel_id, &chapter_id)
        .await
        .unwrap();
    assert!(!debt.is_empty());
    assert!(debt.iter().all(|item| item.status == "resolved"));
    let prompts = llm.prompts();
    assert!(prompts.iter().any(|p| p.contains("STEP:chapter_repair")));
    assert!(
        prompts
            .iter()
            .filter(|p| p.contains("STEP:consistency_review"))
            .count()
            >= 2
    );
}

#[tokio::test]
async fn residual_issues_after_repair_stop_at_safety_without_commit() {
    let (kernel, novel_id, chapter_id, _) = run_with(
        vec![
            ("STEP:story_macro", story()),
            ("STEP:character_cast", cast()),
            ("STEP:volume_strategy", volume()),
            ("STEP:chapter_list", chapters()),
            ("STEP:consistency_review", issues()),
            ("STEP:chapter_repair", "仍有问题的修复候选".into()),
            ("STEP:consistency_review", issues()),
            ("STEP:chapter_draft", "原始候选正文".into()),
        ],
        1,
        Some(1),
    )
    .await;
    let saved = NovelRepository::get_chapter(kernel.as_ref(), &novel_id, &chapter_id)
        .await
        .unwrap();
    assert!(
        saved.content.is_empty(),
        "residual issues must not be committed"
    );
    let artifacts = WorkflowRepository::list_artifacts(kernel.as_ref(), &novel_id)
        .await
        .unwrap();
    assert!(
        !artifacts
            .iter()
            .any(|a| a.artifact_type == "chapter_commit")
    );
    let debt = WorkflowRepository::list_quality_debt(kernel.as_ref(), &novel_id, &chapter_id)
        .await
        .unwrap();
    assert!(debt.iter().any(|item| item.status == "open"));
    // The same two issues occurred in both reviews. Occurrences remain separate,
    // while stable debt identities converge to two rows.
    let occurrences: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM review_issue_occurrence WHERE chapter_id = ?")
            .bind(&chapter_id)
            .fetch_one(kernel.as_ref().pool())
            .await
            .unwrap();
    assert_eq!(occurrences, 4, "both reviews must retain their occurrences");
    assert_eq!(debt.len(), 2, "the same business issues must converge");
    assert!(debt.iter().all(|item| item.decision_version == 0));
    let occurrences_for_debt = WorkflowRepository::list_review_occurrences(
        kernel.as_ref(),
        &debt[0].run_id,
        &debt[0].issue_id,
    )
    .await
    .unwrap();
    assert_eq!(
        occurrences_for_debt.len(),
        2,
        "two reviews must preserve both occurrences"
    );
    assert_eq!(occurrences_for_debt[0].review_version, 1);
    assert_eq!(occurrences_for_debt[1].review_version, 2);
    let accepted = WorkflowRepository::accept_quality_debt(
        kernel.as_ref(),
        &debt[0].run_id,
        &debt[0].id,
        0,
        "作者接受并保留",
        2_000,
    )
    .await
    .unwrap();
    assert_eq!(accepted.status, "accepted");
    assert_eq!(accepted.decision_version, 1);
    let history = WorkflowRepository::list_quality_debt(kernel.as_ref(), &novel_id, &chapter_id)
        .await
        .unwrap();
    assert!(
        history
            .iter()
            .any(|item| item.id == accepted.id && item.status == "accepted")
    );
    assert!(history.iter().any(|item| item.status == "open"));
}

#[tokio::test]
async fn remote_retry_budget_does_not_authorize_quality_repair() {
    let (kernel, novel_id, chapter_id, llm) = run_with(
        vec![
            ("STEP:story_macro", story()),
            ("STEP:character_cast", cast()),
            ("STEP:volume_strategy", volume()),
            ("STEP:chapter_list", chapters()),
            ("STEP:consistency_review", issues()),
            ("STEP:chapter_repair", "不应被调用的修复候选".into()),
            ("STEP:chapter_draft", "原始候选正文".into()),
        ],
        1,
        None,
    )
    .await;
    let saved = NovelRepository::get_chapter(kernel.as_ref(), &novel_id, &chapter_id)
        .await
        .unwrap();
    assert!(saved.content.is_empty());
    assert!(
        !llm.prompts()
            .iter()
            .any(|prompt| prompt.contains("STEP:chapter_repair"))
    );
    let artifacts = WorkflowRepository::list_artifacts(kernel.as_ref(), &novel_id)
        .await
        .unwrap();
    assert!(
        !artifacts
            .iter()
            .any(|artifact| artifact.artifact_type == "repair_notes")
    );
}

#[tokio::test]
async fn deterministic_quality_benchmark_clean_repair_and_residual() {
    let clean = run_with(
        vec![
            ("STEP:story_macro", story()),
            ("STEP:character_cast", cast()),
            ("STEP:volume_strategy", volume()),
            ("STEP:chapter_list", chapters()),
            (
                "STEP:consistency_review",
                serde_json::json!({"issues":[]}).to_string(),
            ),
            ("STEP:chapter_draft", "干净候选正文".into()),
        ],
        0,
        Some(0),
    )
    .await;
    let clean_commits: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workflow_artifact WHERE run_id = (SELECT id FROM workflow_run LIMIT 1) AND type = 'chapter_commit'").fetch_one(clean.0.as_ref().pool()).await.unwrap();

    let repaired = run_with(
        vec![
            ("STEP:story_macro", story()),
            ("STEP:character_cast", cast()),
            ("STEP:volume_strategy", volume()),
            ("STEP:chapter_list", chapters()),
            ("STEP:consistency_review", issues()),
            ("STEP:chapter_repair", "基准修复候选正文".into()),
            (
                "STEP:consistency_review",
                serde_json::json!({"issues":[]}).to_string(),
            ),
            ("STEP:chapter_draft", "基准原始候选正文".into()),
        ],
        1,
        Some(1),
    )
    .await;
    let repaired_notes: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workflow_artifact WHERE run_id = (SELECT id FROM workflow_run LIMIT 1) AND type = 'repair_notes'").fetch_one(repaired.0.as_ref().pool()).await.unwrap();

    let residual = run_with(
        vec![
            ("STEP:story_macro", story()),
            ("STEP:character_cast", cast()),
            ("STEP:volume_strategy", volume()),
            ("STEP:chapter_list", chapters()),
            ("STEP:consistency_review", issues()),
            ("STEP:chapter_repair", "基准残留候选正文".into()),
            ("STEP:consistency_review", issues()),
            ("STEP:chapter_draft", "基准残留原始正文".into()),
        ],
        1,
        Some(1),
    )
    .await;
    let residual_debt =
        WorkflowRepository::list_quality_debt(residual.0.as_ref(), &residual.1, &residual.2)
            .await
            .unwrap();
    assert_eq!(clean_commits, 1, "clean review should commit");
    assert_eq!(repaired_notes, 1, "repairable review should repair");
    assert!(
        residual_debt.iter().any(|item| item.status == "open"),
        "residual issues must stay as open debt"
    );
    println!(
        "P12 quality benchmark: clean_commit={clean_commits} repaired_commit={repaired_notes} residual_open_debt={}",
        residual_debt
            .iter()
            .filter(|item| item.status == "open")
            .count()
    );
}

async fn run_to_quality_stop(
    outputs: Vec<(&'static str, String)>,
    repair_budget: Option<u8>,
) -> (
    Arc<SqliteKernel>,
    Runtime,
    String,
    String,
    String,
    Arc<ScriptedLlm>,
) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.keep().join("quality-stop.sqlite");
    let kernel = Arc::new(SqliteKernel::open(&path).await.unwrap());
    let llm = Arc::new(ScriptedLlm::new(outputs));
    let runtime = Runtime::new(RuntimeDeps {
        novels: kernel.clone(),
        runs: kernel.clone(),
        worlds: kernel.clone(),
        lookups: kernel.clone(),
        task_center: kernel.clone(),
        llm: llm.clone(),
        clock: Arc::new(FakeClock::new(1_000)),
        ids: Arc::new(SequentialIds::new()),
        faults: Arc::new(novel_application::NoFault),
        secrets: Arc::new(novel_application::NoModelSecrets),
        owner: "p157-quality-stop-worker".into(),
        lease_ms: 60_000,
        concurrency: 1,
    });
    let novel = runtime
        .director
        .create_novel("质量停点书", "")
        .await
        .unwrap();
    let chapter = runtime
        .director
        .create_chapter(&novel.id, "第一章", 1)
        .await
        .unwrap();
    runtime
        .director
        .open_run(OpenRunInput {
            command_id: "p157-quality-stop-open".into(),
            novel_id: novel.id.clone(),
            target_chapter_id: chapter.id.clone(),
            provider: "mock".into(),
            model: "mock-model".into(),
            prompt_versions: prompts(),
            token_budget: Some(512),
            context_token_budget: Some(24_000),
            max_automatic_retries: 1,
            repair_budget,
            endpoint: "https://api.example.com/v1/chat/completions".into(),
            allow_localhost_http: false,
        })
        .await
        .unwrap();
    let mut status = "queued".to_string();
    let mut approvals = 0;
    for _ in 0..80 {
        match runtime.worker.tick().await.unwrap() {
            TickOutcome::Advanced {
                status: next,
                run_id,
            } => {
                status = next;
                if status == "paused" || status == "succeeded" {
                    break;
                }
                if status == "waiting_approval" {
                    let detail = runtime
                        .director
                        .get_run_detail(&novel.id, &run_id)
                        .await
                        .unwrap();
                    let chapter_gate = detail.gate.as_ref().is_some_and(|gate| {
                        gate.artifact_types
                            .iter()
                            .any(|item| item == "chapter_approval")
                    });
                    assert!(
                        !chapter_gate,
                        "quality ceiling must pause before the chapter approval gate"
                    );
                    runtime
                        .director
                        .approve(&format!("p157-book-gate-{approvals}"), &run_id)
                        .await
                        .unwrap();
                    approvals += 1;
                }
            }
            TickOutcome::Idle => break,
            other => panic!("unexpected outcome: {other:?}"),
        }
    }
    assert_eq!(
        status, "paused",
        "review issues must stop at the quality ceiling"
    );
    assert!(
        approvals >= 2,
        "book-level gates must be approved before the chapter review"
    );
    let run = runtime
        .director
        .list_runs(&novel.id)
        .await
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    (kernel, runtime, novel.id, chapter.id, run.id, llm)
}

async fn tick_until_settled(runtime: &Runtime) -> String {
    let mut status = String::new();
    for _ in 0..40 {
        match runtime.worker.tick().await.unwrap() {
            TickOutcome::Advanced { status: next, .. } => {
                status = next;
                if status == "waiting_approval" || status == "succeeded" || status == "paused" {
                    break;
                }
            }
            TickOutcome::Idle => break,
            other => panic!("unexpected outcome: {other:?}"),
        }
    }
    status
}

#[tokio::test]
async fn quality_stop_requires_debt_decisions_and_then_reaches_the_approval_gate() {
    let (kernel, runtime, novel_id, chapter_id, run_id, llm) = run_to_quality_stop(
        vec![
            ("STEP:story_macro", story()),
            ("STEP:character_cast", cast()),
            ("STEP:volume_strategy", volume()),
            ("STEP:chapter_list", chapters()),
            ("STEP:consistency_review", issues()),
            (
                "STEP:chapter_repair",
                "修复后的候选正文：林动按时间线在夜战后才进入祖祠。".into(),
            ),
            ("STEP:consistency_review", issues()),
            ("STEP:chapter_draft", "原始候选正文".into()),
        ],
        Some(1),
    )
    .await;

    let detail = runtime
        .director
        .get_run_detail(&novel_id, &run_id)
        .await
        .unwrap();
    assert_eq!(detail.pause_kind.as_deref(), Some("safety"));
    assert_eq!(detail.pause_signal.as_deref(), Some("safety"));
    assert!(detail.legal_actions.contains(&"cancel".to_string()));
    assert!(!detail.legal_actions.contains(&"resume".to_string()));
    assert!(
        !detail
            .legal_actions
            .contains(&"resume_quality_debt".to_string())
    );

    let plain = runtime
        .director
        .resume("p157-resume-plain", &run_id, true)
        .await;
    assert!(
        plain.is_err(),
        "safety pauses must not accept a plain resume"
    );

    let blocked = runtime
        .director
        .resume_accepting_quality_debt("p157-resume-debt-blocked", &run_id, "作者逐条确认")
        .await;
    assert!(
        blocked.is_err(),
        "open quality reminders must block the author from continuing"
    );

    let debts = WorkflowRepository::list_quality_debt(kernel.as_ref(), &novel_id, &chapter_id)
        .await
        .unwrap();
    let open: Vec<_> = debts.iter().filter(|item| item.run_id == run_id).collect();
    assert!(!open.is_empty(), "residual review issues must stay as debt");
    for debt in &open {
        runtime
            .director
            .accept_quality_debt(&run_id, &debt.id, debt.decision_version, "作者确认保留")
            .await
            .unwrap();
    }

    let detail = runtime
        .director
        .get_run_detail(&novel_id, &run_id)
        .await
        .unwrap();
    assert!(
        detail
            .legal_actions
            .contains(&"resume_quality_debt".to_string())
    );

    runtime
        .director
        .resume_accepting_quality_debt("p157-resume-debt", &run_id, "作者逐条确认这些问题")
        .await
        .unwrap();
    let status = tick_until_settled(&runtime).await;
    assert_eq!(status, "waiting_approval");

    let detail = runtime
        .director
        .get_run_detail(&novel_id, &run_id)
        .await
        .unwrap();
    assert!(detail.legal_actions.contains(&"confirm".to_string()));
    runtime
        .director
        .approve("p157-approve-after-debt", &run_id)
        .await
        .unwrap();
    let status = tick_until_settled(&runtime).await;
    assert_eq!(status, "succeeded");

    let saved = NovelRepository::get_chapter(kernel.as_ref(), &novel_id, &chapter_id)
        .await
        .unwrap();
    assert_eq!(saved.official_version, 1);
    assert!(saved.content.contains("修复后的候选正文"));
    assert_eq!(
        llm.prompts()
            .iter()
            .filter(|prompt| prompt.contains("STEP:chapter_repair"))
            .count(),
        1,
        "bounded repair must call the model once"
    );
}

#[tokio::test]
async fn zero_repair_budget_records_a_skip_decision_instead_of_a_dead_end() {
    let (kernel, runtime, novel_id, chapter_id, run_id, llm) = run_to_quality_stop(
        vec![
            ("STEP:story_macro", story()),
            ("STEP:character_cast", cast()),
            ("STEP:volume_strategy", volume()),
            ("STEP:chapter_list", chapters()),
            ("STEP:consistency_review", issues()),
            ("STEP:chapter_repair", "不应被调用的修复候选".into()),
            ("STEP:chapter_draft", "原始候选正文".into()),
        ],
        None,
    )
    .await;

    let debts = WorkflowRepository::list_quality_debt(kernel.as_ref(), &novel_id, &chapter_id)
        .await
        .unwrap();
    for debt in debts.iter().filter(|item| item.run_id == run_id) {
        runtime
            .director
            .accept_quality_debt(&run_id, &debt.id, debt.decision_version, "作者确认保留")
            .await
            .unwrap();
    }
    runtime
        .director
        .resume_accepting_quality_debt("p157-resume-debt-budget-zero", &run_id, "作者接受现状")
        .await
        .unwrap();
    let status = tick_until_settled(&runtime).await;
    assert_eq!(status, "waiting_approval");

    let artifacts = WorkflowRepository::list_artifacts(kernel.as_ref(), &novel_id)
        .await
        .unwrap();
    let notes: Vec<_> = artifacts
        .iter()
        .filter(|artifact| artifact.artifact_type == "repair_notes")
        .collect();
    assert_eq!(notes.len(), 1);
    assert!(notes[0].payload_json.contains("skipped_by_author"));
    assert!(
        !llm.prompts()
            .iter()
            .any(|prompt| prompt.contains("STEP:chapter_repair")),
        "zero repair budget must never call the repair model"
    );

    runtime
        .director
        .approve("p157-approve-budget-zero", &run_id)
        .await
        .unwrap();
    let status = tick_until_settled(&runtime).await;
    assert_eq!(status, "succeeded");
    let saved = NovelRepository::get_chapter(kernel.as_ref(), &novel_id, &chapter_id)
        .await
        .unwrap();
    assert_eq!(saved.official_version, 1);
    assert!(saved.content.contains("原始候选正文"));
}
