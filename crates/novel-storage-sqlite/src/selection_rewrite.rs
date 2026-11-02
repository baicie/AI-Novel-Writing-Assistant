//! 选区改写的请求尝试记录。
//!
//! 选区改写不建 Run、不写候选表，因此「这次请求到底发没发出去、有没有可能已经计费」
//! 需要一个最小独立记录。这里只落状态与少量元数据：
//! - 不保存正文、选区原文、提示词；
//! - 不保存端点、模型密钥或任何凭据；
//! - 不是 RunContract 的一部分，也不会影响章节目录。

use novel_application::{PortError, SelectionRewriteAttemptState, SelectionRewriteAttemptView};
use sqlx::Row;

use crate::db::{SqliteKernel, map_sql};

impl SqliteKernel {
    /// 发起前先登记：同一条请求编号不能带着未知的计费状态再次发出。
    pub async fn begin_selection_rewrite(
        &self,
        request_id: &str,
        novel_id: &str,
        chapter_id: &str,
        now_ms: i64,
    ) -> Result<(), PortError> {
        let existing = self.selection_rewrite_attempt(request_id).await?;
        if let Some(attempt) = existing {
            return Err(match attempt.state {
                SelectionRewriteAttemptState::Unknown => {
                    PortError::Conflict("上一次改写的结果还没有确认，请先确认状态后再重试。".into())
                }
                SelectionRewriteAttemptState::Sending => {
                    PortError::Conflict("这次改写已经发出，正在等待结果。".into())
                }
                SelectionRewriteAttemptState::Succeeded => {
                    PortError::Conflict("这条改写请求已经完成过，请重新发起一次新的改写。".into())
                }
                SelectionRewriteAttemptState::Failed | SelectionRewriteAttemptState::NotSent => {
                    PortError::Conflict("这条改写请求还可以显式重试。".into())
                }
            });
        }
        sqlx::query(
            "INSERT INTO chapter_selection_rewrite_attempt
                (request_id, novel_id, chapter_id, state, error_summary, created_at, updated_at)
             VALUES (?, ?, ?, 'sending', NULL, ?, ?)",
        )
        .bind(request_id)
        .bind(novel_id)
        .bind(chapter_id)
        .bind(now_ms)
        .bind(now_ms)
        .execute(&self.pool)
        .await
        .map_err(map_sql)?;
        Ok(())
    }

    /// 请求没有被发出去：可以安全重试，界面不应提示可能的计费。
    pub async fn mark_selection_rewrite_not_sent(
        &self,
        request_id: &str,
        error_summary: &str,
        now_ms: i64,
    ) -> Result<(), PortError> {
        self.update_selection_rewrite_state(request_id, "not_sent", Some(error_summary), now_ms)
            .await
    }

    /// 请求已发出且服务端明确报错：这次不一定计费，但结果不是未知。
    pub async fn mark_selection_rewrite_failed(
        &self,
        request_id: &str,
        error_summary: &str,
        now_ms: i64,
    ) -> Result<(), PortError> {
        self.update_selection_rewrite_state(request_id, "failed", Some(error_summary), now_ms)
            .await
    }

    /// 请求成功拿到了候选。保留记录，但不再阻塞同一编号的后续判断。
    pub async fn mark_selection_rewrite_succeeded(
        &self,
        request_id: &str,
        now_ms: i64,
    ) -> Result<(), PortError> {
        self.update_selection_rewrite_state(request_id, "succeeded", None, now_ms)
            .await
    }

    /// 请求可能已经发出，但没有拿到可确认的结果（超时、连接中断、进程退出）。
    pub async fn mark_selection_rewrite_unknown(
        &self,
        request_id: &str,
        error_summary: &str,
        now_ms: i64,
    ) -> Result<(), PortError> {
        self.update_selection_rewrite_state(request_id, "unknown", Some(error_summary), now_ms)
            .await
    }

    pub async fn selection_rewrite_attempt(
        &self,
        request_id: &str,
    ) -> Result<Option<SelectionRewriteAttemptView>, PortError> {
        let row = sqlx::query(
            "SELECT request_id, novel_id, chapter_id, state, error_summary, created_at, updated_at
             FROM chapter_selection_rewrite_attempt WHERE request_id = ?",
        )
        .bind(request_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sql)?;
        row.as_ref().map(attempt_from_row).transpose()
    }

    /// 章节内仍处于「未知」的请求：界面要在重新生成前明确提示。
    pub async fn list_unknown_selection_rewrites(
        &self,
        novel_id: &str,
        chapter_id: &str,
    ) -> Result<Vec<SelectionRewriteAttemptView>, PortError> {
        let rows = sqlx::query(
            "SELECT request_id, novel_id, chapter_id, state, error_summary, created_at, updated_at
             FROM chapter_selection_rewrite_attempt
             WHERE novel_id = ? AND chapter_id = ? AND state IN ('sending', 'unknown')
             ORDER BY updated_at DESC",
        )
        .bind(novel_id)
        .bind(chapter_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sql)?;
        rows.iter().map(attempt_from_row).collect()
    }

    async fn update_selection_rewrite_state(
        &self,
        request_id: &str,
        state: &str,
        error_summary: Option<&str>,
        now_ms: i64,
    ) -> Result<(), PortError> {
        sqlx::query(
            "UPDATE chapter_selection_rewrite_attempt
             SET state = ?, error_summary = ?, updated_at = ?
             WHERE request_id = ?",
        )
        .bind(state)
        .bind(error_summary)
        .bind(now_ms)
        .bind(request_id)
        .execute(&self.pool)
        .await
        .map_err(map_sql)?;
        Ok(())
    }
}

fn attempt_from_row(
    row: &sqlx::sqlite::SqliteRow,
) -> Result<SelectionRewriteAttemptView, PortError> {
    let state: String = row.try_get("state").map_err(map_sql)?;
    Ok(SelectionRewriteAttemptView {
        request_id: row.try_get("request_id").map_err(map_sql)?,
        novel_id: row.try_get("novel_id").map_err(map_sql)?,
        chapter_id: row.try_get("chapter_id").map_err(map_sql)?,
        state: SelectionRewriteAttemptState::parse(&state),
        error_summary: row.try_get("error_summary").map_err(map_sql)?,
        created_at: row.try_get("created_at").map_err(map_sql)?,
        updated_at: row.try_get("updated_at").map_err(map_sql)?,
    })
}
