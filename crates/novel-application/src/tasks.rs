//! 任务中心 3.0a 阶段用例。
//!
//! 设计取舍：见 `docs/wiki/architecture/backend-rustization-roadmap.md`。
//!
//! 3.0a 阶段只暴露两个只读端点：
//! - `get_overview`：顶部数字概览
//! - `list_recovery_candidates`：待恢复任务清单
//!
//! 数据源走 `TaskCenterRepository` port。**列表 / 详情 / 重试 / 取消 / 归档 5 类写操作
//! 在 Rust 化前不实现**，对应的 HTTP 路由会落到 `not_implemented` 兜底，等 3.0b~f 完成后再补。
//!
//! 注意：3.0a 范围内 `RecoverableTaskSummary.kind` 只会有 `novel_workflow`。
//! 其他 5 类业务（拆书 / 流水线 / 图像 / 知识 / 风格提取）的统计和恢复候选项保持 0 / 空，
//! 前端需要识别这个"部分可见"的过渡期状态（不报错，但显示"还在迁入中"提示）。

use std::sync::Arc;

use crate::dto::{RecoverableTaskListResponse, TaskOverviewSummary};
use crate::error::AppError;
use crate::ports::TaskCenterRepository;

pub struct TaskCenterService {
    repository: Arc<dyn TaskCenterRepository>,
}

impl TaskCenterService {
    pub fn new(repository: Arc<dyn TaskCenterRepository>) -> Self {
        Self { repository }
    }

    /// 顶部数字概览。3.0a 阶段只统计 auto_director run + quality_debt，其他类目返回 0。
    pub async fn get_overview(&self) -> Result<TaskOverviewSummary, AppError> {
        Ok(self.repository.task_overview().await?)
    }

    /// 待恢复任务清单。3.0a 阶段只包含 auto_director run 中的 paused 项。
    pub async fn list_recovery_candidates(&self) -> Result<RecoverableTaskListResponse, AppError> {
        let items = self.repository.list_recovery_candidates().await?;
        // 防御性：仓储层应该已经按 updated_at desc 排好，这里再排一次避免外部依赖。
        let mut items = items;
        items.sort_by(|left, right| right.id.cmp(&left.id));
        Ok(RecoverableTaskListResponse { items })
    }
}
