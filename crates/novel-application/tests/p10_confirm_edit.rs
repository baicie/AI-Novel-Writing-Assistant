use async_trait::async_trait;
use std::sync::Arc;

use novel_application::{
    ModelSecrets, NovelRepository, OpenRunInput, PortError, Runtime, RuntimeDeps, TickOutcome,
    WorkflowRepository,
};
use novel_storage_sqlite::SqliteKernel;
use novel_testkit::{FakeClock, ScriptedLlm, SequentialIds};

struct TestSecrets;
#[async_trait]
impl ModelSecrets for TestSecrets {
    async fn api_key(&self, _provider: &str) -> Result<Option<String>, PortError> {
        // 这个套件只验证候选稿确认流程，密钥只要存在即可。
        Ok(Some("local-test-key".into()))
    }
}

fn prompts() -> std::collections::BTreeMap<String, String> {
    let mut prompts = std::collections::BTreeMap::new();
    prompts.insert("chapter_draft".into(), "chapter-draft-v2".into());
    prompts
}
fn story() -> String {
    serde_json::json!({"expansion":{"expanded_premise":"前提","protagonist_core":"核心","conflict_engine":"冲突","conflict_layers":{"external":"外"},"mystery_box":"未知","emotional_line":"成长","setpiece_seeds":["场面"],"tone_reference":"冷峻"},"decomposition":{"selling_point":"卖点","core_conflict":"冲突","main_hook":"钩子","progression_loop":"循环","growth_path":"成长","major_payoffs":["回报"],"ending_flavor":"结局"},"constraints":["规则"],"issues":[]}).to_string()
}
fn cast() -> String {
    serde_json::json!({"members":[
    {"name":"林动","role":"主角","castRole":"protagonist","storyFunction":"作用"},
    {"name":"苏晚","role":"同伴","castRole":"ally","storyFunction":"作用"},
    {"name":"陆沉","role":"对手","castRole":"antagonist","storyFunction":"作用"}],
    "relations":[{"sourceName":"林动","targetName":"苏晚","surfaceRelation":"同门"},{"sourceName":"林动","targetName":"陆沉","surfaceRelation":"敌对"}]}).to_string()
}
fn volume() -> String {
    serde_json::json!({"volumes":[{"volumeOrder":1,"title":"卷一","roleLabel":"开局","coreReward":"进入","planningMode":"hard"}]}).to_string()
}
fn chapters() -> String {
    serde_json::json!({"chapters":[{"chapterOrder":1,"title":"第一章","volumeTitle":"卷一","summary":"入山","characterNames":["林动"]}]}).to_string()
}

#[tokio::test]
async fn author_edited_candidate_uses_cas_and_does_not_touch_official_until_commit() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.keep().join("confirm.sqlite");
    let kernel = Arc::new(SqliteKernel::open(&path).await.unwrap());
    let llm = Arc::new(ScriptedLlm::new(vec![
        ("STEP:story_macro", story()),
        ("STEP:character_cast", cast()),
        ("STEP:volume_strategy", volume()),
        ("STEP:chapter_list", chapters()),
        (
            "STEP:consistency_review",
            serde_json::json!({"issues":[]}).to_string(),
        ),
        ("STEP:chapter_draft", "原始候选正文：林动入山。".into()),
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
        secrets: Arc::new(TestSecrets),
        owner: "p10-confirm".into(),
        lease_ms: 60_000,
        concurrency: 1,
    });
    let novel = runtime
        .director
        .create_novel("候选编辑书", "")
        .await
        .unwrap();
    let chapter = runtime
        .director
        .create_chapter(&novel.id, "第一章", 1)
        .await
        .unwrap();
    let opened = runtime
        .director
        .open_run(OpenRunInput {
            command_id: "p10-confirm-open".into(),
            novel_id: novel.id.clone(),
            target_chapter_id: chapter.id.clone(),
            provider: "mock".into(),
            credential_ref: Some("mock".into()),
            model: "mock-model".into(),
            prompt_versions: prompts(),
            token_budget: Some(512),
            context_token_budget: Some(24_000),
            max_automatic_retries: 0,
            repair_budget: Some(0),
            endpoint: "https://api.example.com/v1/chat/completions".into(),
            allow_localhost_http: false,
            step_routes: Default::default(),
        })
        .await
        .unwrap();
    let mut approvals = 0;
    let mut detail = None;
    for _ in 0..30 {
        if let TickOutcome::Advanced { status, run_id } = runtime.worker.tick().await.unwrap()
            && status == "waiting_approval"
        {
            let current = runtime
                .director
                .get_run_detail(&novel.id, &run_id)
                .await
                .unwrap();
            if approvals < 2 {
                runtime
                    .director
                    .approve(&format!("p10-approve-{approvals}"), &run_id)
                    .await
                    .unwrap();
                approvals += 1;
            } else {
                detail = Some(current);
                break;
            }
        }
    }
    let detail = detail.expect("chapter approval gate");
    let candidate = detail.latest_candidate.clone().expect("candidate");
    assert_eq!(
        detail.legal_actions,
        vec!["confirm", "reject", "regenerate", "confirm_after_edit"]
    );
    let official_before = NovelRepository::get_chapter(kernel.as_ref(), &novel.id, &chapter.id)
        .await
        .unwrap();
    assert!(official_before.content.is_empty());

    let stale = runtime
        .director
        .confirm_candidate_after_edit(
            "p10-confirm-stale",
            &opened.run_id,
            Some(detail.control_version),
            &candidate.id,
            "wrong-hash",
            "不应写入的正文",
        )
        .await;
    assert!(stale.is_err(), "stale candidate hash must be rejected");
    assert!(
        NovelRepository::get_chapter(kernel.as_ref(), &novel.id, &chapter.id)
            .await
            .unwrap()
            .content
            .is_empty()
    );

    let edited = "作者修改后的候选正文：林动在夜雨之后才入山。";
    runtime
        .director
        .confirm_candidate_after_edit(
            "p10-confirm-good",
            &opened.run_id,
            Some(detail.control_version),
            &candidate.id,
            &candidate.content_sha256,
            edited,
        )
        .await
        .unwrap();
    for _ in 0..10 {
        if let TickOutcome::Advanced { status, .. } = runtime.worker.tick().await.unwrap()
            && status == "succeeded"
        {
            break;
        }
    }
    let saved = NovelRepository::get_chapter(kernel.as_ref(), &novel.id, &chapter.id)
        .await
        .unwrap();
    assert_eq!(saved.content, edited);
    assert_eq!(saved.official_version, 1);
    let artifacts = WorkflowRepository::list_artifacts(kernel.as_ref(), &novel.id)
        .await
        .unwrap();
    let edited_artifacts: Vec<_> = artifacts
        .iter()
        .filter(|artifact| artifact.artifact_type == "chapter_approval")
        .collect();
    assert!(edited_artifacts.iter().any(
        |artifact| artifact.status.as_str() == "user_edited" && artifact.protected_user_content
    ));
    let candidates: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM chapter_candidate WHERE chapter_id = ?")
            .bind(&chapter.id)
            .fetch_one(kernel.as_ref().pool())
            .await
            .unwrap();
    assert!(
        candidates >= 2,
        "original and author-edited candidates must both remain"
    );
    let reopened = SqliteKernel::open(&path).await.unwrap();
    assert_eq!(
        NovelRepository::get_chapter(&reopened, &novel.id, &chapter.id)
            .await
            .unwrap()
            .content,
        edited
    );
}
