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

    // 同一 commandId 但请求不同，必须明确冲突，不能当成幂等重放。
    let mut mismatched = transition("develop", "c5", "第五章玉佩发光。", 1, "develop-1");
    mismatched.request_hash = "different-request".into();
    let conflict = kernel.transition_plot_thread(mismatched).await;
    assert!(matches!(conflict, Err(PortError::Conflict(_))));

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

    // 来源版本不匹配时必须零副作用。
    let mut wrong_revision = transition("develop", "c5", "第五章玉佩发光。", 1, "develop-rev");
    wrong_revision.source_revision = 9;
    let wrong_revision = kernel.transition_plot_thread(wrong_revision).await;
    assert!(matches!(wrong_revision, Err(PortError::Conflict(_))));
    let (status, version): (String, i64) =
        sqlx::query_as("SELECT status, version FROM plot_thread WHERE id = 'plot-1'")
            .fetch_one(kernel.pool())
            .await
            .unwrap();
    assert_eq!((status.as_str(), version), ("open", 1));

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
            2,
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

async fn insert_legacy_thread(
    kernel: &SqliteKernel,
    id: &str,
    status: &str,
    resolved_chapter_order: Option<i32>,
    evidence: Option<&str>,
    source_revision: &str,
) {
    sqlx::query(
        "INSERT INTO plot_thread
           (id, novel_id, title, body, opened_chapter_order, resolved_chapter_order, status,
            source_revision, evidence_json, created_at, updated_at, source_chapter_id)
         VALUES (?, 'n1', '旧伏笔', '旧伏笔正文', 1, ?, ?, ?, ?, 1, 1, 'c1')",
    )
    .bind(id)
    .bind(resolved_chapter_order)
    .bind(status)
    .bind(source_revision)
    .bind(evidence)
    .execute(kernel.pool())
    .await
    .unwrap();
}

#[tokio::test]
async fn legacy_resolved_thread_does_not_leak_into_earlier_chapters() {
    let (_dir, kernel) = setup().await;
    insert_legacy_thread(
        &kernel,
        "legacy-resolved",
        "resolved",
        Some(10),
        Some("第一章埋下玉佩。"),
        "1",
    )
    .await;

    let at5 = kernel.list_plot_threads("n1", 5).await.unwrap();
    assert_eq!(at5[0].status, "open", "第 5 章不能提前看到第 10 章回收");
    assert_eq!(at5[0].resolved_chapter_order, None);
    let facts5 = NovelRepository::list_story_facts(&kernel, "n1", 5)
        .await
        .unwrap();
    let plot5 = facts5
        .iter()
        .find(|fact| fact.id == "legacy-resolved")
        .expect("旧伏笔在第 5 章仍应可读");
    assert_eq!(plot5.status, "open");
    assert_eq!(plot5.chapter_order, 1);

    let at11 = kernel.list_plot_threads("n1", 11).await.unwrap();
    assert_eq!(at11[0].status, "resolved");
    assert_eq!(at11[0].resolved_chapter_order, Some(10));
}

#[tokio::test]
async fn invalid_legacy_thread_can_recover_with_new_evidence() {
    let (_dir, kernel) = setup().await;
    // 旧写入路径没有证据，但生命周期状态是 resolved：界面显示 stale 时，作者必须能
    // 用当前章节的新证据恢复，而不是被存储里的 resolved 卡死。
    insert_legacy_thread(
        &kernel,
        "legacy-no-evidence",
        "resolved",
        Some(10),
        None,
        "memory:1",
    )
    .await;
    let listed = kernel.list_plot_threads("n1", 11).await.unwrap();
    let thread = listed
        .iter()
        .find(|item| item.id == "legacy-no-evidence")
        .unwrap();
    assert_eq!(thread.status, "stale");
    assert_eq!(
        thread.lifecycle_status, "resolved",
        "展示为 stale 时仍要保留历史生命周期"
    );
    assert!(!thread.source_valid);
    assert_eq!(thread.version, 0);

    let recovered = kernel
        .transition_plot_thread(PlotTransition {
            plot_id: "legacy-no-evidence".into(),
            novel_id: "n1".into(),
            expected_version: 0,
            action: "recover".into(),
            chapter_id: "c5".into(),
            source_revision: 1,
            evidence: "第五章玉佩发光。".into(),
            command_id: "legacy-recover".into(),
            request_hash: "legacy-recover-hash".into(),
            now_ms: 500,
        })
        .await
        .unwrap();
    assert_eq!(recovered.status, "developing");
    assert_eq!(recovered.resolved_chapter_order, None);
    assert_eq!(recovered.version, 1);

    // 旧 open 伏笔没有证据时同样可以用新证据推进。
    insert_legacy_thread(&kernel, "legacy-open", "open", None, None, "memory:1").await;
    let developed = kernel
        .transition_plot_thread(PlotTransition {
            plot_id: "legacy-open".into(),
            novel_id: "n1".into(),
            expected_version: 0,
            action: "develop".into(),
            chapter_id: "c5".into(),
            source_revision: 1,
            evidence: "第五章玉佩发光。".into(),
            command_id: "legacy-develop".into(),
            request_hash: "legacy-develop-hash".into(),
            now_ms: 501,
        })
        .await
        .unwrap();
    assert_eq!(developed.status, "developing");
}

#[tokio::test]
async fn same_chapter_events_fold_in_insertion_order() {
    let (_dir, kernel) = setup().await;
    kernel.create_plot_thread(create_input()).await.unwrap();
    kernel
        .transition_plot_thread(PlotTransition {
            plot_id: "plot-1".into(),
            novel_id: "n1".into(),
            expected_version: 1,
            action: "develop".into(),
            chapter_id: "c5".into(),
            source_revision: 1,
            evidence: "第五章玉佩发光。".into(),
            command_id: "zz-develop".into(),
            request_hash: "zz-develop-hash".into(),
            now_ms: 100,
        })
        .await
        .unwrap();
    // 同一章、同一毫秒内连续两次动作：As-Of 必须按真实写入顺序折叠，
    // 不能按 commandId 的字典序猜测先后。
    kernel
        .transition_plot_thread(PlotTransition {
            plot_id: "plot-1".into(),
            novel_id: "n1".into(),
            expected_version: 2,
            action: "defer".into(),
            chapter_id: "c5".into(),
            source_revision: 1,
            evidence: "第五章玉佩发光。".into(),
            command_id: "aa-defer".into(),
            request_hash: "aa-defer-hash".into(),
            now_ms: 100,
        })
        .await
        .unwrap();
    let listed = kernel.list_plot_threads("n1", 5).await.unwrap();
    assert_eq!(listed[0].status, "deferred");
}

#[tokio::test]
async fn story_facts_reorder_re_evaluates_as_of() {
    let (_dir, kernel) = setup().await;
    sqlx::query(
        "INSERT INTO character(id, novel_id, name, role, profile_json, created_at, updated_at)
         VALUES ('lin', 'n1', '林动', '主角', '{}', 1, 1)",
    )
    .execute(kernel.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO character_state
           (id, character_id, chapter_order, body, created_at, source_revision, status,
            updated_at, source_chapter_id)
         VALUES ('state-1', 'lin', 5, '林动在第五章抵达山门。', 1, '1', 'confirmed', 1, 'c5')",
    )
    .execute(kernel.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO timeline_event
           (id, novel_id, chapter_order, body, created_at, source_revision, status, updated_at, source_chapter_id)
         VALUES ('event-1', 'n1', 5, '第五章事件：山门开启。', 1, '1', 'confirmed', 1, 'c5')",
    )
    .execute(kernel.pool())
    .await
    .unwrap();

    let at6 = NovelRepository::list_story_facts(&kernel, "n1", 6)
        .await
        .unwrap();
    assert!(at6.iter().any(|fact| fact.id == "state-1"));
    assert!(at6.iter().any(|fact| fact.id == "event-1"));

    sqlx::query("UPDATE chapter SET sort_order = 20 WHERE id = 'c5'")
        .execute(kernel.pool())
        .await
        .unwrap();
    let at6 = NovelRepository::list_story_facts(&kernel, "n1", 6)
        .await
        .unwrap();
    assert!(
        !at6.iter()
            .any(|fact| fact.id == "state-1" || fact.id == "event-1"),
        "章节移到第 20 章后，第 6 章不能看到这条人物状态或事件"
    );
    let at21 = NovelRepository::list_story_facts(&kernel, "n1", 21)
        .await
        .unwrap();
    assert_eq!(
        at21.iter()
            .find(|fact| fact.id == "state-1")
            .unwrap()
            .chapter_order,
        20
    );
    assert_eq!(
        at21.iter()
            .find(|fact| fact.id == "event-1")
            .unwrap()
            .chapter_order,
        20
    );
}

#[tokio::test]
async fn action_cannot_be_inserted_before_the_latest_event() {
    let (_dir, kernel) = setup().await;
    kernel.create_plot_thread(create_input()).await.unwrap();
    kernel
        .transition_plot_thread(transition(
            "develop",
            "c10",
            "第十章玉佩碎裂。",
            1,
            "develop-late",
        ))
        .await
        .unwrap();

    // 在第 5 章补一个“回收”会排在第 10 章推进之前，未来章节仍会折叠出旧状态，
    // 造成数据库当前状态与 As-Of 结果不一致，必须拒绝。
    let early = kernel
        .transition_plot_thread(transition(
            "payoff",
            "c5",
            "第五章玉佩发光。",
            2,
            "payoff-early",
        ))
        .await;
    assert!(matches!(early, Err(PortError::Conflict(_))));
    let threads = kernel.list_plot_threads("n1", 5).await.unwrap();
    assert_eq!(threads[0].status, "open");
    assert_eq!(
        threads[0].latest_event_chapter_order,
        Some(10),
        "第 5 章必须知道最新动作在第 10 章，界面才能隐藏无效动作"
    );
    let threads = kernel.list_plot_threads("n1", 11).await.unwrap();
    assert_eq!(threads[0].status, "developing");
    let events = kernel.list_plot_events("n1", 11, 20).await.unwrap();
    assert_eq!(events.len(), 2);
}

#[tokio::test]
async fn chapter_reorder_re_evaluates_plot_as_of() {
    let (_dir, kernel) = setup().await;
    kernel.create_plot_thread(create_input()).await.unwrap();
    kernel
        .transition_plot_thread(transition(
            "develop",
            "c5",
            "第五章玉佩发光。",
            1,
            "develop-1",
        ))
        .await
        .unwrap();

    // 第五章被移到第 20 章：第 6 章不应再看到这次推进。
    sqlx::query("UPDATE chapter SET sort_order = 20 WHERE id = 'c5'")
        .execute(kernel.pool())
        .await
        .unwrap();
    let at6 = kernel.list_plot_threads("n1", 6).await.unwrap();
    assert_eq!(at6[0].status, "open");
    assert_eq!(at6[0].last_development_chapter_order, None);
    let events6 = kernel.list_plot_events("n1", 6, 20).await.unwrap();
    assert!(events6.iter().all(|event| event.kind != "develop"));
    let facts6 = NovelRepository::list_story_facts(&kernel, "n1", 6)
        .await
        .unwrap();
    assert!(
        facts6
            .iter()
            .find(|fact| fact.id == "plot-1")
            .map(|fact| fact.status == "open")
            .unwrap_or(false)
    );

    // 埋设章被移到第 30 章：第 6 章的 As-Of 视图里这条伏笔还不存在。
    sqlx::query("UPDATE chapter SET sort_order = 30 WHERE id = 'c1'")
        .execute(kernel.pool())
        .await
        .unwrap();
    let at6 = kernel.list_plot_threads("n1", 6).await.unwrap();
    assert!(at6.is_empty(), "埋设章移到 30 后，第 6 章不能看到这条伏笔");
}

async fn seed_race_chapters(kernel: &SqliteKernel) {
    NovelRepository::create_novel(
        kernel,
        NewNovel {
            id: "n1".into(),
            title: "并发".into(),
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
    ] {
        official_chapter(kernel, id, order, content).await;
    }
}

#[tokio::test]
async fn concurrent_transitions_only_one_commits() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("plot-race.sqlite");
    let first = SqliteKernel::open(&path).await.unwrap();
    seed_race_chapters(&first).await;
    first.create_plot_thread(create_input()).await.unwrap();
    let second = SqliteKernel::open(&path).await.unwrap();

    let (left, right) = tokio::join!(
        first.transition_plot_thread(transition(
            "develop",
            "c5",
            "第五章玉佩发光。",
            1,
            "race-develop",
        )),
        second.transition_plot_thread(PlotTransition {
            plot_id: "plot-1".into(),
            novel_id: "n1".into(),
            expected_version: 1,
            action: "payoff".into(),
            chapter_id: "c10".into(),
            source_revision: 1,
            evidence: "第十章玉佩碎裂。".into(),
            command_id: "race-payoff".into(),
            request_hash: "race-payoff-hash".into(),
            now_ms: 201,
        })
    );
    assert_eq!(
        usize::from(left.is_ok()) + usize::from(right.is_ok()),
        1,
        "同一版本只能有一个转换成功"
    );
    let events = first.list_plot_events("n1", 20, 20).await.unwrap();
    assert_eq!(events.len(), 2, "setup + 一次成功转换");
    let threads = first.list_plot_threads("n1", 20).await.unwrap();
    assert_eq!(threads[0].version, 2);
}

#[tokio::test]
async fn legacy_upsert_does_not_override_lifecycle_state() {
    let (_dir, kernel) = setup().await;
    kernel.create_plot_thread(create_input()).await.unwrap();
    kernel
        .transition_plot_thread(transition(
            "develop",
            "c5",
            "第五章玉佩发光。",
            1,
            "develop-1",
        ))
        .await
        .unwrap();

    NovelRepository::upsert_plot_thread(
        &kernel,
        novel_application::NewPlotThread {
            id: "plot-1".into(),
            novel_id: "n1".into(),
            title: "玉佩（旧入口改名）".into(),
            body: "旧入口正文".into(),
            opened_chapter_order: 1,
            resolved_chapter_order: Some(10),
            status: "resolved".into(),
            source_chapter_id: "c1".into(),
            source_revision: "1".into(),
            now_ms: 400,
        },
    )
    .await
    .unwrap();

    let listed = kernel.list_plot_threads("n1", 6).await.unwrap();
    assert_eq!(
        listed[0].status, "developing",
        "旧入口不能改写已记录的生命周期"
    );
    assert_eq!(listed[0].title, "玉佩（旧入口改名）");
    assert_eq!(listed[0].version, 3, "旧入口更新也必须递增版本");
}

#[tokio::test]
async fn transition_rolls_back_when_event_write_fails() {
    let (_dir, kernel) = setup().await;
    kernel.create_plot_thread(create_input()).await.unwrap();
    sqlx::query(
        "INSERT INTO plot_thread_event
           (id, plot_id, novel_id, kind, chapter_id, chapter_order, source_revision, evidence,
            from_status, to_status, command_id, request_hash, created_at)
         VALUES ('transition-fail-cmd', 'plot-1', 'n1', 'develop', 'c5', 5, 1, '第五章玉佩发光。',
                 'open', 'developing', 'other-command', 'other-hash', 1)",
    )
    .execute(kernel.pool())
    .await
    .unwrap();

    let result = kernel
        .transition_plot_thread(transition(
            "develop",
            "c5",
            "第五章玉佩发光。",
            1,
            "fail-cmd",
        ))
        .await;
    assert!(result.is_err(), "重复事件 ID 必须让转换失败");
    // 直接读表，避免测试预置的 develop 事件干扰 As-Of 折叠结果。
    let (status, version): (String, i64) =
        sqlx::query_as("SELECT status, version FROM plot_thread WHERE id = 'plot-1'")
            .fetch_one(kernel.pool())
            .await
            .unwrap();
    assert_eq!(status, "open");
    assert_eq!(version, 1);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM plot_thread_event")
        .fetch_one(kernel.pool())
        .await
        .unwrap();
    assert_eq!(
        count, 2,
        "失败的事务不能留下转换事件（只有 setup 与预置事件）"
    );
}

#[tokio::test]
async fn plot_events_stay_visible_but_mark_invalid_sources() {
    let (_dir, kernel) = setup().await;
    kernel.create_plot_thread(create_input()).await.unwrap();
    kernel
        .transition_plot_thread(transition(
            "develop",
            "c5",
            "第五章玉佩发光。",
            1,
            "develop-1",
        ))
        .await
        .unwrap();
    NovelRepository::save_user_chapter(
        &kernel,
        novel_application::SaveChapterRecord {
            novel_id: "n1".into(),
            chapter_id: "c5".into(),
            expected_version: 1,
            title: "第5章".into(),
            content: "第五章已经改写。".into(),
            revision_id: "c5-rev2".into(),
            artifact_id: "c5-art2".into(),
            content_sha256: "hash2".into(),
            now_ms: 600,
        },
    )
    .await
    .unwrap();

    let events = kernel.list_plot_events("n1", 11, 20).await.unwrap();
    let setup_event = events.iter().find(|event| event.kind == "setup").unwrap();
    let develop_event = events.iter().find(|event| event.kind == "develop").unwrap();
    assert!(setup_event.source_valid);
    assert!(
        !develop_event.source_valid,
        "来源章节改写后事件仍保留，但必须标记失效"
    );
}

#[tokio::test]
async fn candidate_confirmation_does_not_reopen_finished_thread() {
    use novel_application::MemoryRepository;
    use sha2::{Digest, Sha256};

    let (_dir, kernel) = setup().await;
    let mut create = create_input();
    create.id = "memory-cand-1".into();
    create.command_id = "create-cand".into();
    kernel.create_plot_thread(create).await.unwrap();
    kernel
        .transition_plot_thread(PlotTransition {
            plot_id: "memory-cand-1".into(),
            novel_id: "n1".into(),
            expected_version: 1,
            action: "payoff".into(),
            chapter_id: "c10".into(),
            source_revision: 1,
            evidence: "第十章玉佩碎裂。".into(),
            command_id: "payoff-cand".into(),
            request_hash: "payoff-cand-hash".into(),
            now_ms: 30,
        })
        .await
        .unwrap();

    // 候选来自第五章：旧写路径不能在重确认时把作者已经确定的“第十章回收”
    // 改写成第五条证据。
    let content = "第五章玉佩发光。";
    let digest = hex::encode(Sha256::digest(content.as_bytes()));
    sqlx::query(
        "INSERT INTO memory_extraction_job
           (id, command_id, novel_id, chapter_id, chapter_version, source_content_hash,
            extraction_version, provider, model, endpoint, status, created_at, updated_at)
         VALUES ('job-1', 'job-command', 'n1', 'c5', 1, ?, 'v1', 'mock', 'mock', 'local',
                 'succeeded', 1, 1)",
    )
    .bind(&digest)
    .execute(kernel.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO memory_extraction_candidate
           (id, job_id, novel_id, chapter_id, chapter_version, source_content_hash, fact_type,
            truthfulness, entity_name, entity_id, value, evidence_quote, evidence_start,
            confidence, status, ambiguous, version, created_at, updated_at)
         VALUES ('cand-1', 'job-1', 'n1', 'c5', 1, ?, 'plot_thread', 'narrator_fact',
                 '玉佩', NULL, '玉佩另有秘密', ?, 0, 0.9, 'proposed', 0, 0, 1, 1)",
    )
    .bind(&digest)
    .bind(content)
    .execute(kernel.pool())
    .await
    .unwrap();

    kernel
        .confirm_memory_candidate("cand-1", 0, 700)
        .await
        .unwrap();

    let (status, version, source_chapter, evidence): (String, i64, Option<String>, Option<String>) =
        sqlx::query_as(
            "SELECT status, version, source_chapter_id, evidence_json
             FROM plot_thread WHERE id = 'memory-cand-1'",
        )
        .fetch_one(kernel.pool())
        .await
        .unwrap();
    assert_eq!(status, "resolved", "重新确认候选不能重开已回收的伏笔");
    assert_eq!(version, 2, "旧写路径不应改动生命周期版本");
    assert_eq!(source_chapter.as_deref(), Some("c10"));
    assert_eq!(evidence.as_deref(), Some("第十章玉佩碎裂。"));
    let events: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM plot_thread_event WHERE plot_id = 'memory-cand-1'",
    )
    .fetch_one(kernel.pool())
    .await
    .unwrap();
    assert_eq!(events, 2, "历史事件必须完整保留");
}

#[tokio::test]
async fn causality_is_scoped_to_one_novel() {
    let (_dir, kernel) = setup().await;
    NovelRepository::create_novel(
        &kernel,
        NewNovel {
            id: "n2".into(),
            title: "另一本".into(),
            description: String::new(),
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    for (id, novel, order, body) in [("e1", "n1", 1, "事件一"), ("e2", "n2", 1, "别书事件")]
    {
        sqlx::query(
            "INSERT INTO timeline_event
               (id, novel_id, chapter_order, body, created_at, source_revision, status, updated_at, source_chapter_id)
             VALUES (?, ?, ?, ?, 1, '1', 'confirmed', 1, 'c1')",
        )
        .bind(id)
        .bind(novel)
        .bind(order)
        .bind(body)
        .execute(kernel.pool())
        .await
        .unwrap();
    }
    let cross = kernel
        .create_causality_edge(NewCausalityEdge {
            id: "cross-edge".into(),
            novel_id: "n1".into(),
            from_kind: "timeline_event".into(),
            from_id: "e1".into(),
            to_kind: "timeline_event".into(),
            to_id: "e2".into(),
            relation: "导致".into(),
            chapter_id: "c1".into(),
            source_revision: 1,
            evidence: "第一章埋下玉佩。".into(),
            now_ms: 1,
        })
        .await;
    assert!(matches!(cross, Err(PortError::Conflict(_))));
}

#[tokio::test]
async fn memory_conflict_supersede_does_not_override_lifecycle_thread() {
    use novel_application::MemoryRepository;
    use sha2::{Digest, Sha256};

    let (_dir, kernel) = setup().await;
    let mut create = create_input();
    create.id = "memory-cand-1".into();
    create.command_id = "create-cand".into();
    kernel.create_plot_thread(create).await.unwrap();
    kernel
        .transition_plot_thread(PlotTransition {
            plot_id: "memory-cand-1".into(),
            novel_id: "n1".into(),
            expected_version: 1,
            action: "payoff".into(),
            chapter_id: "c10".into(),
            source_revision: 1,
            evidence: "第十章玉佩碎裂。".into(),
            command_id: "payoff-cand".into(),
            request_hash: "payoff-cand-hash".into(),
            now_ms: 30,
        })
        .await
        .unwrap();

    let content = "第五章玉佩发光。";
    let digest = hex::encode(Sha256::digest(content.as_bytes()));
    sqlx::query(
        "INSERT INTO memory_extraction_job
           (id, command_id, novel_id, chapter_id, chapter_version, source_content_hash,
            extraction_version, provider, model, endpoint, status, created_at, updated_at)
         VALUES ('job-1', 'job-command', 'n1', 'c5', 1, ?, 'v1', 'mock', 'mock', 'local',
                 'succeeded', 1, 1)",
    )
    .bind(&digest)
    .execute(kernel.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO memory_extraction_candidate
           (id, job_id, novel_id, chapter_id, chapter_version, source_content_hash, fact_type,
            truthfulness, entity_name, entity_id, value, evidence_quote, evidence_start,
            confidence, status, ambiguous, version, created_at, updated_at)
         VALUES ('cand-1', 'job-1', 'n1', 'c5', 1, ?, 'plot_thread', 'narrator_fact',
                 '玉佩', NULL, '玉佩另有秘密', ?, 0, 0.9, 'proposed', 0, 0, 1, 1)",
    )
    .bind(&digest)
    .bind(content)
    .execute(kernel.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO memory_conflict
           (id, novel_id, conflict_type, entity_key, chapter_id, chapter_order,
            candidate_ids_json, fact_ids_json, status, version, description, created_at, updated_at)
         VALUES ('conflict-1', 'n1', 'plot_thread', '玉佩', 'c5', 5,
                 '[\"cand-1\"]', '[\"memory-cand-1\"]', 'open', 0, '同一伏笔出现新证据', 1, 1)",
    )
    .execute(kernel.pool())
    .await
    .unwrap();

    kernel
        .resolve_memory_conflict(
            "conflict-1",
            0,
            "conflict-command",
            "conflict-hash",
            "supersede_with_evidence",
            Some("cand-1"),
            "采用第五章的新证据",
            "author",
            800,
        )
        .await
        .unwrap();

    let (status, source_chapter, evidence): (String, Option<String>, Option<String>) =
        sqlx::query_as(
            "SELECT status, source_chapter_id, evidence_json
             FROM plot_thread WHERE id = 'memory-cand-1'",
        )
        .fetch_one(kernel.pool())
        .await
        .unwrap();
    assert_eq!(status, "resolved", "冲突裁决不能覆盖有事件历史的生命周期");
    assert_eq!(source_chapter.as_deref(), Some("c10"));
    assert_eq!(evidence.as_deref(), Some("第十章玉佩碎裂。"));
    let events: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM plot_thread_event WHERE plot_id = 'memory-cand-1'",
    )
    .fetch_one(kernel.pool())
    .await
    .unwrap();
    assert_eq!(events, 2);
}

#[tokio::test]
async fn causality_rejects_cycles_across_entity_types() {
    let (_dir, kernel) = setup().await;
    kernel.create_plot_thread(create_input()).await.unwrap();
    sqlx::query(
        "INSERT INTO timeline_event
           (id, novel_id, chapter_order, body, created_at, source_revision, status, updated_at, source_chapter_id)
         VALUES ('mixed-event', 'n1', 1, '事件一', 1, '1', 'confirmed', 1, 'c1')",
    )
    .execute(kernel.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO structured_story_fact
           (id, novel_id, entity_id, fact_type, value_json, source_chapter_id, source_revision,
            evidence_quote, truthfulness, effective_chapter_order, status, created_at, updated_at)
         VALUES ('mixed-fact', 'n1', 'lin', 'location', '{\"location\":\"山门\"}', 'c1', 1,
                 '第一章埋下玉佩。', 'confirmed', 1, 'active', 1, 1)",
    )
    .execute(kernel.pool())
    .await
    .unwrap();
    let edge =
        |id: &str, from_kind: &str, from_id: &str, to_kind: &str, to_id: &str| NewCausalityEdge {
            id: id.into(),
            novel_id: "n1".into(),
            from_kind: from_kind.into(),
            from_id: from_id.into(),
            to_kind: to_kind.into(),
            to_id: to_id.into(),
            relation: "导致".into(),
            chapter_id: "c1".into(),
            source_revision: 1,
            evidence: "第一章埋下玉佩。".into(),
            now_ms: 1,
        };
    kernel
        .create_causality_edge(edge(
            "mixed-1",
            "timeline_event",
            "mixed-event",
            "plot_thread",
            "plot-1",
        ))
        .await
        .unwrap();
    kernel
        .create_causality_edge(edge(
            "mixed-2",
            "plot_thread",
            "plot-1",
            "structured_fact",
            "mixed-fact",
        ))
        .await
        .unwrap();
    let cycle = kernel
        .create_causality_edge(edge(
            "mixed-3",
            "structured_fact",
            "mixed-fact",
            "timeline_event",
            "mixed-event",
        ))
        .await;
    assert!(matches!(cycle, Err(PortError::Conflict(_))));
    assert_eq!(kernel.list_causality_edges("n1").await.unwrap().len(), 2);
}

#[tokio::test]
async fn causality_rejects_new_edges_to_stale_entities() {
    let (_dir, kernel) = setup().await;
    kernel.create_plot_thread(create_input()).await.unwrap();
    sqlx::query(
        "INSERT INTO timeline_event
           (id, novel_id, chapter_order, body, created_at, source_revision, status, updated_at, source_chapter_id)
         VALUES ('fresh-event', 'n1', 5, '第五章事件', 1, '1', 'confirmed', 1, 'c5')",
    )
    .execute(kernel.pool())
    .await
    .unwrap();
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
            now_ms: 900,
        },
    )
    .await
    .unwrap();

    let edge = kernel
        .create_causality_edge(NewCausalityEdge {
            id: "stale-edge".into(),
            novel_id: "n1".into(),
            from_kind: "plot_thread".into(),
            from_id: "plot-1".into(),
            to_kind: "timeline_event".into(),
            to_id: "fresh-event".into(),
            relation: "导致".into(),
            chapter_id: "c5".into(),
            source_revision: 1,
            evidence: "第五章玉佩发光。".into(),
            now_ms: 1,
        })
        .await;
    assert!(
        matches!(edge, Err(PortError::Conflict(_))),
        "来源失效的伏笔不能作为新因果边的端点"
    );
}

#[tokio::test]
async fn causality_marks_invalid_endpoints_after_rewrite_or_delete() {
    let (_dir, kernel) = setup().await;
    for (id, chapter, body) in [
        ("e1", "c1", "事件一：得到玉佩"),
        ("e2", "c5", "事件二：玉佩发光"),
    ] {
        sqlx::query(
            "INSERT INTO timeline_event
               (id, novel_id, chapter_order, body, created_at, source_revision, status, updated_at, source_chapter_id)
             VALUES (?, 'n1', 1, ?, 1, '1', 'confirmed', 1, ?)",
        )
        .bind(id)
        .bind(body)
        .bind(chapter)
        .execute(kernel.pool())
        .await
        .unwrap();
    }
    kernel
        .create_causality_edge(NewCausalityEdge {
            id: "endpoint-edge".into(),
            novel_id: "n1".into(),
            from_kind: "timeline_event".into(),
            from_id: "e1".into(),
            to_kind: "timeline_event".into(),
            to_id: "e2".into(),
            relation: "导致".into(),
            chapter_id: "c1".into(),
            source_revision: 1,
            evidence: "第一章埋下玉佩。".into(),
            now_ms: 1,
        })
        .await
        .unwrap();
    let edge = &kernel.list_causality_edges("n1").await.unwrap()[0];
    assert!(edge.from_valid && edge.to_valid);

    // 起点来源章节被改写：事件保留但被标记 superseded，边仍存在。
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
            now_ms: 800,
        },
    )
    .await
    .unwrap();
    let edge = &kernel.list_causality_edges("n1").await.unwrap()[0];
    assert!(!edge.from_valid, "起点事件被替代后必须标记失效");
    assert!(edge.to_valid);
    assert!(!edge.source_valid);

    // 终点来源章节删除：边不能静默消失，但终点必须标记失效。
    NovelRepository::delete_chapter(
        &kernel,
        novel_application::DeleteChapterRecord {
            novel_id: "n1".into(),
            chapter_id: "c5".into(),
            expected_version: 1,
        },
    )
    .await
    .unwrap();
    let edge = &kernel.list_causality_edges("n1").await.unwrap()[0];
    assert!(!edge.from_valid && !edge.to_valid);
    assert_eq!(kernel.list_causality_edges("n1").await.unwrap().len(), 1);
}

#[tokio::test]
async fn concurrent_causality_edges_cannot_form_a_cycle() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("edge-race.sqlite");
    let first = SqliteKernel::open(&path).await.unwrap();
    seed_race_chapters(&first).await;
    first.create_plot_thread(create_input()).await.unwrap();
    first
        .transition_plot_thread(transition(
            "develop",
            "c5",
            "第五章玉佩发光。",
            1,
            "develop-1",
        ))
        .await
        .unwrap();
    let second = SqliteKernel::open(&path).await.unwrap();

    // 两条伏笔事件作为端点，分别从两个连接同时建立 A→B 与 B→A。
    for (id, order) in [("edge-a", 7), ("edge-b", 8)] {
        sqlx::query(
            "INSERT INTO plot_thread
               (id, novel_id, title, body, opened_chapter_order, resolved_chapter_order, status,
                source_revision, evidence_json, created_at, updated_at, source_chapter_id, version)
             VALUES (?, 'n1', ?, ?, 1, NULL, 'open', '1', '第一章埋下玉佩。', 1, 1, 'c1', 0)",
        )
        .bind(id)
        .bind(format!("伏笔{order}"))
        .bind(format!("正文{order}"))
        .execute(first.pool())
        .await
        .unwrap();
    }
    let edge = |id: &str, from: &str, to: &str| NewCausalityEdge {
        id: id.into(),
        novel_id: "n1".into(),
        from_kind: "plot_thread".into(),
        from_id: from.into(),
        to_kind: "plot_thread".into(),
        to_id: to.into(),
        relation: "导致".into(),
        chapter_id: "c1".into(),
        source_revision: 1,
        evidence: "第一章埋下玉佩。".into(),
        now_ms: 1,
    };
    let (left, right) = tokio::join!(
        first.create_causality_edge(edge("race-edge-1", "edge-a", "edge-b")),
        second.create_causality_edge(edge("race-edge-2", "edge-b", "edge-a"))
    );
    assert_eq!(
        usize::from(left.is_ok()) + usize::from(right.is_ok()),
        1,
        "并发建边只能有一条成功"
    );
    let failure = if left.is_err() { &left } else { &right };
    assert!(
        matches!(failure, Err(PortError::Conflict(_))),
        "锁冲突或环检测失败必须报告为冲突，不能伪装成成功：{failure:?}"
    );
    let edges = first.list_causality_edges("n1").await.unwrap();
    assert_eq!(edges.len(), 1, "失败操作不能留下半完成边");
    assert!(edges[0].from_valid && edges[0].to_valid);
}
