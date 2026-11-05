use novel_application::{
    CreatePlotThreadInput, NewCausalityEdge, NewChapter, NewNovel, NovelRepository, PlotRepository,
    PlotTransition, PortError,
};
use novel_storage_sqlite::SqliteKernel;

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
    sqlx::query(
        "INSERT INTO chapter_revision(id, chapter_id, version, origin, content, content_sha256, created_at)
         VALUES (?, ?, 1, 'user', ?, 'hash', 1)",
    )
    .bind(format!("rev-{id}"))
    .bind(id)
    .bind(content)
    .execute(kernel.pool())
    .await
    .unwrap();
    sqlx::query("UPDATE chapter SET official_revision_id = ? WHERE id = ?")
        .bind(format!("rev-{id}"))
        .bind(id)
        .execute(kernel.pool())
        .await
        .unwrap();
}

async fn setup() -> (tempfile::TempDir, SqliteKernel) {
    let dir = tempfile::tempdir().unwrap();
    let kernel = SqliteKernel::open(&dir.path().join("plot.sqlite"))
        .await
        .unwrap();
    NovelRepository::create_novel(
        &kernel,
        NewNovel {
            id: "n1".into(),
            title: "伏笔".into(),
            description: String::new(),
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    for (id, order, content) in [
        ("c0", 0, "第零章引子。"),
        ("c1", 1, "第一章埋下玉佩。"),
        ("c5", 5, "第五章玉佩发光。"),
        ("c10", 10, "第十章玉佩碎裂。"),
        ("c11", 11, "第十一章重新拾起碎片。"),
        ("c12", 12, "第十二章一切平静。"),
    ] {
        official_chapter(&kernel, id, order, content).await;
    }
    (dir, kernel)
}

fn create_input() -> CreatePlotThreadInput {
    CreatePlotThreadInput {
        id: "plot-1".into(),
        novel_id: "n1".into(),
        title: "玉佩".into(),
        body: "祖传玉佩里有一道裂痕".into(),
        chapter_id: "c1".into(),
        source_revision: 1,
        evidence: "第一章埋下玉佩。".into(),
        command_id: "create-1".into(),
        request_hash: "create-hash".into(),
        now_ms: 10,
    }
}

fn transition(
    action: &str,
    chapter: &str,
    evidence: &str,
    version: i64,
    command: &str,
) -> PlotTransition {
    PlotTransition {
        plot_id: "plot-1".into(),
        novel_id: "n1".into(),
        expected_version: version,
        action: action.into(),
        chapter_id: chapter.into(),
        source_revision: 1,
        evidence: evidence.into(),
        command_id: command.into(),
        request_hash: format!("hash-{command}"),
        now_ms: 100 + version,
    }
}

#[tokio::test]
async fn plot_lifecycle_is_as_of_and_versioned() {
    let (_dir, kernel) = setup().await;
    let created = kernel.create_plot_thread(create_input()).await.unwrap();
    assert_eq!(created.status, "open");
    assert_eq!(created.version, 1);

    let developed = kernel
        .transition_plot_thread(transition(
            "develop",
            "c5",
            "第五章玉佩发光。",
            1,
            "develop-1",
        ))
        .await
        .unwrap();
    assert_eq!(developed.status, "developing");
    assert_eq!(developed.last_development_chapter_order, Some(5));
    assert_eq!(developed.version, 2);

    // 重复同一条命令必须幂等，不追加事件、不改版本。
    let replay = kernel
        .transition_plot_thread(transition(
            "develop",
            "c5",
            "第五章玉佩发光。",
            1,
            "develop-1",
        ))
        .await
        .unwrap();
    assert_eq!(replay.version, 2);

    // 过期版本必须拒绝。
    let stale = kernel
        .transition_plot_thread(transition(
            "payoff",
            "c10",
            "第十章玉佩碎裂。",
            1,
            "payoff-stale",
        ))
        .await;
    assert!(matches!(stale, Err(PortError::Conflict(_))));

    let payoff = kernel
        .transition_plot_thread(transition(
            "payoff",
            "c10",
            "第十章玉佩碎裂。",
            2,
            "payoff-1",
        ))
        .await
        .unwrap();
    assert_eq!(payoff.status, "resolved");
    assert_eq!(payoff.resolved_chapter_order, Some(10));
    assert_eq!(payoff.version, 3);

    // As-Of：第 3 章埋设、第 6 章推进、第 11 章回收。
    let at3 = kernel.list_plot_threads("n1", 3).await.unwrap();
    assert_eq!(at3[0].status, "open");
    assert_eq!(at3[0].resolved_chapter_order, None);
    let at6 = kernel.list_plot_threads("n1", 6).await.unwrap();
    assert_eq!(at6[0].status, "developing");
    assert_eq!(at6[0].last_development_chapter_order, Some(5));
    let at9 = kernel.list_plot_threads("n1", 9).await.unwrap();
    assert_eq!(at9[0].status, "developing");
    let at11 = kernel.list_plot_threads("n1", 11).await.unwrap();
    assert_eq!(at11[0].status, "resolved");
    assert_eq!(at11[0].resolved_chapter_order, Some(10));

    let events = kernel.list_plot_events("n1", 11, 20).await.unwrap();
    assert_eq!(events.len(), 3, "setup + develop + payoff");
    assert_eq!(events.last().unwrap().kind, "setup");
}

#[tokio::test]
async fn plot_evidence_and_source_order_are_validated() {
    let (_dir, kernel) = setup().await;
    kernel.create_plot_thread(create_input()).await.unwrap();

    let fake = kernel
        .transition_plot_thread(transition(
            "develop",
            "c5",
            "这句话不在正文里。",
            1,
            "develop-fake",
        ))
        .await;
    assert!(matches!(fake, Err(PortError::Conflict(_))));

    // 推进章节不能早于埋设章节。
    let early = kernel
        .transition_plot_thread(transition(
            "develop",
            "c0",
            "第零章引子。",
            1,
            "develop-early",
        ))
        .await;
    assert!(matches!(early, Err(PortError::Conflict(_))));

    // 埋设来源章节被改写后，伏笔进入 stale，必须用新证据恢复。
    NovelRepository::save_user_chapter(
        &kernel,
        novel_application::SaveChapterRecord {
            novel_id: "n1".into(),
            chapter_id: "c1".into(),
            expected_version: 1,
            title: "第1章".into(),
            content: "第一章已经改写。".into(),
            revision_id: "c1-rev2".into(),
            artifact_id: "c1-art2".into(),
            content_sha256: "hash2".into(),
            now_ms: 200,
        },
    )
    .await
    .unwrap();
    let stale = kernel.list_plot_threads("n1", 6).await.unwrap();
    assert_eq!(stale[0].status, "stale");

    let recovered = kernel
        .transition_plot_thread(transition(
            "recover",
            "c5",
            "第五章玉佩发光。",
            1,
            "recover-1",
        ))
        .await
        .unwrap();
    assert_eq!(recovered.status, "developing");
}

#[tokio::test]
async fn causality_rejects_missing_entities_self_reference_and_cycles() {
    let (_dir, kernel) = setup().await;
    for (id, order, body) in [
        ("e1", 1, "事件一：得到玉佩"),
        ("e2", 2, "事件二：被追捕"),
        ("e3", 3, "事件三：逃入山门"),
    ] {
        sqlx::query(
            "INSERT INTO timeline_event
               (id, novel_id, chapter_order, body, created_at, source_revision, status, updated_at, source_chapter_id)
             VALUES (?, 'n1', ?, ?, ?, '1', 'confirmed', ?, ?)",
        )
        .bind(id)
        .bind(order)
        .bind(body)
        .bind(order)
        .bind(order)
        .bind(format!("c{order}"))
        .execute(kernel.pool())
        .await
        .unwrap();
    }
    let edge = |id: &str, from: &str, to: &str| NewCausalityEdge {
        id: id.into(),
        novel_id: "n1".into(),
        from_kind: "timeline_event".into(),
        from_id: from.into(),
        to_kind: "timeline_event".into(),
        to_id: to.into(),
        relation: "导致".into(),
        chapter_id: "c1".into(),
        source_revision: 1,
        evidence: "第一章埋下玉佩。".into(),
        now_ms: 10,
    };
    kernel
        .create_causality_edge(edge("edge-1", "e1", "e2"))
        .await
        .unwrap();
    kernel
        .create_causality_edge(edge("edge-2", "e2", "e3"))
        .await
        .unwrap();
    let cycle = kernel
        .create_causality_edge(edge("edge-3", "e3", "e1"))
        .await;
    assert!(matches!(cycle, Err(PortError::Conflict(_))));
    let self_edge = kernel
        .create_causality_edge(edge("edge-4", "e1", "e1"))
        .await;
    assert!(matches!(self_edge, Err(PortError::Conflict(_))));
    let missing = kernel
        .create_causality_edge(edge("edge-5", "e1", "missing"))
        .await;
    assert!(matches!(missing, Err(PortError::Conflict(_))));
    let edges = kernel.list_causality_edges("n1").await.unwrap();
    assert_eq!(edges.len(), 2);
    assert!(edges.iter().all(|edge| edge.source_valid));

    // 来源章节改写后，边保留但不再可靠。
    NovelRepository::save_user_chapter(
        &kernel,
        novel_application::SaveChapterRecord {
            novel_id: "n1".into(),
            chapter_id: "c1".into(),
            expected_version: 1,
            title: "第1章".into(),
            content: "第一章已经改写。".into(),
            revision_id: "c1-rev2".into(),
            artifact_id: "c1-art2".into(),
            content_sha256: "hash2".into(),
            now_ms: 300,
        },
    )
    .await
    .unwrap();
    let edges = kernel.list_causality_edges("n1").await.unwrap();
    assert!(edges.iter().all(|edge| !edge.source_valid));
}
