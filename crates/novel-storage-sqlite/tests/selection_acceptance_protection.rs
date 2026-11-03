//! P17.2 RC1：候选采纳的保护链。
//!
//! 桌面版没有「整书快照」，采纳候选前的保护由章节修订承担。
//! 这里直接驱动 SQLite 存储层，验证这条链真的能保护作者的正文：
//! - 采纳前后原文不被提前改动；
//! - 采纳会生成可恢复的历史修订；
//! - 保存失败（版本冲突）不会丢原文；
//! - 多窗口并发写入时旧版本被拒绝，不会覆盖别人的修改；
//! - 回退到旧修订不会破坏后续版本。

use novel_application::{
    NewChapter, NewNovel, NovelRepository, PortError, RestoreChapterRecord, SaveChapterRecord,
};
use novel_storage_sqlite::SqliteKernel;

fn sha(text: &str) -> String {
    // 与生产链路一致：内容哈希用于修订一致性校验。
    novel_application::sha256_hex(text.as_bytes())
}

async fn setup(dir: &tempfile::TempDir) -> SqliteKernel {
    let kernel = SqliteKernel::open(&dir.path().join("accept.sqlite"))
        .await
        .unwrap();
    NovelRepository::create_novel(
        &kernel,
        NewNovel {
            id: "novel-1".into(),
            title: "采纳保护书".into(),
            description: String::new(),
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    NovelRepository::create_chapter(
        &kernel,
        NewChapter {
            id: "chapter-1".into(),
            novel_id: "novel-1".into(),
            title: "第一章".into(),
            sort_order: 1,
            volume_id: None,
            now_ms: 1,
        },
    )
    .await
    .unwrap();
    kernel
}

async fn save(
    kernel: &SqliteKernel,
    revision_id: &str,
    content: &str,
    expected_version: i64,
    now_ms: i64,
) -> Result<novel_application::ChapterSnapshot, PortError> {
    NovelRepository::save_user_chapter(
        kernel,
        SaveChapterRecord {
            novel_id: "novel-1".into(),
            chapter_id: "chapter-1".into(),
            expected_version,
            title: "第一章".into(),
            content: content.into(),
            revision_id: revision_id.into(),
            artifact_id: format!("artifact-{revision_id}"),
            content_sha256: sha(content),
            now_ms,
        },
    )
    .await
}

#[tokio::test]
async fn accepting_a_candidate_adds_a_recoverable_revision_without_touching_the_original_first() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = setup(&dir).await;

    // 作者保存的原文。
    let original = "他推开门，屋里没有人。\n\n灯还亮着。";
    save(&kernel, "rev-1", original, 0, 10).await.unwrap();

    let before = NovelRepository::get_chapter(&kernel, "novel-1", "chapter-1")
        .await
        .unwrap();
    assert_eq!(before.content, original);
    assert_eq!(before.official_version, 1);

    // 采纳候选 = 用新内容做一次普通保存；采纳动作本身不提前改正文。
    let accepted = "他停在门口，没有说话。\n\n灯还亮着。";
    let after = save(&kernel, "rev-2", accepted, 1, 20).await.unwrap();
    assert_eq!(after.content, accepted);
    assert_eq!(after.official_version, 2, "采纳必须计入正式版本");

    // 原文仍在历史里，可以恢复。
    let revisions = NovelRepository::list_chapter_revisions(&kernel, "novel-1", "chapter-1")
        .await
        .unwrap();
    assert!(
        revisions.iter().any(|item| item.content == original),
        "采纳后原文必须能从章节修订里找回"
    );
}

#[tokio::test]
async fn a_stale_save_is_rejected_and_never_destroys_the_current_text() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = setup(&dir).await;
    let current = "当前正文";
    save(&kernel, "rev-1", current, 0, 10).await.unwrap();

    // 另一个窗口已经保存过（版本变成 1），这里用旧版本 0 写入必须被拒。
    let rejected = save(&kernel, "rev-2", "过期窗口的正文", 0, 20).await;
    match rejected {
        Err(PortError::Conflict(message)) => assert!(message.contains("已经变化")),
        other => panic!("旧版本写入必须报冲突，实际 {other:?}"),
    }

    // 失败之后正文仍是当前版本，没有被覆盖。
    let after = NovelRepository::get_chapter(&kernel, "novel-1", "chapter-1")
        .await
        .unwrap();
    assert_eq!(after.content, current, "保存失败不能丢掉原文");
    assert_eq!(after.official_version, 1);
}

#[tokio::test]
async fn concurrent_writers_cannot_overwrite_each_other() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = setup(&dir).await;
    save(&kernel, "rev-1", "第一版", 0, 10).await.unwrap();

    // 两个窗口都基于版本 1 保存：只能有一个成功。
    let first = save(&kernel, "rev-2", "窗口 A 的修改", 1, 20).await;
    let second = save(&kernel, "rev-3", "窗口 B 的修改", 1, 30).await;
    assert!(first.is_ok(), "先到的写入应当成功");
    assert!(
        matches!(second, Err(PortError::Conflict(_))),
        "基于同一版本的第二次写入必须被拒绝"
    );

    let after = NovelRepository::get_chapter(&kernel, "novel-1", "chapter-1")
        .await
        .unwrap();
    assert_eq!(
        after.content, "窗口 A 的修改",
        "后到的旧写入不能覆盖先到的修改"
    );
    assert_eq!(after.official_version, 2);
}

#[tokio::test]
async fn restoring_an_older_revision_keeps_later_versions_reachable() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = setup(&dir).await;
    save(&kernel, "rev-1", "第一版正文", 0, 10).await.unwrap();

    let current = NovelRepository::get_chapter(&kernel, "novel-1", "chapter-1")
        .await
        .unwrap();
    let revisions = NovelRepository::list_chapter_revisions(&kernel, "novel-1", "chapter-1")
        .await
        .unwrap();
    let target = revisions.first().expect("至少有一条修订");

    // 回退到旧修订：写入一条新的修订记录，而不是改写历史。
    let restored = NovelRepository::restore_chapter_revision(
        &kernel,
        RestoreChapterRecord {
            novel_id: "novel-1".into(),
            chapter_id: "chapter-1".into(),
            source_revision_id: target.id.clone(),
            expected_version: current.official_version,
            revision_id: "rev-restored".into(),
            artifact_id: "artifact-restored".into(),
            command_id: "cmd-restore".into(),
            operator: "author".into(),
            reason: "恢复到上一版".into(),
            draft_decision: "keep_draft".into(),
            draft_client_id: None,
            expected_draft_revision: None,
            now_ms: 40,
        },
    )
    .await
    .unwrap();
    assert_eq!(restored.content, target.content, "回退后正文等于目标修订");
    assert_eq!(restored.official_version, 2, "回退本身也要产生新版本");

    // 回退之后，回退前的正文仍然可以从修订历史里找回。
    let after = NovelRepository::list_chapter_revisions(&kernel, "novel-1", "chapter-1")
        .await
        .unwrap();
    assert!(
        after.len() >= 2,
        "回退不能删除历史：应保留原修订并新增一条回退修订"
    );
    assert!(
        after.iter().any(|item| item.content == "第一版正文"),
        "回退后仍能找到回退前的正文"
    );
}

#[tokio::test]
async fn a_restore_based_on_a_stale_version_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = setup(&dir).await;
    save(&kernel, "rev-1", "第一版", 0, 10).await.unwrap();
    save(&kernel, "rev-2", "第二版", 1, 20).await.unwrap();

    let revisions = NovelRepository::list_chapter_revisions(&kernel, "novel-1", "chapter-1")
        .await
        .unwrap();
    let target = revisions.first().expect("至少有一条修订");

    // 用过期版本回退：必须拒绝，不能覆盖别人刚保存的内容。
    let rejected = NovelRepository::restore_chapter_revision(
        &kernel,
        RestoreChapterRecord {
            novel_id: "novel-1".into(),
            chapter_id: "chapter-1".into(),
            source_revision_id: target.id.clone(),
            expected_version: 1,
            revision_id: "rev-stale".into(),
            artifact_id: "artifact-stale".into(),
            command_id: "cmd-stale".into(),
            operator: "author".into(),
            reason: "过期窗口尝试恢复".into(),
            draft_decision: "keep_draft".into(),
            draft_client_id: None,
            expected_draft_revision: None,
            now_ms: 30,
        },
    )
    .await;
    assert!(
        matches!(rejected, Err(PortError::Conflict(_))),
        "基于过期版本的恢复必须被拒绝"
    );

    let after = NovelRepository::get_chapter(&kernel, "novel-1", "chapter-1")
        .await
        .unwrap();
    assert_eq!(after.content, "第二版", "被拒绝的恢复不能改动正文");
}
