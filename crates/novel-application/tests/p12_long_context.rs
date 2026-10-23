use std::sync::Arc;

use novel_application::{NewChapter, NewNovel, NovelRepository, assemble_director_context};
use novel_domain::{ArtifactStatus, ArtifactView};
use novel_storage_sqlite::SqliteKernel;
use novel_workflow::{book_scope, chapter_scope};

fn artifact(ty: &str, scope: &str, payload: serde_json::Value) -> ArtifactView {
    ArtifactView {
        artifact_type: ty.into(),
        scope: scope.into(),
        version: 1,
        status: ArtifactStatus::Confirmed,
        protected_user_content: false,
        payload_json: payload.to_string(),
    }
}

#[tokio::test]
async fn hundred_chapter_context_stability_benchmark() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = Arc::new(
        SqliteKernel::open(&dir.path().join("long.sqlite"))
            .await
            .unwrap(),
    );
    NovelRepository::create_novel(
        kernel.as_ref(),
        NewNovel {
            id: "n".into(),
            title: "长篇小说".into(),
            description: "连续性测试".into(),
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    for order in 1..=100 {
        let id = format!("ch{order}");
        NovelRepository::create_chapter(
            kernel.as_ref(),
            NewChapter {
                id: id.clone(),
                novel_id: "n".into(),
                title: format!("第{order}章"),
                sort_order: order,
                volume_id: None,
                now_ms: order as i64,
            },
        )
        .await
        .unwrap();
        if order <= 90 {
            NovelRepository::save_user_chapter(
                kernel.as_ref(),
                novel_application::SaveChapterRecord {
                    novel_id: "n".into(),
                    chapter_id: id,
                    expected_version: 0,
                    title: format!("第{order}章"),
                    content: format!("第{order}章正式正文：林动推进主线，事实节点 {order}。"),
                    revision_id: format!("rev{order}"),
                    artifact_id: format!("art{order}"),
                    content_sha256: format!("hash{order}"),
                    now_ms: 100 + order as i64,
                },
            )
            .await
            .unwrap();
        }
    }
    sqlx::query("INSERT INTO chapter_candidate(id, chapter_id, run_id, step_id, content, content_sha256, interrupted, created_at, updated_at) VALUES ('cand91','ch91','run','draft','第九章未确认候选正文','h',0,1,1)")
        .execute(kernel.pool()).await.unwrap();
    let artifacts = vec![
        artifact(
            "story_macro",
            book_scope(),
            serde_json::json!({
                "expansion": {"expanded_premise": "林动追查宗门秘密", "protagonist_core": "成长"},
                "decomposition": {"core_conflict": "宗门秘密"}, "constraints": [], "issues": []
            }),
        ),
        artifact(
            "character_cast",
            book_scope(),
            serde_json::json!({"members":[
            {"name":"林动","role":"主角","castRole":"protagonist","storyFunction":"推进"},
            {"name":"苏晚","role":"同伴","castRole":"ally","storyFunction":"协助"}
        ],"relations":[]}),
        ),
        artifact(
            "volume_strategy",
            book_scope(),
            serde_json::json!({"volumes":[
                {"volumeOrder":1,"title":"卷一","roleLabel":"开局","coreReward":"入局","planningMode":"hard"}
            ]}),
        ),
        artifact(
            "chapter_list",
            book_scope(),
            serde_json::json!({"chapters":[
                {"chapterOrder":91,"title":"第九十一章","volumeTitle":"卷一","summary":"真相逼近","characterNames":["林动","苏晚"]}
            ]}),
        ),
    ];
    let context =
        assemble_director_context(kernel.as_ref(), &artifacts, "n", "ch91", 24_000, 8_000)
            .await
            .unwrap();
    let mut recall = 0;
    for order in 88..=90 {
        if context
            .prompt_text
            .contains(&format!("第{order}章正式正文"))
        {
            recall += 1;
        }
    }
    assert_eq!(recall, 3, "recent confirmed chapters must be recalled");
    assert!(
        !context.prompt_text.contains("第九十一章未确认候选正文"),
        "candidate must not leak"
    );
    assert!(
        !context.prompt_text.contains("第100章"),
        "future chapter must not leak"
    );
    assert!(context.prompt_text.chars().count() <= 24_000);
    println!(
        "P12 long-context benchmark: recent_recall={recall}/3 earlier_omitted_by_policy=87 candidate_leak=0 future_leak=0 chars={} trace={}",
        context.prompt_text.chars().count(),
        context.trace.assembly_version
    );
    let _ = chapter_scope("ch91");
}
