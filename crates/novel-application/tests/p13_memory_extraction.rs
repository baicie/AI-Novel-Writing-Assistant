use std::sync::Arc;

use novel_application::{
    ExtractMemoryInput, MemoryRepository, MemoryService, NewChapter, NewCharacter, NewNovel,
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
