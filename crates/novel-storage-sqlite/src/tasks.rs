//! 任务中心 3.0a 阶段的仓储实现。
//!
//! 范围：只读 `workflow_run` / `workflow_control` / `quality_debt` 三张 Rust kernel
//! 已有的表，对应 Node 端 `TaskCenterService` 中 auto_director run 的统计和恢复候选项。
//! 其他 5 类业务（拆书 / 流水线 / 图像 / 知识 / 风格提取）的统计在 3.0b~f 阶段补全。

use async_trait::async_trait;
use novel_application::{
    PortError, RecoverableTaskSummary, TaskCenterRepository, TaskOverviewSummary,
};
use sqlx::Row;

use crate::db::{SqliteKernel, map_sql};

#[async_trait]
impl TaskCenterRepository for SqliteKernel {
    async fn task_overview(&self) -> Result<TaskOverviewSummary, PortError> {
        // 状态分组：从 workflow_control 拿 auto_director run 的计数。
        // 候选 status 集合和 Node 端一致，方便前端零修改对接。
        let status_rows = sqlx::query(
            "SELECT status, COUNT(*) AS cnt
             FROM workflow_control
             WHERE status IN ('queued', 'running', 'failed', 'cancelled', 'waiting_approval')
             GROUP BY status",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sql)?;

        let mut overview = TaskOverviewSummary::default();
        for row in status_rows {
            let status: String = row.try_get("status").map_err(map_sql)?;
            let count: i64 = row.try_get("cnt").map_err(map_sql)?;
            match status.as_str() {
                "queued" => overview.queued_count = count,
                "running" => overview.running_count = count,
                "failed" => overview.failed_count = count,
                "cancelled" => overview.cancelled_count = count,
                "waiting_approval" => overview.waiting_approval_count = count,
                _ => {}
            }
        }

        // recovery 候选项数：auto_director run 中 paused 且有失败原因。
        // 这里和 list_recovery_candidates 的查询条件保持一致。
        let recovery_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)
             FROM workflow_control
             WHERE status = 'paused'
               AND failure_reason IS NOT NULL
               AND length(failure_reason) > 0",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_sql)?;
        overview.recovery_candidate_count = recovery_count;

        Ok(overview)
    }

    async fn list_recovery_candidates(
        &self,
    ) -> Result<Vec<RecoverableTaskSummary>, PortError> {
        // 主体查询：paused + 有 failure_reason 的 auto_director run。
        // 关联 novel 拿到标题和创作体验（用于 sourceRoute 决策）。
        // 排序用 workflow_run.created_at desc，因为 workflow_control 本身没有 updated_at。
        let rows = sqlx::query(
            "SELECT r.id AS run_id,
                    r.novel_id,
                    n.title AS novel_title,
                    n.creation_experience,
                    c.cursor_step_id,
                    c.failure_reason,
                    r.created_at
             FROM workflow_run r
             JOIN workflow_control c ON c.run_id = r.id
             JOIN novel n ON n.id = r.novel_id
             WHERE c.status = 'paused'
               AND c.failure_reason IS NOT NULL
               AND length(c.failure_reason) > 0
             ORDER BY r.created_at DESC, r.id DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sql)?;

        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            let run_id: String = row.try_get("run_id").map_err(map_sql)?;
            let novel_id: String = row.try_get("novel_id").map_err(map_sql)?;
            let novel_title: String = row.try_get("novel_title").map_err(map_sql)?;
            let creation_experience: String =
                row.try_get("creation_experience").map_err(map_sql)?;
            let cursor_step_id: Option<String> =
                row.try_get("cursor_step_id").map_err(map_sql)?;
            let failure_reason: String = row.try_get("failure_reason").map_err(map_sql)?;

            // 关联 quality_debt：取这条 run 的最近一条债作为 hint，比纯 failure_reason 更有上下文。
            let hint_row = sqlx::query(
                "SELECT detail FROM quality_debt
                 WHERE run_id = ?
                 ORDER BY created_at DESC, id DESC
                 LIMIT 1",
            )
            .bind(&run_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_sql)?;
            let debt_detail: Option<String> = match hint_row {
                Some(row) => Some(row.try_get("detail").map_err(map_sql)?),
                None => None,
            };

            let owner_label = if novel_title.trim().is_empty() {
                format!("未命名小说 {}", &novel_id[..novel_id.len().min(8)])
            } else {
                novel_title.trim().to_string()
            };

            let title = if creation_experience == "simple" {
                format!("自动导演（简易模式）：{owner_label}")
            } else {
                format!("自动导演（专业模式）：{owner_label}")
            };

            let source_route = if creation_experience == "simple" {
                format!("/novels/{novel_id}/simple")
            } else {
                format!("/novels/{novel_id}/edit?taskPanel=1")
            };

            let recovery_hint = debt_detail
                .filter(|detail| !detail.trim().is_empty())
                .unwrap_or_else(|| failure_reason.clone());

            items.push(RecoverableTaskSummary {
                id: run_id,
                kind: "novel_workflow".to_string(),
                title,
                owner_label,
                status: "queued".to_string(),
                current_stage: cursor_step_id,
                current_item_label: None,
                resume_action: "恢复自动导演".to_string(),
                source_route,
                recovery_hint,
            });
        }

        Ok(items)
    }
}
