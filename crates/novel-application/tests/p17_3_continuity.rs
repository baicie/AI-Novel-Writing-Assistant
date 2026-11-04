use std::sync::Arc;

use novel_application::{
    ChapterContinuityService, NewChapter, NewCharacterState, NewNovel, NewStructuredFact,
    NovelRepository, SaveChapterRecord, StructuredMemoryRepository,
};
use novel_storage_sqlite::SqliteKernel;

async fn kernel(dir: &tempfile::TempDir) -> Arc<SqliteKernel> {
    Arc::new(
        SqliteKernel::open(&dir.path().join("continuity.sqlite"))
            .await
            .unwrap(),
    )
}

async fn seed_novel(kernel: &SqliteKernel, novel_id: &str, chapters: i32) {
    NovelRepository::create_novel(
        kernel,
        NewNovel {
            id: novel_id.into(),
            title: format!("连续性 {novel_id}"),
            description: String::new(),
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    for order in 1..=chapters {
        NovelRepository::create_chapter(
            kernel,
            NewChapter {
                id: format!("{novel_id}-ch{order}"),
                novel_id: novel_id.into(),
                title: format!("第{order}章"),
                sort_order: order,
                volume_id: None,
                now_ms: order as i64,
            },
        )
        .await
        .unwrap();
    }
}

async fn save_chapter(
    kernel: &SqliteKernel,
    novel_id: &str,
    chapter_id: &str,
    order: i32,
    expected_version: i64,
) -> i64 {
    let version = expected_version + 1;
    NovelRepository::save_user_chapter(
        kernel,
        SaveChapterRecord {
            novel_id: novel_id.into(),
            chapter_id: chapter_id.into(),
            expected_version,
            title: format!("第{order}章"),
            content: format!("第{order}章正式正文，版本 {version}。"),
            revision_id: format!("{chapter_id}-rev{version}"),
            artifact_id: format!("{chapter_id}-artifact{version}"),
            content_sha256: format!("{chapter_id}-hash{version}"),
            now_ms: 100 + version,
        },
    )
    .await
    .unwrap();
    version
}

#[allow(clippy::too_many_arguments)]
async fn confirm_fact(
    kernel: &SqliteKernel,
    id: &str,
    novel_id: &str,
    entity_id: &str,
    fact_type: &str,
    value_json: &str,
    chapter_id: &str,
    source_revision: i64,
    effective_order: i64,
) {
    StructuredMemoryRepository::confirm_structured_fact(
        kernel,
        NewStructuredFact {
            id: id.into(),
            novel_id: novel_id.into(),
            entity_id: entity_id.into(),
            fact_type: fact_type.into(),
            value_json: value_json.into(),
            source_chapter_id: chapter_id.into(),
            source_revision,
            evidence_quote: format!("{chapter_id} 的原文证据"),
            effective_chapter_order: effective_order,
            now_ms: effective_order + 1_000,
        },
    )
    .await
    .unwrap();
}

async fn add_character(kernel: &SqliteKernel, novel_id: &str, id: &str, name: &str) {
    sqlx::query(
        "INSERT INTO character(id, novel_id, name, role, profile_json, created_at, updated_at)
         VALUES (?, ?, ?, '主角', '{}', 1, 1)",
    )
    .bind(id)
    .bind(novel_id)
    .bind(name)
    .execute(kernel.pool())
    .await
    .unwrap();
}

async fn add_candidate(
    kernel: &SqliteKernel,
    novel_id: &str,
    chapter_id: &str,
    chapter_version: i64,
    id: &str,
    value: &str,
    entity_id: Option<&str>,
) {
    sqlx::query(
        "INSERT INTO memory_extraction_job
           (id, command_id, novel_id, chapter_id, chapter_version, source_content_hash,
            extraction_version, provider, model, endpoint, status, created_at, updated_at)
         VALUES ('job-1', 'cmd-1', ?, ?, ?, 'hash', 'v1', 'mock', 'mock', 'mock', 'succeeded', 1, 1)",
    )
    .bind(novel_id)
    .bind(chapter_id)
    .bind(chapter_version)
    .execute(kernel.pool())
    .await
    .ok();
    sqlx::query(
        "INSERT INTO memory_extraction_candidate
           (id, job_id, novel_id, chapter_id, chapter_version, source_content_hash, fact_type,
            truthfulness, entity_name, entity_id, value, evidence_quote, evidence_start,
            confidence, status, ambiguous, version, decision_reason, created_at, updated_at)
         VALUES (?, 'job-1', ?, ?, ?, 'hash', 'character_state', 'narrator_fact',
                 '林动', ?, ?, '候选原文证据', 0, 0.9, 'proposed', 0, 0, NULL, 10, 10)",
    )
    .bind(id)
    .bind(novel_id)
    .bind(chapter_id)
    .bind(chapter_version)
    .bind(entity_id)
    .bind(value)
    .execute(kernel.pool())
    .await
    .unwrap();
}

#[tokio::test]
async fn thousand_chapter_projection_stays_bounded_and_keeps_as_of() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = kernel(&dir).await;
    seed_novel(kernel.as_ref(), "n", 1000).await;
    for index in 0..40 {
        add_character(
            kernel.as_ref(),
            "n",
            &format!("c{index}"),
            &format!("人物{index:02}"),
        )
        .await;
    }
    // 只给前 200 章建立正式版本与已确认事实，目标章节读第 500 章。
    for order in (1..=200).rev() {
        save_chapter(kernel.as_ref(), "n", &format!("n-ch{order}"), order, 0).await;
        let character = format!("c{}", order % 40);
        confirm_fact(
            kernel.as_ref(),
            &format!("f-{order}"),
            "n",
            &character,
            "life_state",
            &format!(r#"{{"state":"alive","note":"{order}"}}"#),
            &format!("n-ch{order}"),
            1,
            i64::from(order),
        )
        .await;
    }
    let service = ChapterContinuityService::new(kernel.clone(), kernel.clone(), kernel.clone());
    let started = std::time::Instant::now();
    let projection = service.project("n", "n-ch500").await.unwrap();
    let elapsed = started.elapsed();
    assert_eq!(projection.chapter_order, 500);
    assert!(
        projection
            .facts
            .iter()
            .all(|fact| fact.effective_chapter_order <= 500),
        "投影不能包含未来章节的事实"
    );
    assert_eq!(
        projection
            .facts
            .iter()
            .filter(|fact| fact.fact_type == "life_state")
            .count(),
        40,
        "每个角色只保留 As-Of 时点的一条状态"
    );
    assert!(
        elapsed < std::time::Duration::from_secs(2),
        "1000 章规模下投影耗时过长：{elapsed:?}"
    );
    println!(
        "P17.3 continuity scale: chapters=1000 characters=40 facts=200 elapsed={elapsed:?} projected={}",
        projection.facts.len()
    );
}

#[tokio::test]
async fn as_of_chapter_never_exposes_future_facts() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = kernel(&dir).await;
    seed_novel(kernel.as_ref(), "n", 30).await;
    add_character(kernel.as_ref(), "n", "c1", "林动").await;
    save_chapter(kernel.as_ref(), "n", "n-ch1", 1, 0).await;
    save_chapter(kernel.as_ref(), "n", "n-ch20", 20, 0).await;
    confirm_fact(
        kernel.as_ref(),
        "f-alive",
        "n",
        "c1",
        "life_state",
        r#"{"state":"alive"}"#,
        "n-ch1",
        1,
        1,
    )
    .await;
    confirm_fact(
        kernel.as_ref(),
        "f-dead",
        "n",
        "c1",
        "life_state",
        r#"{"state":"dead"}"#,
        "n-ch20",
        1,
        20,
    )
    .await;

    let service = ChapterContinuityService::new(kernel.clone(), kernel.clone(), kernel.clone());
    let at15 = service.project("n", "n-ch15").await.unwrap();
    assert!(
        at15.facts.iter().any(|fact| fact.id == "f-alive"),
        "第15章必须看到第1章的存活状态"
    );
    assert!(
        !at15.facts.iter().any(|fact| fact.id == "f-dead"),
        "第15章不能看到第20章的死亡事实"
    );
    assert_eq!(at15.chapter_order, 15);
    assert_eq!(at15.chapter_version, 0);
    let alive = at15.facts.iter().find(|fact| fact.id == "f-alive").unwrap();
    assert_eq!(alive.source_chapter_order, Some(1));
    assert_eq!(alive.source_chapter_title.as_deref(), Some("第1章"));
    assert_eq!(alive.source_revision, Some(1));
    assert_eq!(alive.status, "valid");
    assert_eq!(alive.entity_name.as_deref(), Some("林动"));
    assert_eq!(
        at15.characters[0]
            .current_state
            .as_ref()
            .map(|state| state.body.as_str()),
        Some("存活"),
        "人物卡片要显示截至本章的有效状态"
    );

    let at25 = service.project("n", "n-ch25").await.unwrap();
    assert!(at25.facts.iter().any(|fact| fact.id == "f-dead"));
    assert!(!at25.facts.iter().any(|fact| fact.id == "f-alive"));
    assert_eq!(
        at25.characters[0]
            .current_state
            .as_ref()
            .map(|state| state.body.as_str()),
        Some("死亡")
    );

    // 稳定排序：同一章重复读取必须得到完全一致的顺序。
    let again = service.project("n", "n-ch25").await.unwrap();
    assert_eq!(
        at25.facts.iter().map(|fact| &fact.id).collect::<Vec<_>>(),
        again.facts.iter().map(|fact| &fact.id).collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn facts_are_isolated_per_novel_and_candidates_are_never_confirmed() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = kernel(&dir).await;
    seed_novel(kernel.as_ref(), "n", 10).await;
    seed_novel(kernel.as_ref(), "other", 10).await;
    add_character(kernel.as_ref(), "n", "c1", "林动").await;
    add_character(kernel.as_ref(), "other", "c2", "外人").await;
    save_chapter(kernel.as_ref(), "n", "n-ch3", 3, 0).await;
    save_chapter(kernel.as_ref(), "other", "other-ch3", 3, 0).await;
    confirm_fact(
        kernel.as_ref(),
        "f-own",
        "n",
        "c1",
        "location",
        r#"{"location":"山门"}"#,
        "n-ch3",
        1,
        3,
    )
    .await;
    confirm_fact(
        kernel.as_ref(),
        "f-other",
        "other",
        "c2",
        "location",
        r#"{"location":"别处"}"#,
        "other-ch3",
        1,
        3,
    )
    .await;
    add_candidate(
        kernel.as_ref(),
        "n",
        "n-ch3",
        1,
        "cand-1",
        "状态：受伤",
        Some("c1"),
    )
    .await;

    let service = ChapterContinuityService::new(kernel.clone(), kernel.clone(), kernel.clone());
    let projection = service.project("n", "n-ch5").await.unwrap();
    assert!(projection.facts.iter().any(|fact| fact.id == "f-own"));
    assert!(!projection.facts.iter().any(|fact| fact.id == "f-other"));
    assert_eq!(projection.candidates.len(), 1);
    let candidate = &projection.candidates[0];
    assert_eq!(candidate.status, "proposed");
    assert!(candidate.can_confirm);
    assert!(!candidate.expired);
    assert!(
        projection.facts.iter().all(|fact| fact.id != candidate.id),
        "候选不能出现在已确认事实里"
    );
}

#[tokio::test]
async fn chapter_version_change_marks_fact_stale_and_candidate_expired() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = kernel(&dir).await;
    seed_novel(kernel.as_ref(), "n", 10).await;
    add_character(kernel.as_ref(), "n", "c1", "林动").await;
    save_chapter(kernel.as_ref(), "n", "n-ch3", 3, 0).await;
    confirm_fact(
        kernel.as_ref(),
        "f-location",
        "n",
        "c1",
        "location",
        r#"{"location":"山门"}"#,
        "n-ch3",
        1,
        3,
    )
    .await;
    add_candidate(
        kernel.as_ref(),
        "n",
        "n-ch3",
        1,
        "cand-1",
        "状态：受伤",
        Some("c1"),
    )
    .await;

    let service = ChapterContinuityService::new(kernel.clone(), kernel.clone(), kernel.clone());
    let before = service.project("n", "n-ch5").await.unwrap();
    assert!(before.facts.iter().any(|fact| fact.id == "f-location"));
    assert!(before.stale_facts.is_empty());
    assert!(!before.candidates[0].expired);

    save_chapter(kernel.as_ref(), "n", "n-ch3", 3, 1).await;

    let after = service.project("n", "n-ch5").await.unwrap();
    assert!(
        !after.facts.iter().any(|fact| fact.id == "f-location"),
        "来源章节更新后，旧事实不能继续冒充有效证据"
    );
    let stale = after
        .stale_facts
        .iter()
        .find(|fact| fact.id == "f-location")
        .expect("旧事实必须出现在 stale 列表");
    assert_eq!(stale.status, "stale");
    assert_eq!(stale.source_current_version, Some(2));
    assert_eq!(stale.source_revision, Some(1));
    let expired = &after.candidates[0];
    assert!(expired.expired, "来源版本变化后候选必须标记过期");
    assert!(!expired.can_confirm, "过期候选不能直接确认");

    // 按当前版本重新确认后，旧事实不再停留在「待重新确认」列表里。
    confirm_fact(
        kernel.as_ref(),
        "f-location-v2",
        "n",
        "c1",
        "location",
        r#"{"location":"山门"}"#,
        "n-ch3",
        2,
        3,
    )
    .await;
    let reconfirmed = service.project("n", "n-ch5").await.unwrap();
    assert!(
        reconfirmed
            .stale_facts
            .iter()
            .all(|fact| fact.id != "f-location"),
        "被新事实替代的旧版本不能继续提示重新确认"
    );
    assert!(
        reconfirmed
            .facts
            .iter()
            .any(|fact| fact.id == "f-location-v2"),
        "重新确认后应出现新的有效事实"
    );
}

#[tokio::test]
async fn story_fact_without_source_is_marked_unavailable() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = kernel(&dir).await;
    seed_novel(kernel.as_ref(), "n", 5).await;
    add_character(kernel.as_ref(), "n", "c1", "林动").await;
    NovelRepository::upsert_character_state(
        kernel.as_ref(),
        NewCharacterState {
            id: "legacy-state".into(),
            character_id: "c1".into(),
            chapter_order: 2,
            body: "旧版人物状态".into(),
            source_chapter_id: String::new(),
            source_revision: "legacy".into(),
            status: "confirmed".into(),
            now_ms: 10,
        },
    )
    .await
    .unwrap();

    let service = ChapterContinuityService::new(kernel.clone(), kernel.clone(), kernel.clone());
    let projection = service.project("n", "n-ch3").await.unwrap();
    let fact = projection
        .facts
        .iter()
        .find(|fact| fact.id == "legacy-state")
        .expect("没有来源的旧事实仍要展示内容");
    assert_eq!(fact.status, "unavailable");
    assert_eq!(fact.source_chapter_order, None);
    assert_eq!(
        projection.characters[0]
            .current_state
            .as_ref()
            .map(|state| state.body.as_str()),
        Some("旧版人物状态"),
        "内容可以展示，但来源状态必须是 unavailable"
    );
}

#[tokio::test]
async fn conflict_sides_expose_both_sources_without_auto_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = kernel(&dir).await;
    seed_novel(kernel.as_ref(), "n", 10).await;
    add_character(kernel.as_ref(), "n", "c1", "林动").await;
    save_chapter(kernel.as_ref(), "n", "n-ch3", 3, 0).await;
    confirm_fact(
        kernel.as_ref(),
        "f-alive",
        "n",
        "c1",
        "life_state",
        r#"{"state":"alive"}"#,
        "n-ch3",
        1,
        3,
    )
    .await;
    // 同一章节位置出现相反状态：写入 disputed 事实并生成冲突，不覆盖原事实。
    confirm_fact(
        kernel.as_ref(),
        "f-dead",
        "n",
        "c1",
        "life_state",
        r#"{"state":"dead"}"#,
        "n-ch3",
        1,
        3,
    )
    .await;

    let service = ChapterContinuityService::new(kernel.clone(), kernel.clone(), kernel.clone());
    let projection = service.project("n", "n-ch5").await.unwrap();
    assert!(
        projection.facts.iter().any(|fact| fact.id == "f-alive"),
        "冲突不能静默覆盖已确认事实"
    );
    assert!(
        !projection.facts.iter().any(|fact| fact.id == "f-dead"),
        "disputed 事实不能进入有效事实"
    );
    let conflict = projection
        .conflicts
        .iter()
        .find(|conflict| conflict.entity_key == "c1")
        .expect("必须展示冲突");
    assert_eq!(conflict.sides.len(), 2);
    let confirmed = conflict
        .sides
        .iter()
        .find(|side| side.id == "f-alive")
        .unwrap();
    assert_eq!(confirmed.status, "valid");
    assert!(confirmed.available);
    assert_eq!(confirmed.source_chapter_order, Some(3));
    let disputed = conflict
        .sides
        .iter()
        .find(|side| side.id == "f-dead")
        .unwrap();
    assert_eq!(disputed.status, "disputed");
    assert!(!disputed.available);
    assert!(
        disputed
            .evidence_quote
            .as_deref()
            .unwrap()
            .contains("原文证据")
    );
}
