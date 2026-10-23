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
            endpoint: "https://api.example.com/v1/chat/completions".into(),
            allow_localhost_http: false,
        })
        .await
        .unwrap();
    let mut approvals = 0;
    let mut status = "queued".to_string();
    for tick in 0..60 {
        let tick_result = runtime.worker.tick().await;
        if let Err(error) = tick_result {
            let runs = runtime
                .director
                .list_runs(&novel.id)
                .await
                .unwrap_or_default();
            if runs.iter().any(|run| run.state == "paused") {
                break;
            }
            panic!("unexpected worker error: {error:?}");
        }
        match tick_result.unwrap() {
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
            ("STEP:chapter_draft", "原始候选正文".into()),
        ],
        1,
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
}
