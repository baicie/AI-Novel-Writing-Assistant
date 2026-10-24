use std::sync::Arc;

use novel_application::{NewChapter, NewNovel, NovelRepository, assemble_director_context};
use novel_domain::{ArtifactStatus, ArtifactView};
use novel_storage_sqlite::SqliteKernel;
use novel_workflow::book_scope;

fn artifact(ty: &str, payload: serde_json::Value) -> ArtifactView {
    ArtifactView {
        artifact_type: ty.into(),
        scope: book_scope().to_string(),
        version: 1,
        status: ArtifactStatus::Confirmed,
        protected_user_content: false,
        payload_json: payload.to_string(),
    }
}

#[tokio::test]
async fn story_memory_respects_chapter_time_and_does_not_use_future_facts() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = Arc::new(
        SqliteKernel::open(&dir.path().join("memory.sqlite"))
            .await
            .unwrap(),
    );
    NovelRepository::create_novel(
        kernel.as_ref(),
        NewNovel {
            id: "n".into(),
            title: "三十年".into(),
            description: String::new(),
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    for order in 1..=30 {
        NovelRepository::create_chapter(
            kernel.as_ref(),
            NewChapter {
                id: format!("ch{order}"),
                novel_id: "n".into(),
                title: format!("第{order}章"),
                sort_order: order,
                volume_id: None,
                now_ms: order as i64,
            },
        )
        .await
        .unwrap();
    }
    sqlx::query("INSERT INTO character(id, novel_id, name, role, profile_json, created_at, updated_at) VALUES ('c1','n','林动','主角','{}',1,1)")
        .execute(kernel.pool()).await.unwrap();
    for (id, order, body, revision) in [
        ("s1", 1, "身份：外门弟子", "rev1"),
        ("s2", 15, "身份：掌门", "rev15"),
        ("s3", 25, "状态：死亡", "rev25"),
    ] {
        sqlx::query("INSERT INTO character_state(id, character_id, chapter_order, body, created_at, source_revision, status, updated_at) VALUES (?, 'c1', ?, ?, ?, ?, 'confirmed', ?)")
            .bind(id).bind(order).bind(body).bind(order as i64).bind(revision).bind(order as i64)
            .execute(kernel.pool()).await.unwrap();
    }
    for (id, order, body) in [("e1", 3, "事件：拜入山门"), ("e2", 22, "事件：宗门大战")]
    {
        sqlx::query("INSERT INTO timeline_event(id, novel_id, chapter_order, body, created_at, source_revision, status, updated_at) VALUES (?, 'n', ?, ?, ?, ?, 'confirmed', ?)")
            .bind(id).bind(order).bind(body).bind(order as i64).bind(format!("rev{order}")).bind(order as i64)
            .execute(kernel.pool()).await.unwrap();
    }
    sqlx::query("INSERT INTO plot_thread(id, novel_id, title, body, opened_chapter_order, resolved_chapter_order, status, source_revision, created_at, updated_at) VALUES ('p1','n','祖祠伏笔','祖祠里有一道未解封印',5,NULL,'open','rev5',5,5)")
        .execute(kernel.pool()).await.unwrap();
    let artifacts = vec![
        artifact(
            "story_macro",
            serde_json::json!({"expansion":{"expanded_premise":"成长"},"decomposition":{},"constraints":[],"issues":[]}),
        ),
        artifact(
            "character_cast",
            serde_json::json!({"members":[{"name":"林动","role":"主角","castRole":"protagonist","storyFunction":"推进"}],"relations":[]}),
        ),
        artifact(
            "volume_strategy",
            serde_json::json!({"volumes":[{"volumeOrder":1,"title":"卷一","roleLabel":"开局","coreReward":"入局","planningMode":"hard"}]}),
        ),
        artifact(
            "chapter_list",
            serde_json::json!({"chapters":[{"chapterOrder":20,"title":"第20章","volumeTitle":"卷一","summary":"宗门大战前夜","characterNames":["林动"]}]}),
        ),
    ];
    let at20 = assemble_director_context(kernel.as_ref(), &artifacts, "n", "ch20", 24_000, 8_000)
        .await
        .unwrap();
    assert!(at20.prompt_text.contains("身份：掌门"));
    assert!(!at20.prompt_text.contains("身份：外门弟子"));
    assert!(!at20.prompt_text.contains("状态：死亡"));
    assert!(at20.prompt_text.contains("拜入山门"));
    assert!(!at20.prompt_text.contains("事件：宗门大战"));
    assert!(at20.prompt_text.contains("祖祠里有一道未解封印"));

    let facts30 = NovelRepository::list_story_facts(kernel.as_ref(), "n", 30)
        .await
        .unwrap();
    assert!(facts30.iter().any(|fact| fact.body.contains("状态：死亡")));
    assert!(
        facts30
            .iter()
            .any(|fact| fact.body.contains("事件：宗门大战"))
    );
    assert!(facts30.iter().any(|fact| fact.fact_type == "plot_thread"));
    println!(
        "P13 story-memory: at20=掌门/no-future at30=death+battle facts={} sources=3",
        facts30.len()
    );
}
