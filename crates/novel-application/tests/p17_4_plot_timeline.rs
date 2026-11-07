use std::sync::Arc;

use novel_application::{
    CreatePlotThreadInput, NewChapter, NewNovel, NovelRepository, PlotRepository, PlotTransition,
    assemble_director_context,
};
use novel_domain::{ArtifactStatus, ArtifactView};
use novel_storage_sqlite::SqliteKernel;
use novel_workflow::book_scope;

async fn official_chapter(kernel: &SqliteKernel, id: &str, order: i32, content: &str) {
    NovelRepository::create_chapter(
        kernel,
        NewChapter {
            id: id.into(),
            novel_id: "n1".into(),
            title: format!("第{order}章"),
            sort_order: order,
            volume_id: None,
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    NovelRepository::save_user_chapter(
        kernel,
        novel_application::SaveChapterRecord {
            novel_id: "n1".into(),
            chapter_id: id.into(),
            expected_version: 0,
            title: format!("第{order}章"),
            content: content.into(),
            revision_id: format!("rev-{id}"),
            artifact_id: format!("art-{id}"),
            content_sha256: format!("hash-{id}"),
            now_ms: order as i64,
        },
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn story_facts_and_continuity_use_plot_lifecycle_as_of() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = Arc::new(
        SqliteKernel::open(&dir.path().join("plot-context.sqlite"))
            .await
            .unwrap(),
    );
    NovelRepository::create_novel(
        kernel.as_ref(),
        NewNovel {
            id: "n1".into(),
            title: "伏笔上下文".into(),
            description: String::new(),
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    sqlx::query("INSERT INTO character(id, novel_id, name, role, profile_json, created_at, updated_at) VALUES ('lin','n1','林动','主角','{}',1,1)")
        .execute(kernel.pool())
        .await
        .unwrap();
    for (id, order, content) in [
        ("c1", 1, "第一章埋下玉佩。"),
        ("c5", 5, "第五章玉佩发光。"),
        ("c10", 10, "第十章玉佩碎裂。"),
        ("c11", 11, "第十一章一切结束。"),
    ] {
        official_chapter(kernel.as_ref(), id, order, content).await;
    }

    kernel
        .create_plot_thread(CreatePlotThreadInput {
            id: "plot-1".into(),
            novel_id: "n1".into(),
            title: "玉佩".into(),
            body: "祖传玉佩里有一道裂痕".into(),
            chapter_id: "c1".into(),
            source_revision: 1,
            evidence: "第一章埋下玉佩。".into(),
            command_id: "create-1".into(),
            request_hash: "hash-create".into(),
            now_ms: 10,
        })
        .await
        .unwrap();
    kernel
        .transition_plot_thread(PlotTransition {
            plot_id: "plot-1".into(),
            novel_id: "n1".into(),
            expected_version: 1,
            action: "develop".into(),
            chapter_id: "c5".into(),
            source_revision: 1,
            evidence: "第五章玉佩发光。".into(),
            command_id: "develop-1".into(),
            request_hash: "hash-develop".into(),
            now_ms: 20,
        })
        .await
        .unwrap();
    kernel
        .transition_plot_thread(PlotTransition {
            plot_id: "plot-1".into(),
            novel_id: "n1".into(),
            expected_version: 2,
            action: "payoff".into(),
            chapter_id: "c10".into(),
            source_revision: 1,
            evidence: "第十章玉佩碎裂。".into(),
            command_id: "payoff-1".into(),
            request_hash: "hash-payoff".into(),
            now_ms: 30,
        })
        .await
        .unwrap();

    let at6 = NovelRepository::list_story_facts(kernel.as_ref(), "n1", 6)
        .await
        .unwrap();
    let plot = at6
        .iter()
        .find(|fact| fact.id == "plot-1")
        .expect("第 6 章应看到埋设中的伏笔");
    assert_eq!(plot.status, "developing");
    assert!(!plot.body.contains("碎裂"), "不能提前泄露回收内容");
    assert_eq!(plot.source_chapter_id.as_deref(), Some("c5"));

    let at9 = NovelRepository::list_story_facts(kernel.as_ref(), "n1", 9)
        .await
        .unwrap();
    assert_eq!(
        at9.iter().find(|fact| fact.id == "plot-1").unwrap().status,
        "developing"
    );
    let at11 = NovelRepository::list_story_facts(kernel.as_ref(), "n1", 11)
        .await
        .unwrap();
    let resolved = at11
        .iter()
        .find(|fact| fact.id == "plot-1")
        .expect("第 11 章应看到已回收的伏笔");
    assert_eq!(resolved.status, "resolved");
    assert_eq!(resolved.chapter_order, 10);
    assert_eq!(resolved.evidence_quote.as_deref(), Some("第十章玉佩碎裂。"));
}

fn artifact(ty: &str, payload: serde_json::Value) -> ArtifactView {
    ArtifactView {
        artifact_type: ty.into(),
        scope: book_scope().to_string(),
        version: 1,
        run_id: None,
        status: ArtifactStatus::Confirmed,
        protected_user_content: false,
        payload_json: payload.to_string(),
    }
}

#[tokio::test]
async fn trusted_context_excludes_unverifiable_and_stale_plots() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = Arc::new(
        SqliteKernel::open(&dir.path().join("plot-context-trust.sqlite"))
            .await
            .unwrap(),
    );
    NovelRepository::create_novel(
        kernel.as_ref(),
        NewNovel {
            id: "n1".into(),
            title: "可信上下文".into(),
            description: String::new(),
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    for (id, order, content) in [
        ("c1", 1, "第一章埋下玉佩。"),
        ("c5", 5, "第五章玉佩发光。"),
        ("c10", 10, "第十章玉佩碎裂。"),
        ("c11", 11, "第十一章一切结束。"),
    ] {
        official_chapter(kernel.as_ref(), id, order, content).await;
    }
    // 有来源与证据的旧伏笔：可以进入可信上下文。
    sqlx::query(
        "INSERT INTO plot_thread
           (id, novel_id, title, body, opened_chapter_order, resolved_chapter_order, status,
            source_revision, evidence_json, created_at, updated_at, source_chapter_id)
         VALUES ('legacy-valid', 'n1', '有效旧伏笔', '有效旧伏笔：祖祠封印', 1, NULL, 'open',
                 '1', '第一章埋下玉佩。', 1, 1, 'c1')",
    )
    .execute(kernel.pool())
    .await
    .unwrap();
    // 没有证据的旧伏笔：保留数据，但不能进入可信上下文。
    sqlx::query(
        "INSERT INTO plot_thread
           (id, novel_id, title, body, opened_chapter_order, resolved_chapter_order, status,
            source_revision, evidence_json, created_at, updated_at, source_chapter_id)
         VALUES ('legacy-no-evidence', 'n1', '无证据旧伏笔', '无证据旧伏笔：不能进入上下文', 1, NULL,
                 'open', '1', NULL, 1, 1, 'c1')",
    )
    .execute(kernel.pool())
    .await
    .unwrap();

    // 来源被改写的伏笔：状态变为 stale，必须退出可信上下文。
    kernel
        .create_plot_thread(CreatePlotThreadInput {
            id: "plot-stale".into(),
            novel_id: "n1".into(),
            title: "失效伏笔".into(),
            body: "失效伏笔：来源已改写".into(),
            chapter_id: "c5".into(),
            source_revision: 1,
            evidence: "第五章玉佩发光。".into(),
            command_id: "create-stale".into(),
            request_hash: "create-stale-hash".into(),
            now_ms: 10,
        })
        .await
        .unwrap();
    NovelRepository::save_user_chapter(
        kernel.as_ref(),
        novel_application::SaveChapterRecord {
            novel_id: "n1".into(),
            chapter_id: "c5".into(),
            expected_version: 1,
            title: "第5章".into(),
            content: "第五章已经改写。".into(),
            revision_id: "c5-rev2".into(),
            artifact_id: "c5-art2".into(),
            content_sha256: "hash2".into(),
            now_ms: 200,
        },
    )
    .await
    .unwrap();

    let facts = NovelRepository::list_story_facts(kernel.as_ref(), "n1", 11)
        .await
        .unwrap();
    assert!(
        facts.iter().any(|fact| fact.id == "legacy-valid"),
        "有来源与证据的旧伏笔应保留"
    );
    assert!(
        !facts.iter().any(|fact| fact.id == "legacy-no-evidence"),
        "无证据旧伏笔不能进入 list_story_facts"
    );
    assert!(
        !facts.iter().any(|fact| fact.id == "plot-stale"),
        "来源失效伏笔不能进入 list_story_facts"
    );

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
            serde_json::json!({"chapters":[{"chapterOrder":11,"title":"第11章","volumeTitle":"卷一","summary":"收束","characterNames":["林动"]}]}),
        ),
    ];
    let context =
        assemble_director_context(kernel.as_ref(), &artifacts, "n1", "c11", 24_000, 8_000)
            .await
            .unwrap();
    assert!(context.prompt_text.contains("有效旧伏笔：祖祠封印"));
    assert!(!context.prompt_text.contains("无证据旧伏笔：不能进入上下文"));
    assert!(!context.prompt_text.contains("失效伏笔：来源已改写"));
}
