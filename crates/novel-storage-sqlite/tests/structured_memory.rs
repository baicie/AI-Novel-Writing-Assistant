use novel_application::{
    NewChapter, NewNovel, NewStructuredFact, NovelRepository, StructuredMemoryRepository,
};
use novel_storage_sqlite::SqliteKernel;

async fn character(kernel: &SqliteKernel, id: &str, name: &str) {
    sqlx::query("INSERT INTO character(id, novel_id, name, role, profile_json, created_at, updated_at) VALUES (?, 'n1', ?, '主角', '{}', 1, 1)")
        .bind(id).bind(name).execute(kernel.pool()).await.unwrap();
}

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
    sqlx::query("INSERT INTO chapter_revision(id, chapter_id, version, origin, content, content_sha256, created_at) VALUES (?, ?, 1, 'user', ?, 'hash', 1)")
        .bind(format!("rev-{id}")).bind(id).bind(content).execute(kernel.pool()).await.unwrap();
    sqlx::query("UPDATE chapter SET official_revision_id = ? WHERE id = ?")
        .bind(format!("rev-{id}"))
        .bind(id)
        .execute(kernel.pool())
        .await
        .unwrap();
}

#[tokio::test]
async fn structured_facts_supersede_at_as_of_and_reject_stale_revision() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = SqliteKernel::open(&dir.path().join("memory.sqlite"))
        .await
        .unwrap();
    NovelRepository::create_novel(
        &kernel,
        NewNovel {
            id: "n1".into(),
            title: "记忆".into(),
            description: String::new(),
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    character(&kernel, "lin", "林动").await;
    official_chapter(&kernel, "c1", 1, "林动在山门。").await;
    official_chapter(&kernel, "c2", 2, "林动抵达祖祠。").await;
    let first = kernel
        .confirm_structured_fact(NewStructuredFact {
            id: "f1".into(),
            novel_id: "n1".into(),
            entity_id: "lin".into(),
            fact_type: "location".into(),
            value_json: r#"{"location":"山门"}"#.into(),
            source_chapter_id: "c1".into(),
            source_revision: 1,
            evidence_quote: "林动在山门。".into(),
            effective_chapter_order: 1,
            now_ms: 10,
        })
        .await
        .unwrap();
    assert_eq!(first.truthfulness, "confirmed");
    kernel
        .confirm_structured_fact(NewStructuredFact {
            id: "f2".into(),
            novel_id: "n1".into(),
            entity_id: "lin".into(),
            fact_type: "location".into(),
            value_json: r#"{"location":"祖祠"}"#.into(),
            source_chapter_id: "c2".into(),
            source_revision: 1,
            evidence_quote: "林动抵达祖祠。".into(),
            effective_chapter_order: 2,
            now_ms: 20,
        })
        .await
        .unwrap();
    let all = kernel
        .list_structured_facts("n1", Some("lin"), None)
        .await
        .unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(all.iter().filter(|fact| fact.status == "active").count(), 1);
    assert_eq!(
        all.iter()
            .find(|fact| fact.id == "f1")
            .unwrap()
            .superseded_by
            .as_deref(),
        Some("f2")
    );
    let as_of_one = kernel
        .list_structured_facts("n1", Some("lin"), Some(1))
        .await
        .unwrap();
    assert_eq!(as_of_one.len(), 1);
    assert!(as_of_one[0].value_json.contains("山门"));
    let as_of_two = kernel
        .list_structured_facts("n1", Some("lin"), Some(2))
        .await
        .unwrap();
    assert_eq!(as_of_two.len(), 1);
    assert!(as_of_two[0].value_json.contains("祖祠"));
    // A new official revision invalidates the fact source until the author reconfirms.
    sqlx::query("INSERT INTO chapter_revision(id, chapter_id, version, origin, content, content_sha256, created_at) VALUES ('rev-c2-v2','c2',2,'user','改稿', 'hash2', 2)").execute(kernel.pool()).await.unwrap();
    sqlx::query("UPDATE chapter SET official_revision_id='rev-c2-v2' WHERE id='c2'")
        .execute(kernel.pool())
        .await
        .unwrap();
    let after_invalidation = kernel
        .list_structured_facts("n1", Some("lin"), None)
        .await
        .unwrap();
    assert!(
        after_invalidation
            .iter()
            .all(|fact| fact.source_chapter_id != "c2")
    );
}

#[tokio::test]
async fn conflict_is_atomic_and_resolves_new_or_existing() {
    use novel_application::MemoryRepository;
    let dir = tempfile::tempdir().unwrap();
    let kernel = SqliteKernel::open(&dir.path().join("conflict.sqlite"))
        .await
        .unwrap();
    NovelRepository::create_novel(
        &kernel,
        NewNovel {
            id: "n1".into(),
            title: "冲突".into(),
            description: String::new(),
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    character(&kernel, "lin", "林动").await;
    official_chapter(&kernel, "c1", 1, "林动在山门。").await;
    kernel
        .confirm_structured_fact(NewStructuredFact {
            id: "f1".into(),
            novel_id: "n1".into(),
            entity_id: "lin".into(),
            fact_type: "location".into(),
            value_json: r#"{"location":"山门"}"#.into(),
            source_chapter_id: "c1".into(),
            source_revision: 1,
            evidence_quote: "林动在山门。".into(),
            effective_chapter_order: 1,
            now_ms: 10,
        })
        .await
        .unwrap();
    let proposed = kernel
        .confirm_structured_fact(NewStructuredFact {
            id: "f2".into(),
            novel_id: "n1".into(),
            entity_id: "lin".into(),
            fact_type: "location".into(),
            value_json: r#"{"location":"祖祠"}"#.into(),
            source_chapter_id: "c1".into(),
            source_revision: 1,
            evidence_quote: "林动在祖祠。".into(),
            effective_chapter_order: 1,
            now_ms: 20,
        })
        .await
        .unwrap();
    assert_eq!(proposed.truthfulness, "proposed");
    assert_eq!(proposed.status, "disputed");
    let conflicts = kernel.list_memory_conflicts("n1").await.unwrap();
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].status, "open");
    let resolved = kernel
        .resolve_memory_conflict(
            &conflicts[0].id,
            0,
            "cmd-confirm",
            "hash-confirm",
            "confirm_new",
            None,
            "采用新事实",
            "author",
            30,
        )
        .await
        .unwrap();
    assert_eq!(resolved.status, "resolved");
    let facts = kernel
        .list_structured_facts("n1", Some("lin"), None)
        .await
        .unwrap();
    let old = facts.iter().find(|fact| fact.id == "f1").unwrap();
    let new = facts.iter().find(|fact| fact.id == "f2").unwrap();
    assert_eq!(old.status, "superseded");
    assert_eq!(new.truthfulness, "confirmed");
    assert_eq!(new.status, "active");
    let as_of = kernel
        .list_structured_facts("n1", Some("lin"), Some(1))
        .await
        .unwrap();
    assert_eq!(as_of.len(), 1);
    assert!(as_of[0].value_json.contains("祖祠"));

    // A second conflict can be kept as the existing fact.
    let conflict_two = kernel
        .confirm_structured_fact(NewStructuredFact {
            id: "f3".into(),
            novel_id: "n1".into(),
            entity_id: "lin".into(),
            fact_type: "life_state".into(),
            value_json: r#"{"state":"alive"}"#.into(),
            source_chapter_id: "c1".into(),
            source_revision: 1,
            evidence_quote: "林动活着。".into(),
            effective_chapter_order: 1,
            now_ms: 40,
        })
        .await
        .unwrap();
    assert_eq!(conflict_two.truthfulness, "confirmed");
    kernel
        .confirm_structured_fact(NewStructuredFact {
            id: "f4".into(),
            novel_id: "n1".into(),
            entity_id: "lin".into(),
            fact_type: "life_state".into(),
            value_json: r#"{"state":"dead"}"#.into(),
            source_chapter_id: "c1".into(),
            source_revision: 1,
            evidence_quote: "传闻林动已死。".into(),
            effective_chapter_order: 1,
            now_ms: 50,
        })
        .await
        .unwrap();
    let life_conflict = kernel
        .list_memory_conflicts("n1")
        .await
        .unwrap()
        .into_iter()
        .find(|c| c.status == "open")
        .unwrap();
    kernel
        .resolve_memory_conflict(
            &life_conflict.id,
            0,
            "cmd-keep",
            "hash-keep",
            "keep_existing",
            None,
            "保留现有",
            "author",
            60,
        )
        .await
        .unwrap();
    let life = kernel
        .list_structured_facts("n1", Some("lin"), None)
        .await
        .unwrap()
        .into_iter()
        .find(|f| f.fact_type == "life_state" && f.id == "f3")
        .unwrap();
    assert_eq!(life.status, "active");
}

#[tokio::test]
async fn aliases_and_relationships_coexist_and_do_not_merge_people() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = SqliteKernel::open(&dir.path().join("alias.sqlite"))
        .await
        .unwrap();
    NovelRepository::create_novel(
        &kernel,
        NewNovel {
            id: "n1".into(),
            title: "别名".into(),
            description: String::new(),
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    character(&kernel, "lin", "林动").await;
    character(&kernel, "su", "苏晚").await;
    official_chapter(&kernel, "c1", 1, "林动，人称林公子。").await;
    let base = NewStructuredFact {
        id: String::new(),
        novel_id: "n1".into(),
        entity_id: "lin".into(),
        fact_type: "alias".into(),
        value_json: String::new(),
        source_chapter_id: "c1".into(),
        source_revision: 1,
        evidence_quote: "人称林公子。".into(),
        effective_chapter_order: 1,
        now_ms: 10,
    };
    kernel
        .confirm_structured_fact(NewStructuredFact {
            id: "a1".into(),
            value_json: r#"{"alias":"林公子"}"#.into(),
            ..base.clone()
        })
        .await
        .unwrap();
    kernel
        .confirm_structured_fact(NewStructuredFact {
            id: "a2".into(),
            value_json: r#"{"alias":"山主"}"#.into(),
            ..base.clone()
        })
        .await
        .unwrap();
    kernel
        .confirm_structured_fact(NewStructuredFact {
            id: "r1".into(),
            entity_id: "lin".into(),
            fact_type: "relationship".into(),
            value_json: format!(
                r#"{{"relation":"mentor_of","targetEntityId":"su","targetName":"苏晚"}}"#
            ),
            ..base.clone()
        })
        .await
        .unwrap();
    kernel
        .confirm_structured_fact(NewStructuredFact {
            id: "r2".into(),
            entity_id: "lin".into(),
            fact_type: "relationship".into(),
            value_json: format!(
                r#"{{"relation":"ally_of","targetEntityId":"su","targetName":"苏晚"}}"#
            ),
            ..base
        })
        .await
        .unwrap();
    let as_of = kernel
        .list_structured_facts("n1", Some("lin"), Some(1))
        .await
        .unwrap();
    assert_eq!(
        as_of
            .iter()
            .filter(|fact| fact.fact_type == "alias")
            .count(),
        2
    );
    assert_eq!(
        as_of
            .iter()
            .filter(|fact| fact.fact_type == "relationship")
            .count(),
        2
    );
    assert!(as_of.iter().all(|fact| fact.status == "active"));
}
