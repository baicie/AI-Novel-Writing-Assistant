use std::sync::Arc;

use novel_application::{
    EventHub, FaultInjector, GenerationEvent, KnowledgeRepository, NewArtifact, NewDocument,
    NewEmbedding, NovelRepository, OpenRunInput, Runtime, RuntimeDeps, TickOutcome,
    WorkflowRepository,
};
use novel_context::{Lexicon, fts_query, tokenize};
use novel_llm::UnconfiguredProvider;
use novel_storage_sqlite::SqliteKernel;
use novel_testkit::{BlockingMockLlm, FailDraftAfterCandidate, FakeClock, MockLlm, SequentialIds};
use novel_workflow::{book_scope, chapter_scope};
use sqlx::Row;

fn prompts() -> std::collections::BTreeMap<String, String> {
    let mut prompts = std::collections::BTreeMap::new();
    prompts.insert("chapter_draft".into(), "chapter-draft-v1".into());
    prompts
}

async fn runtime(
    kernel: Arc<SqliteKernel>,
    llm: Arc<MockLlm>,
    faults: Arc<dyn FaultInjector>,
) -> Runtime {
    Runtime::new(RuntimeDeps {
        novels: kernel.clone(),
        runs: kernel.clone(),
        worlds: kernel.clone(),
        lookups: kernel.clone(),
        task_center: kernel.clone(),
        llm,
        clock: Arc::new(FakeClock::new(1_000)),
        ids: Arc::new(SequentialIds::new()),
        faults,
        secrets: Arc::new(novel_application::NoModelSecrets),
        owner: "test-worker".into(),
        lease_ms: 60_000,
        concurrency: 1,
    })
}

async fn seed_ready_for_draft(kernel: &SqliteKernel, novel_id: &str, chapter_id: &str) {
    for (kind, scope) in [
        ("story_macro", book_scope().to_string()),
        ("character_cast", book_scope().to_string()),
        ("volume_strategy", book_scope().to_string()),
        ("chapter_list", book_scope().to_string()),
        ("context", chapter_scope(chapter_id)),
        ("consistency_review", chapter_scope(chapter_id)),
    ] {
        WorkflowRepository::insert_external_artifact(
            kernel,
            NewArtifact {
                id: format!("seed-{kind}"),
                novel_id: novel_id.to_string(),
                run_id: None,
                artifact_type: kind.to_string(),
                scope,
                status: "confirmed".into(),
                protected_user_content: false,
                payload_json: if kind == "consistency_review" {
                    "{\"issues\":0}".into()
                } else {
                    "{}".into()
                },
                content_sha256: "seed".into(),
                created_at: 1,
            },
        )
        .await
        .unwrap();
    }
}

#[tokio::test]
async fn interrupted_draft_does_not_call_the_model_again_or_cover_author_text() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = Arc::new(
        SqliteKernel::open(&dir.path().join("novel.sqlite"))
            .await
            .unwrap(),
    );
    let llm = Arc::new(MockLlm::new("模型生成的候选"));
    let runtime = runtime(
        kernel.clone(),
        llm.clone(),
        Arc::new(FailDraftAfterCandidate),
    )
    .await;
    let novel = runtime
        .director
        .create_novel("山海", "一个开始")
        .await
        .unwrap();
    let chapter = runtime
        .director
        .create_chapter(&novel.id, "第一章", 1)
        .await
        .unwrap();
    seed_ready_for_draft(&kernel, &novel.id, &chapter.id).await;
    let opened = runtime
        .director
        .open_run(OpenRunInput {
            command_id: "open-1".into(),
            novel_id: novel.id.clone(),
            target_chapter_id: chapter.id.clone(),
            provider: "mock".into(),
            model: "mock-model".into(),
            prompt_versions: prompts(),
            token_budget: None,
            context_token_budget: None,
            max_automatic_retries: 0,
            repair_budget: None,
            endpoint: "https://api.example.com/v1/chat/completions".into(),
            allow_localhost_http: false,
            step_routes: Default::default(),
        })
        .await
        .unwrap();
    assert!(!opened.replayed);
    let replayed = runtime
        .director
        .open_run(OpenRunInput {
            command_id: "open-1".into(),
            novel_id: novel.id.clone(),
            target_chapter_id: chapter.id.clone(),
            provider: "mock".into(),
            model: "mock-model".into(),
            prompt_versions: prompts(),
            token_budget: None,
            context_token_budget: None,
            max_automatic_retries: 0,
            repair_budget: None,
            endpoint: "https://api.example.com/v1/chat/completions".into(),
            allow_localhost_http: false,
            step_routes: Default::default(),
        })
        .await
        .unwrap();
    assert!(replayed.replayed);
    assert_eq!(replayed.run_id, opened.run_id);

    let first = runtime.worker.tick().await.unwrap();
    assert!(matches!(first, TickOutcome::Interrupted { .. }));
    assert_eq!(llm.calls(), 1);
    let second = runtime.worker.tick().await.unwrap();
    assert!(matches!(second, TickOutcome::Advanced { ref status, .. } if status == "paused"));
    assert_eq!(llm.calls(), 1);
    let tokens = kernel.latest_invocation_tokens().await.unwrap().unwrap();
    assert_eq!(tokens, (None, None));

    let saved = runtime
        .director
        .save_chapter(&novel.id, &chapter.id, 0, "第一章", "人工改过的正文")
        .await
        .unwrap();
    assert_eq!(saved.content, "人工改过的正文");
    assert_eq!(saved.protection, "user_edited");
    runtime
        .director
        .resume("resume-1", &opened.run_id, true)
        .await
        .unwrap();

    let mut saw_completed = false;
    for step in 0..12 {
        let outcome = runtime.worker.tick().await.unwrap();
        if let TickOutcome::Advanced { status, run_id } = outcome {
            if status == "waiting_approval" {
                runtime
                    .director
                    .approve(&format!("approve-{step}"), &run_id)
                    .await
                    .unwrap();
            }
            if status == "succeeded" {
                saw_completed = true;
                break;
            }
        }
    }
    assert!(saw_completed);
    let final_chapter = runtime
        .director
        .get_chapter(&novel.id, &chapter.id)
        .await
        .unwrap();
    assert_eq!(final_chapter.content, "人工改过的正文");
    assert_eq!(llm.calls(), 1);
    let events = runtime.director.replay(&opened.run_id, 0).await.unwrap();
    assert!(
        events
            .iter()
            .all(|event| !matches!(event, GenerationEvent::TokenDelta { .. }))
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event, GenerationEvent::CheckpointSaved { .. }))
    );
}

async fn seed_character_gate(kernel: &SqliteKernel, novel_id: &str) {
    for (kind, status, payload) in [
        (
            "story_macro",
            "confirmed",
            r#"{"expansion":{},"decomposition":{},"constraints":[],"issues":[]}"#,
        ),
        (
            "character_cast",
            "draft",
            r#"{"members":[],"relations":[]}"#,
        ),
    ] {
        WorkflowRepository::insert_external_artifact(
            kernel,
            NewArtifact {
                id: format!("gate-{kind}"),
                novel_id: novel_id.to_string(),
                run_id: None,
                artifact_type: kind.to_string(),
                scope: book_scope().to_string(),
                status: status.to_string(),
                protected_user_content: false,
                payload_json: payload.to_string(),
                content_sha256: "gate".into(),
                created_at: 1,
            },
        )
        .await
        .unwrap();
    }
}

#[tokio::test]
async fn approval_wait_is_not_claimed_and_a_second_run_conflicts() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = Arc::new(
        SqliteKernel::open(&dir.path().join("novel.sqlite"))
            .await
            .unwrap(),
    );
    let llm = Arc::new(MockLlm::new("不会被调用"));
    let runtime = runtime(
        kernel.clone(),
        llm.clone(),
        Arc::new(novel_application::NoFault),
    )
    .await;
    let novel = runtime.director.create_novel("另一本", "").await.unwrap();
    let chapter = runtime
        .director
        .create_chapter(&novel.id, "序章", 1)
        .await
        .unwrap();
    let chapter_id = chapter.id.clone();
    seed_character_gate(&kernel, &novel.id).await;
    let opened = runtime
        .director
        .open_run(OpenRunInput {
            command_id: "open-2".into(),
            novel_id: novel.id.clone(),
            target_chapter_id: chapter_id.clone(),
            provider: "mock".into(),
            model: "mock-model".into(),
            prompt_versions: prompts(),
            token_budget: None,
            context_token_budget: None,
            max_automatic_retries: 0,
            repair_budget: None,
            endpoint: "https://api.example.com/v1/chat/completions".into(),
            allow_localhost_http: false,
            step_routes: Default::default(),
        })
        .await
        .unwrap();
    let waiting = runtime.worker.tick().await.unwrap();
    assert!(
        matches!(waiting, TickOutcome::Advanced { ref status, .. } if status == "waiting_approval")
    );
    assert!(matches!(
        runtime.worker.tick().await.unwrap(),
        TickOutcome::Idle
    ));
    assert_eq!(llm.calls(), 0);
    let conflict = runtime
        .director
        .open_run(OpenRunInput {
            command_id: "open-3".into(),
            novel_id: novel.id.clone(),
            target_chapter_id: chapter_id,
            provider: "mock".into(),
            model: "mock-model".into(),
            prompt_versions: prompts(),
            token_budget: None,
            context_token_budget: None,
            max_automatic_retries: 0,
            repair_budget: None,
            endpoint: "https://api.example.com/v1/chat/completions".into(),
            allow_localhost_http: false,
            step_routes: Default::default(),
        })
        .await;
    assert!(conflict.is_err());
    runtime
        .director
        .record_debt(&opened.run_id, &novel.id, 1, "continuity", "局部问题先记下")
        .await
        .unwrap();
    let status: String = sqlx::query("SELECT status FROM workflow_control WHERE run_id = ?")
        .bind(&opened.run_id)
        .fetch_one(kernel.pool())
        .await
        .unwrap()
        .try_get("status")
        .unwrap();
    assert_eq!(status, "waiting_approval");
}

#[tokio::test]
async fn saturated_worker_does_not_start_work() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = Arc::new(
        SqliteKernel::open(&dir.path().join("novel.sqlite"))
            .await
            .unwrap(),
    );
    let worker = novel_application::Worker::new(
        kernel.clone(),
        kernel,
        Arc::new(UnconfiguredProvider),
        Arc::new(FakeClock::new(1)),
        Arc::new(SequentialIds::new()),
        Arc::new(EventHub::new()),
        Arc::new(novel_application::NoFault),
        Arc::new(novel_application::NoModelSecrets),
        "limited".into(),
        1_000,
        0,
    );
    assert!(matches!(
        worker.tick().await.unwrap(),
        TickOutcome::Saturated
    ));
}

#[tokio::test]
async fn knowledge_search_keeps_trace_to_ids() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = SqliteKernel::open(&dir.path().join("novel.sqlite"))
        .await
        .unwrap();
    NovelRepository::create_novel(
        &kernel,
        novel_application::NewNovel {
            id: "n".into(),
            title: "书".into(),
            description: String::new(),
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    let lexicon = Lexicon {
        aliases: vec!["魂兽".into()],
    };
    let body = "林动遇见魂兽，这段正文不应该进入检索诊断。";
    let terms = tokenize(body, &lexicon).join(" ");
    KnowledgeRepository::index_document(
        &kernel,
        NewDocument {
            id: "doc".into(),
            novel_id: "n".into(),
            owner_type: "character".into(),
            title: "角色".into(),
            body: body.into(),
            terms,
            created_at: 1,
            source_revision: None,
            source_content_hash: None,
            embedding: Some(NewEmbedding {
                model: "m".into(),
                normalized: true,
                vector: vec![1.0, 0.0],
            }),
        },
    )
    .await
    .unwrap();
    KnowledgeRepository::index_document(
        &kernel,
        NewDocument {
            id: "other".into(),
            novel_id: "n".into(),
            owner_type: "world".into(),
            title: "世界".into(),
            body: "另一段".into(),
            terms: "另一 段".into(),
            created_at: 2,
            source_revision: None,
            source_content_hash: None,
            embedding: Some(NewEmbedding {
                model: "m".into(),
                normalized: true,
                vector: vec![0.0, 1.0, 0.0],
            }),
        },
    )
    .await
    .unwrap();
    let documents = KnowledgeRepository::list_documents(&kernel).await.unwrap();
    assert_eq!(documents.len(), 2);
    assert!(documents.iter().any(|document| document.title == "角色"));
    let hits = KnowledgeRepository::search_keyword(
        &kernel,
        "n",
        &fts_query(&tokenize("魂兽", &lexicon)),
        5,
    )
    .await
    .unwrap();
    assert!(hits.iter().any(|hit| hit.chunk_id == "doc:0"));
    let vectors = KnowledgeRepository::search_vector(&kernel, "n", "m", &[1.0, 0.0], true, 5)
        .await
        .unwrap();
    assert!(vectors.iter().all(|hit| hit.chunk_id != "other:0"));
    let text = novel_application::assemble_context(
        &kernel,
        "n",
        "魂兽",
        &lexicon,
        &["character".into()],
        &[],
        Some(("m", &[1.0, 0.0], true)),
        false,
        20,
        "trace-1",
        3,
    )
    .await
    .unwrap();
    assert!(text.contains("魂兽"));
    let trace = KnowledgeRepository::latest_trace(&kernel, "n")
        .await
        .unwrap()
        .unwrap();
    assert!(!trace.query_summary.contains("这段正文"));
    assert!(!trace.chunk_ids.is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn cancel_generation_during_draft_returns_cancelled_instead_of_pausing() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = Arc::new(
        SqliteKernel::open(&dir.path().join("novel.sqlite"))
            .await
            .unwrap(),
    );
    let llm: Arc<BlockingMockLlm> = Arc::new(BlockingMockLlm::new("不会被用到"));
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
        owner: "test-worker".into(),
        lease_ms: 60_000,
        concurrency: 1,
    });
    let novel = runtime
        .director
        .create_novel("可取消的书", "说明")
        .await
        .unwrap();
    let chapter = runtime
        .director
        .create_chapter(&novel.id, "第一章", 1)
        .await
        .unwrap();
    seed_ready_for_draft(&kernel, &novel.id, &chapter.id).await;
    let opened = runtime
        .director
        .open_run(OpenRunInput {
            command_id: "open-cancel".into(),
            novel_id: novel.id.clone(),
            target_chapter_id: chapter.id.clone(),
            provider: "mock".into(),
            model: "mock-model".into(),
            prompt_versions: prompts(),
            token_budget: None,
            context_token_budget: None,
            max_automatic_retries: 0,
            repair_budget: None,
            endpoint: "https://api.example.com/v1/chat/completions".into(),
            allow_localhost_http: false,
            step_routes: Default::default(),
        })
        .await
        .unwrap();
    let run_id = opened.run_id.clone();
    let worker_for_tick = runtime.worker.clone();
    let tick = tokio::spawn(async move { worker_for_tick.tick().await });
    // 等 Worker 进入 LLM 调用，再取消
    for _ in 0..50 {
        if llm.calls() == 1 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert_eq!(llm.calls(), 1, "tick should reach the LLM before cancel");
    runtime.worker.cancel_generation(&run_id);
    let outcome = tick.await.unwrap().unwrap();
    match outcome {
        TickOutcome::Advanced { status, run_id: id } => {
            assert_eq!(status, "cancelled", "want cancelled, got {status}");
            assert_eq!(id, run_id);
        }
        other => panic!("expected Advanced(cancelled), got {other:?}"),
    }
    let artifacts = WorkflowRepository::list_artifacts(kernel.as_ref(), &novel.id)
        .await
        .unwrap();
    assert!(
        !artifacts
            .iter()
            .any(|artifact| artifact.artifact_type == "chapter_commit")
    );
    let chapter = NovelRepository::get_chapter(kernel.as_ref(), &novel.id, &chapter.id)
        .await
        .unwrap();
    assert!(
        chapter.content.is_empty(),
        "cancelled run must not promote a candidate"
    );
    assert!(matches!(
        runtime.worker.tick().await.unwrap(),
        TickOutcome::Idle
    ));
}

#[tokio::test]
async fn mock_draft_stream_reaches_event_subscribers_before_checkpoint() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = Arc::new(
        SqliteKernel::open(&dir.path().join("novel.sqlite"))
            .await
            .unwrap(),
    );
    let llm = Arc::new(MockLlm::new("流式候选正文"));
    let runtime = runtime(kernel.clone(), llm, Arc::new(novel_application::NoFault)).await;
    let novel = runtime
        .director
        .create_novel("流式验证", "验证 Tauri Channel 的上游事件")
        .await
        .unwrap();
    let chapter = runtime
        .director
        .create_chapter(&novel.id, "第一章", 1)
        .await
        .unwrap();
    seed_ready_for_draft(&kernel, &novel.id, &chapter.id).await;
    let opened = runtime
        .director
        .open_run(OpenRunInput {
            command_id: "stream-open".into(),
            novel_id: novel.id.clone(),
            target_chapter_id: chapter.id.clone(),
            provider: "mock".into(),
            model: "mock-model".into(),
            prompt_versions: prompts(),
            token_budget: Some(256),
            context_token_budget: None,
            max_automatic_retries: 0,
            repair_budget: None,
            endpoint: "https://api.example.com/v1/chat/completions".into(),
            allow_localhost_http: false,
            step_routes: Default::default(),
        })
        .await
        .unwrap();
    let mut events = runtime.hub.subscribe();
    let outcome = runtime.worker.tick().await.unwrap();
    assert!(
        matches!(outcome, TickOutcome::Advanced { .. }),
        "unexpected {outcome:?}"
    );
    let mut saw_token = false;
    let mut saw_checkpoint = false;
    while let Ok(event) = events.try_recv() {
        match event {
            GenerationEvent::TokenDelta { run_id, text, .. } if run_id == opened.run_id => {
                assert_eq!(text, "流式候选正文");
                saw_token = true;
            }
            GenerationEvent::CheckpointSaved { run_id, .. } if run_id == opened.run_id => {
                saw_checkpoint = true;
            }
            _ => {}
        }
    }
    assert!(saw_token, "subscription must receive streamed tokens");
    assert!(saw_checkpoint, "worker must persist a candidate checkpoint");
}
