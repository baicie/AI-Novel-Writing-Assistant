use std::sync::Arc;

use novel_application::{OpenRunInput, Runtime, RuntimeDeps, TickOutcome, WorkflowRepository};
use novel_storage_sqlite::SqliteKernel;
use novel_testkit::{FakeClock, ScriptedLlm, SequentialIds};

fn prompts() -> std::collections::BTreeMap<String, String> {
    let mut prompts = std::collections::BTreeMap::new();
    prompts.insert("chapter_draft".into(), "chapter-draft-v2".into());
    prompts
}
fn story() -> String {
    serde_json::json!({"expansion":{"expanded_premise":"前提","protagonist_core":"核心","conflict_engine":"冲突","conflict_layers":{"external":"外"},"mystery_box":"未知","emotional_line":"成长","setpiece_seeds":["场面"],"tone_reference":"冷峻"},"decomposition":{"selling_point":"卖点","core_conflict":"冲突","main_hook":"钩子","progression_loop":"循环","growth_path":"成长","major_payoffs":["回报"],"ending_flavor":"结局"},"constraints":["规则"],"issues":[]}).to_string()
}
fn cast(name: &str) -> String {
    serde_json::json!({"members":[
    {"name":name,"role":"主角","castRole":"protagonist","storyFunction":"作用"},
    {"name":"苏晚","role":"同伴","castRole":"ally","storyFunction":"作用"},
    {"name":"陆沉","role":"对手","castRole":"antagonist","storyFunction":"作用"}],
    "relations":[{"sourceName":name,"targetName":"苏晚","surfaceRelation":"同门"},{"sourceName":name,"targetName":"陆沉","surfaceRelation":"敌对"}]}).to_string()
}
fn volume() -> String {
    serde_json::json!({"volumes":[{"volumeOrder":1,"title":"卷一","roleLabel":"开局","coreReward":"进入","planningMode":"hard"}]}).to_string()
}

#[tokio::test]
async fn reject_regenerates_and_confirm_after_edit_protects_author_artifact() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.keep().join("decisions.sqlite");
    let kernel = Arc::new(SqliteKernel::open(&path).await.unwrap());
    let llm = Arc::new(ScriptedLlm::new(vec![
        ("STEP:story_macro", story()),
        ("STEP:character_cast", cast("林动")),
        ("STEP:character_cast", cast("林动二")),
        ("STEP:volume_strategy", volume()),
    ]));
    let runtime = Runtime::new(RuntimeDeps {
        novels: kernel.clone(),
        runs: kernel.clone(),
        worlds: kernel.clone(),
        lookups: kernel.clone(),
        task_center: kernel.clone(),
        llm,
        clock: Arc::new(FakeClock::new(1_000)),
        ids: Arc::new(SequentialIds::new()),
        faults: Arc::new(novel_application::NoFault),
        secrets: Arc::new(novel_application::NoModelSecrets),
        owner: "p9-decisions".into(),
        lease_ms: 60_000,
        concurrency: 1,
    });
    let novel = runtime.director.create_novel("决策书", "").await.unwrap();
    let chapter = runtime
        .director
        .create_chapter(&novel.id, "第一章", 1)
        .await
        .unwrap();
    let opened = runtime
        .director
        .open_run(OpenRunInput {
            command_id: "p9-decisions-open".into(),
            novel_id: novel.id.clone(),
            target_chapter_id: chapter.id.clone(),
            provider: "mock".into(),
            model: "mock-model".into(),
            prompt_versions: prompts(),
            token_budget: Some(512),
            context_token_budget: Some(24_000),
            max_automatic_retries: 0,
            repair_budget: None,
            endpoint: "https://api.example.com/v1/chat/completions".into(),
            allow_localhost_http: false,
            step_routes: Default::default(),
        })
        .await
        .unwrap();
    for _ in 0..6 {
        if let TickOutcome::Advanced { status, .. } = runtime.worker.tick().await.unwrap()
            && status == "waiting_approval"
        {
            break;
        }
    }
    let first = runtime
        .director
        .get_run_detail(&novel.id, &opened.run_id)
        .await
        .unwrap();
    assert_eq!(first.status, "waiting_approval");
    assert_eq!(
        first.legal_actions,
        vec!["confirm", "reject", "regenerate", "confirm_after_edit"]
    );
    runtime
        .director
        .reject("p9-reject", &opened.run_id, Some(first.control_version))
        .await
        .unwrap();
    assert!(
        matches!(runtime.worker.tick().await.unwrap(), TickOutcome::Advanced { status, .. } if status == "waiting_approval")
    );
    let second = runtime
        .director
        .get_run_detail(&novel.id, &opened.run_id)
        .await
        .unwrap();
    let cast_artifacts: Vec<_> = second
        .artifacts
        .iter()
        .filter(|artifact| artifact.artifact_type == "character_cast")
        .collect();
    assert_eq!(
        cast_artifacts.len(),
        2,
        "rejected artifact and regenerated artifact must both remain"
    );
    assert!(
        cast_artifacts
            .iter()
            .any(|artifact| artifact.status == "stale")
    );

    runtime
        .director
        .confirm_after_edit(
            "p9-edit",
            &opened.run_id,
            Some(second.control_version),
            r#"{"members":[],"relations":[],"authorEdited":true}"#,
        )
        .await
        .unwrap();
    let edited = runtime
        .director
        .get_run_detail(&novel.id, &opened.run_id)
        .await
        .unwrap();
    assert_eq!(edited.legal_actions, vec!["cancel"]);
    let edited_artifact = edited
        .artifacts
        .iter()
        .filter(|artifact| artifact.artifact_type == "character_cast")
        .max_by_key(|artifact| artifact.version)
        .unwrap();
    assert_eq!(edited_artifact.status, "user_edited");
    assert!(edited_artifact.protected_user_content);
    assert!(edited_artifact.payload_json.contains("authorEdited"));
    // The run is runnable again; no decision command may be replayed as a stale approve.
    let runs = runtime.director.list_runs(&novel.id).await.unwrap();
    assert_eq!(runs[0].state, "running");
}
