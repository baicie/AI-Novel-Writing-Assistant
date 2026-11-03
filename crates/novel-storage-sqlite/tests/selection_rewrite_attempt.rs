//! 选区改写的尝试记录：同一条请求编号不能在未知计费状态下被再次发出。
//!
//! 这些用例直接驱动 SQLite 存储层，不依赖 Tauri、也不调用任何真实模型。

use novel_application::{PortError, SelectionRewriteAttemptState};
use novel_storage_sqlite::SqliteKernel;

async fn kernel(dir: &tempfile::TempDir) -> SqliteKernel {
    SqliteKernel::open(&dir.path().join("rewrite.sqlite"))
        .await
        .unwrap()
}

#[tokio::test]
async fn an_attempt_in_flight_blocks_the_same_request_id() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = kernel(&dir).await;

    kernel
        .begin_selection_rewrite("req-1", "novel-1", "chapter-1", 10)
        .await
        .unwrap();

    // 进程在请求中途退出：记录停在 sending，这本身就是「未知计费」。
    let attempt = kernel
        .selection_rewrite_attempt("req-1")
        .await
        .unwrap()
        .expect("登记后必须能读回");
    assert_eq!(attempt.state, SelectionRewriteAttemptState::Sending);
    assert!(attempt.state.may_have_been_billed());

    let error = kernel
        .begin_selection_rewrite("req-1", "novel-1", "chapter-1", 20)
        .await
        .expect_err("同一条请求编号不能带着未知状态再次发出");
    match error {
        PortError::Conflict(message) => {
            assert!(message.contains("已经发出") || message.contains("确认"));
        }
        other => panic!("重复请求必须是竞争错误，实际 {other:?}"),
    }
}

#[tokio::test]
async fn unknown_result_keeps_blocking_until_explicitly_resolved() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = kernel(&dir).await;

    kernel
        .begin_selection_rewrite("req-2", "novel-1", "chapter-1", 10)
        .await
        .unwrap();
    kernel
        .mark_selection_rewrite_unknown("req-2", "模型请求超时", 11)
        .await
        .unwrap();

    let attempt = kernel
        .selection_rewrite_attempt("req-2")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(attempt.state, SelectionRewriteAttemptState::Unknown);
    assert!(attempt.state.may_have_been_billed());
    assert_eq!(attempt.error_summary.as_deref(), Some("模型请求超时"));

    let blocked = kernel
        .begin_selection_rewrite("req-2", "novel-1", "chapter-1", 12)
        .await
        .expect_err("结果未知时不能自动重发");
    assert!(matches!(blocked, PortError::Conflict(_)));

    // 未知记录要能被章节目录查询到，界面才知道在重新生成前该提示什么。
    let pending = kernel
        .list_unknown_selection_rewrites("novel-1", "chapter-1")
        .await
        .unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].request_id, "req-2");
}

#[tokio::test]
async fn definite_failures_are_recorded_without_claiming_unknown_billing() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = kernel(&dir).await;

    kernel
        .begin_selection_rewrite("req-3", "novel-1", "chapter-1", 10)
        .await
        .unwrap();
    kernel
        .mark_selection_rewrite_not_sent("req-3", "还没有填写模型密钥", 11)
        .await
        .unwrap();

    let attempt = kernel
        .selection_rewrite_attempt("req-3")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(attempt.state, SelectionRewriteAttemptState::NotSent);
    assert!(
        !attempt.state.may_have_been_billed(),
        "确定没发出去的请求不能被描述成可能计费"
    );
    // 明确没发出的请求不应出现在「待确认」列表里。
    let pending = kernel
        .list_unknown_selection_rewrites("novel-1", "chapter-1")
        .await
        .unwrap();
    assert!(pending.is_empty());
}

#[tokio::test]
async fn unknown_attempts_are_scoped_to_their_own_chapter() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = kernel(&dir).await;

    kernel
        .begin_selection_rewrite("req-a", "novel-1", "chapter-1", 10)
        .await
        .unwrap();
    kernel
        .mark_selection_rewrite_unknown("req-a", "连接中断", 11)
        .await
        .unwrap();
    kernel
        .begin_selection_rewrite("req-b", "novel-1", "chapter-2", 12)
        .await
        .unwrap();

    let chapter_one = kernel
        .list_unknown_selection_rewrites("novel-1", "chapter-1")
        .await
        .unwrap();
    assert_eq!(chapter_one.len(), 1);
    assert_eq!(chapter_one[0].request_id, "req-a");

    // 另一章的未确认请求不能污染当前章节的提示。
    let chapter_two = kernel
        .list_unknown_selection_rewrites("novel-1", "chapter-2")
        .await
        .unwrap();
    assert_eq!(chapter_two.len(), 1);
    assert_eq!(chapter_two[0].request_id, "req-b");
}

#[tokio::test]
async fn stored_attempts_never_carry_prompt_or_chapter_text() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = kernel(&dir).await;

    kernel
        .begin_selection_rewrite("req-4", "novel-1", "chapter-1", 10)
        .await
        .unwrap();
    kernel
        .mark_selection_rewrite_unknown("req-4", "模型请求超时", 11)
        .await
        .unwrap();

    // 记录只用于判断计费状态；表里没有正文、提示词或凭据列。
    let columns: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM pragma_table_info('chapter_selection_rewrite_attempt') ORDER BY name",
    )
    .fetch_all(kernel.pool())
    .await
    .unwrap();
    assert_eq!(
        columns,
        vec![
            "chapter_id".to_string(),
            "created_at".to_string(),
            "error_summary".to_string(),
            "novel_id".to_string(),
            "request_id".to_string(),
            "state".to_string(),
            "updated_at".to_string(),
        ]
    );
}

// ---- 进程重启：未知计费状态必须跨进程保留 ----

#[tokio::test]
async fn unknown_attempts_survive_a_real_database_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("rewrite.sqlite");

    // 第一个进程：登记请求后崩溃（停在 sending）。
    {
        let kernel = SqliteKernel::open(&db_path).await.unwrap();
        kernel
            .begin_selection_rewrite("req-crash", "novel-1", "chapter-1", 10)
            .await
            .unwrap();
        // 这里不调用任何收尾方法，模拟进程被直接终止。
    }

    // 第二个进程：重新打开同一个数据库。
    let reopened = SqliteKernel::open(&db_path).await.unwrap();
    let attempt = reopened
        .selection_rewrite_attempt("req-crash")
        .await
        .unwrap()
        .expect("重启后仍然读得到这条请求");
    assert_eq!(attempt.state, SelectionRewriteAttemptState::Sending);
    assert!(
        attempt.state.may_have_been_billed(),
        "停在 sending 的请求在重启后必须继续按「可能已计费」处理"
    );

    // 重启后的进程不能自动重发同一条请求。
    let blocked = reopened
        .begin_selection_rewrite("req-crash", "novel-1", "chapter-1", 20)
        .await
        .expect_err("重启不能让未知状态消失");
    assert!(matches!(blocked, PortError::Conflict(_)));

    // 界面在重启后仍然能看到这条待确认记录。
    let pending = reopened
        .list_unknown_selection_rewrites("novel-1", "chapter-1")
        .await
        .unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].request_id, "req-crash");
}

#[tokio::test]
async fn an_explicit_retry_requires_a_new_request_id_after_unknown() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = kernel(&dir).await;

    kernel
        .begin_selection_rewrite("req-a", "novel-1", "chapter-1", 10)
        .await
        .unwrap();
    kernel
        .mark_selection_rewrite_unknown("req-a", "模型请求超时", 11)
        .await
        .unwrap();

    // 作者确认后重试：必须换一个新的请求编号，原记录保持未知。
    kernel
        .begin_selection_rewrite("req-b", "novel-1", "chapter-1", 12)
        .await
        .expect("换新编号后允许再次发起");
    kernel
        .mark_selection_rewrite_succeeded("req-b", 13)
        .await
        .unwrap();

    // 原来的未知记录不会被新请求「洗白」：仍要保留，供作者判断可能的费用。
    let original = kernel
        .selection_rewrite_attempt("req-a")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(original.state, SelectionRewriteAttemptState::Unknown);

    let new_attempt = kernel
        .selection_rewrite_attempt("req-b")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(new_attempt.state, SelectionRewriteAttemptState::Succeeded);

    // 未知记录仍在待确认列表里，直到作者显式处理。
    let pending = kernel
        .list_unknown_selection_rewrites("novel-1", "chapter-1")
        .await
        .unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].request_id, "req-a");
}
