use std::sync::Arc;

use novel_application::{
    ContextAssembly, NewChapter, NewNovel, NewWorld, NovelRepository, WorldRepository,
    assemble_director_context,
};
use novel_domain::{ArtifactStatus, ArtifactView};
use novel_storage_sqlite::SqliteKernel;
use novel_workflow::{book_scope, chapter_scope};

fn artifact(ty: &str, version: i64, scope: &str, payload: serde_json::Value) -> ArtifactView {
    ArtifactView {
        artifact_type: ty.into(),
        scope: scope.into(),
        version,
        status: ArtifactStatus::Confirmed,
        protected_user_content: false,
        payload_json: payload.to_string(),
    }
}

fn story() -> serde_json::Value {
    serde_json::json!({
        "expansion": {"expanded_premise":"林动走进山门。","protagonist_core":"被逐少年"},
        "decomposition": {"core_conflict":"宗门秘密"},
        "constraints":["不揭露终极秘密"], "issues":[]
    })
}
fn cast() -> serde_json::Value {
    serde_json::json!({"members":[
        {"name":"林动","role":"主角","castRole":"protagonist","storyFunction":"承担真相"},
        {"name":"苏晚","role":"同伴","castRole":"ally","storyFunction":"协助"},
        {"name":"陆沉","role":"对手","castRole":"antagonist","storyFunction":"施压"}],
        "relations":[]})
}
fn volume() -> serde_json::Value {
    serde_json::json!({"volumes":[{"volumeOrder":1,"title":"山门卷","roleLabel":"开局","coreReward":"入门","planningMode":"hard"}]})
}
fn plan(target_order: i64) -> serde_json::Value {
    serde_json::json!({"chapters":[{"chapterOrder":target_order,"title":"第四章 夜战","volumeTitle":"山门卷","summary":"林动在夜里迎战陆沉","characterNames":["林动","陆沉"]}]})
}
fn artifacts() -> Vec<ArtifactView> {
    vec![
        artifact("story_macro", 1, book_scope(), story()),
        artifact("character_cast", 1, book_scope(), cast()),
        artifact("volume_strategy", 1, book_scope(), volume()),
        artifact("chapter_list", 1, book_scope(), plan(4)),
    ]
}

async fn fixture() -> (Arc<SqliteKernel>, String, String, String) {
    let dir = tempfile::tempdir().unwrap();
    // Keep the TempDir alive by leaking the path in a stable temp directory for the test.
    let path = dir.keep().join("context.sqlite");
    let kernel = Arc::new(SqliteKernel::open(&path).await.unwrap());
    NovelRepository::create_novel(
        kernel.as_ref(),
        NewNovel {
            id: "novel-a".into(),
            title: "山门".into(),
            description: "一个被逐出家族的少年进入山门。".into(),
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    NovelRepository::create_chapter(
        kernel.as_ref(),
        NewChapter {
            id: "ch1".into(),
            novel_id: "novel-a".into(),
            title: "第一章".into(),
            sort_order: 1,
            volume_id: None,
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    NovelRepository::create_chapter(
        kernel.as_ref(),
        NewChapter {
            id: "ch2".into(),
            novel_id: "novel-a".into(),
            title: "第二章".into(),
            sort_order: 2,
            volume_id: None,
            now_ms: 2,
        },
    )
    .await
    .unwrap();
    NovelRepository::create_chapter(
        kernel.as_ref(),
        NewChapter {
            id: "ch3".into(),
            novel_id: "novel-a".into(),
            title: "第三章".into(),
            sort_order: 3,
            volume_id: None,
            now_ms: 3,
        },
    )
    .await
    .unwrap();
    NovelRepository::create_chapter(
        kernel.as_ref(),
        NewChapter {
            id: "ch4".into(),
            novel_id: "novel-a".into(),
            title: "第四章".into(),
            sort_order: 4,
            volume_id: None,
            now_ms: 4,
        },
    )
    .await
    .unwrap();
    NovelRepository::create_chapter(
        kernel.as_ref(),
        NewChapter {
            id: "ch5-future".into(),
            novel_id: "novel-a".into(),
            title: "未来章".into(),
            sort_order: 5,
            volume_id: None,
            now_ms: 5,
        },
    )
    .await
    .unwrap();
    for (chapter_id, version, content) in [
        ("ch1", 1i64, "第一章正式正文：林动进入山门。"),
        ("ch2", 1, "第二章正式正文：苏晚在山门等他。"),
        ("ch3", 1, "第三章正式正文：陆沉在夜里出现。"),
        ("ch5-future", 1, "未来章节不应进入当前上下文。"),
    ] {
        NovelRepository::save_user_chapter(
            kernel.as_ref(),
            novel_application::SaveChapterRecord {
                novel_id: "novel-a".into(),
                chapter_id: chapter_id.into(),
                expected_version: 0,
                title: format!("第{version}章"),
                content: content.into(),
                revision_id: format!("rev-{chapter_id}"),
                artifact_id: format!("art-{chapter_id}"),
                content_sha256: format!("hash-{chapter_id}"),
                now_ms: 10 + version,
            },
        )
        .await
        .unwrap();
    }
    WorldRepository::create_world(
        kernel.as_ref(),
        NewWorld {
            id: "world-a".into(),
            name: "山门世界".into(),
            description: format!("宗门以剑为尊。{}", "设定".repeat(500)),
            overview_summary: "山门封闭，外门弟子不得进入祖祠。".into(),
            source_template_id: None,
            owner_novel_id: Some("novel-a".into()),
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    sqlx::query("INSERT INTO character(id, novel_id, name, role, profile_json, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind("char-lin").bind("novel-a").bind("林动").bind("主角").bind("{\"state\":\"刚入外门\"}").bind(1).bind(1)
        .execute(kernel.as_ref().pool()).await.unwrap();
    // A candidate must never be treated as confirmed context.
    sqlx::query("INSERT INTO chapter_candidate(id, chapter_id, run_id, step_id, content, content_sha256, interrupted, created_at, updated_at) VALUES ('cand','ch4','run','draft','未确认候选正文','h',0,1,1)")
        .execute(kernel.as_ref().pool()).await.unwrap();
    (kernel, "novel-a".into(), "ch4".into(), "novel-b".into())
}

#[tokio::test]
async fn context_is_real_traceable_deterministic_and_scoped() {
    let (kernel, novel_id, target_chapter, _) = fixture().await;
    let first = assemble_director_context(
        kernel.as_ref(),
        &artifacts(),
        &novel_id,
        &target_chapter,
        24_000,
        8_000,
    )
    .await
    .unwrap();
    let second = assemble_director_context(
        kernel.as_ref(),
        &artifacts(),
        &novel_id,
        &target_chapter,
        24_000,
        8_000,
    )
    .await
    .unwrap();
    assert_eq!(first.prompt_text, second.prompt_text);
    assert_eq!(first.trace.content_hash, second.trace.content_hash);
    assert_eq!(first.trace.assembly_version, "director-context-v1");
    assert!(first.prompt_text.contains("宗门以剑为尊"));
    assert!(first.prompt_text.contains("林动"));
    assert!(first.prompt_text.contains("第一章正式正文"));
    assert!(first.prompt_text.contains("第三章正式正文"));
    assert!(!first.prompt_text.contains("未来章节"));
    assert!(!first.prompt_text.contains("未确认候选正文"));
    let sources = &first.trace.sources;
    assert!(
        sources
            .iter()
            .any(|s| s.source_type == "world_facts" && s.selected)
    );
    assert!(
        sources
            .iter()
            .any(|s| s.source_type == "character_facts" && s.selected)
    );
    assert!(
        sources
            .iter()
            .any(|s| s.source_type == "confirmed_chapter" && s.selected)
    );
    assert!(sources.iter().any(|s| s.source_type == "recent_summary"
        && s.exclusion_reason.as_deref() == Some("not_migrated")));
    assert!(
        sources
            .iter()
            .any(|s| s.source_type == "user_supplement" && !s.selected)
    );
}

#[tokio::test]
async fn context_marks_missing_sources_and_rejects_too_small_budget() {
    let (kernel, novel_id, target_chapter, _) = fixture().await;
    let normal = assemble_director_context(
        kernel.as_ref(),
        &artifacts(),
        &novel_id,
        &target_chapter,
        24_000,
        8_000,
    )
    .await
    .unwrap();
    assert_eq!(normal.trace.input_budget, 24_000);
    assert_eq!(normal.trace.output_reserve, 8_000);
    let mandatory_types = [
        "novel_seed",
        "story_macro",
        "character_cast",
        "volume_strategy",
        "chapter_list",
    ];
    let mandatory_tokens: u64 = normal
        .trace
        .sources
        .iter()
        .filter(|source| mandatory_types.contains(&source.source_type.as_str()) && source.selected)
        .map(|source| source.estimated_tokens)
        .sum();
    let trimmed = assemble_director_context(
        kernel.as_ref(),
        &artifacts(),
        &novel_id,
        &target_chapter,
        (mandatory_tokens + 1).max(512),
        50,
    )
    .await
    .unwrap();
    assert!(trimmed.trace.sources.iter().any(
        |source| !source.selected && source.exclusion_reason.as_deref() == Some("over_budget")
    ));
    // Mandatory fact overflow must be an explicit error, not a silent empty context.
    let huge = serde_json::json!({"expansion": {"expanded_premise": "长".repeat(4_000)}, "decomposition": {"core_conflict":"冲突"}});
    let huge_artifacts = vec![
        artifact("story_macro", 1, book_scope(), huge),
        artifact("character_cast", 1, book_scope(), cast()),
        artifact("volume_strategy", 1, book_scope(), volume()),
        artifact("chapter_list", 1, book_scope(), plan(4)),
    ];
    let overflow = assemble_director_context(
        kernel.as_ref(),
        &huge_artifacts,
        &novel_id,
        &target_chapter,
        512,
        50,
    )
    .await;
    assert!(overflow.is_err());
}

#[tokio::test]
async fn context_does_not_cross_novels_and_missing_data_is_explicit() {
    let (kernel, _, _, _) = fixture().await;
    NovelRepository::create_novel(
        kernel.as_ref(),
        NewNovel {
            id: "novel-b".into(),
            title: "另一本".into(),
            description: "另一本书".into(),
            now_ms: 3,
        },
    )
    .await
    .unwrap();
    NovelRepository::create_chapter(
        kernel.as_ref(),
        NewChapter {
            id: "b1".into(),
            novel_id: "novel-b".into(),
            title: "B章".into(),
            sort_order: 1,
            volume_id: None,
            now_ms: 3,
        },
    )
    .await
    .unwrap();
    let b_artifacts = vec![
        artifact("story_macro", 1, book_scope(), story()),
        artifact("character_cast", 1, book_scope(), cast()),
        artifact("volume_strategy", 1, book_scope(), volume()),
        artifact("chapter_list", 1, book_scope(), plan(1)),
    ];
    let b = assemble_director_context(
        kernel.as_ref(),
        &b_artifacts,
        "novel-b",
        "b1",
        24_000,
        8_000,
    )
    .await
    .unwrap();
    assert!(!b.prompt_text.contains("宗门以剑为尊"));
    assert!(!b.prompt_text.contains("第一章正式正文"));
    assert!(b.trace.sources.iter().any(
        |s| s.source_type == "world_facts" && s.exclusion_reason.as_deref() == Some("missing")
    ));
    assert!(
        b.trace
            .sources
            .iter()
            .any(|s| s.source_type == "character_facts"
                && s.exclusion_reason.as_deref() == Some("missing"))
    );
}
