//! 伏笔生命周期与剧情因果。
//!
//! 复用既有 `plot_thread` 表作为唯一权威：`status` 就是生命周期，不另建第二套伏笔模型。
//! 每次状态转换都要求真实章节证据、版本 CAS 与 commandId 幂等；来源失效时进入 stale，
//! 必须由作者选取新证据才能继续。

use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::PortError;

pub const PLOT_STATUS_OPEN: &str = "open";
pub const PLOT_STATUS_DEVELOPING: &str = "developing";
pub const PLOT_STATUS_RESOLVED: &str = "resolved";
pub const PLOT_STATUS_DEFERRED: &str = "deferred";
pub const PLOT_STATUS_ABANDONED: &str = "abandoned";
pub const PLOT_STATUS_STALE: &str = "stale";
pub const PLOT_STATUS_SUPERSEDED: &str = "superseded";

pub const PLOT_TRANSITION_DEVELOP: &str = "develop";
pub const PLOT_TRANSITION_PAYOFF: &str = "payoff";
pub const PLOT_TRANSITION_DEFER: &str = "defer";
pub const PLOT_TRANSITION_ABANDON: &str = "abandon";
pub const PLOT_TRANSITION_RECOVER: &str = "recover";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlotThreadView {
    pub id: String,
    pub novel_id: String,
    pub title: String,
    pub body: String,
    /// 作者看到的状态：来源失效时统一显示 stale。
    pub status: String,
    /// 截至本章的生命周期状态（来源失效前的 resolved/developing 等）。
    pub lifecycle_status: String,
    /// 数据库中的原始状态；失效流程可能已把它改成 stale。
    pub stored_status: String,
    pub opened_chapter_order: i32,
    pub last_development_chapter_order: Option<i32>,
    pub resolved_chapter_order: Option<i32>,
    /// 最近一次生命周期动作所在的当前章节顺序；查看更早章节时界面不提供动作。
    pub latest_event_chapter_order: Option<i32>,
    pub source_chapter_id: Option<String>,
    pub source_chapter_order: Option<i32>,
    pub source_chapter_title: Option<String>,
    pub source_revision: Option<i64>,
    pub source_current_version: Option<i64>,
    pub evidence: Option<String>,
    pub evidence_present_in_current: bool,
    pub source_valid: bool,
    pub version: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlotThreadEventView {
    pub id: String,
    pub plot_id: String,
    pub title: String,
    pub kind: String,
    pub chapter_id: String,
    pub chapter_order: i32,
    pub chapter_title: Option<String>,
    pub evidence: String,
    pub from_status: Option<String>,
    pub to_status: String,
    /// 事件来源章节的当前正式版本是否仍等于记录版本，且证据仍在正文中。
    pub source_valid: bool,
    pub created_at: i64,
}

#[derive(Debug, Clone)]
pub struct CreatePlotThreadInput {
    pub id: String,
    pub novel_id: String,
    pub title: String,
    pub body: String,
    pub chapter_id: String,
    pub source_revision: i64,
    pub evidence: String,
    pub command_id: String,
    pub request_hash: String,
    pub now_ms: i64,
}

#[derive(Debug, Clone)]
pub struct PlotTransition {
    pub plot_id: String,
    pub novel_id: String,
    pub expected_version: i64,
    pub action: String,
    pub chapter_id: String,
    pub source_revision: i64,
    pub evidence: String,
    pub command_id: String,
    pub request_hash: String,
    pub now_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CausalityEdgeView {
    pub id: String,
    pub novel_id: String,
    pub from_kind: String,
    pub from_id: String,
    pub from_label: Option<String>,
    /// 起点/终点实体仍然存在且未被替代；章节删除或事实失效后为 false。
    pub from_valid: bool,
    pub to_kind: String,
    pub to_id: String,
    pub to_label: Option<String>,
    pub to_valid: bool,
    pub relation: String,
    pub evidence: String,
    pub source_chapter_id: String,
    pub source_chapter_order: Option<i32>,
    pub source_revision: i64,
    pub source_current_version: Option<i64>,
    pub source_valid: bool,
    pub status: String,
    pub version: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone)]
pub struct NewCausalityEdge {
    pub id: String,
    pub novel_id: String,
    pub from_kind: String,
    pub from_id: String,
    pub to_kind: String,
    pub to_id: String,
    pub relation: String,
    pub chapter_id: String,
    pub source_revision: i64,
    pub evidence: String,
    pub now_ms: i64,
}

#[async_trait]
pub trait PlotRepository: Send + Sync {
    async fn create_plot_thread(
        &self,
        input: CreatePlotThreadInput,
    ) -> Result<PlotThreadView, PortError>;
    async fn transition_plot_thread(
        &self,
        input: PlotTransition,
    ) -> Result<PlotThreadView, PortError>;
    async fn list_plot_threads(
        &self,
        novel_id: &str,
        as_of_chapter_order: i32,
    ) -> Result<Vec<PlotThreadView>, PortError>;
    async fn list_plot_events(
        &self,
        novel_id: &str,
        as_of_chapter_order: i32,
        limit: i64,
    ) -> Result<Vec<PlotThreadEventView>, PortError>;
    async fn create_causality_edge(
        &self,
        input: NewCausalityEdge,
    ) -> Result<CausalityEdgeView, PortError>;
    async fn list_causality_edges(
        &self,
        novel_id: &str,
    ) -> Result<Vec<CausalityEdgeView>, PortError>;
}

/// 时间线按时间升序输入，保留最近 `limit` 条；返回是否发生截断。
/// 截断必须丢弃最早条目，不能把“最近 500 条”做成“最旧 500 条”。
pub fn truncate_to_most_recent<T>(items: &mut Vec<T>, limit: usize) -> bool {
    if items.len() <= limit {
        return false;
    }
    let drop_count = items.len() - limit;
    items.drain(0..drop_count);
    true
}

pub struct PlotService {
    repository: Arc<dyn PlotRepository>,
}

impl PlotService {
    pub fn new(repository: Arc<dyn PlotRepository>) -> Self {
        Self { repository }
    }

    pub async fn create(
        &self,
        input: CreatePlotThreadInput,
    ) -> Result<PlotThreadView, crate::error::AppError> {
        if input.title.trim().is_empty() || input.body.trim().is_empty() {
            return Err(crate::error::AppError::validation("伏笔需要标题和描述。"));
        }
        if input.evidence.trim().is_empty() {
            return Err(crate::error::AppError::validation(
                "伏笔必须有真实章节证据。",
            ));
        }
        Ok(self.repository.create_plot_thread(input).await?)
    }

    pub async fn transition(
        &self,
        input: PlotTransition,
    ) -> Result<PlotThreadView, crate::error::AppError> {
        if !matches!(
            input.action.as_str(),
            PLOT_TRANSITION_DEVELOP
                | PLOT_TRANSITION_PAYOFF
                | PLOT_TRANSITION_DEFER
                | PLOT_TRANSITION_ABANDON
                | PLOT_TRANSITION_RECOVER
        ) {
            return Err(crate::error::AppError::validation("未知的伏笔动作。"));
        }
        if input.evidence.trim().is_empty() {
            return Err(crate::error::AppError::validation(
                "伏笔状态转换必须有当前章节的真实证据。",
            ));
        }
        Ok(self.repository.transition_plot_thread(input).await?)
    }

    pub async fn list(
        &self,
        novel_id: &str,
        as_of_chapter_order: i32,
    ) -> Result<Vec<PlotThreadView>, crate::error::AppError> {
        Ok(self
            .repository
            .list_plot_threads(novel_id, as_of_chapter_order)
            .await?)
    }

    pub async fn events(
        &self,
        novel_id: &str,
        as_of_chapter_order: i32,
        limit: i64,
    ) -> Result<Vec<PlotThreadEventView>, crate::error::AppError> {
        Ok(self
            .repository
            .list_plot_events(novel_id, as_of_chapter_order, limit)
            .await?)
    }

    pub async fn create_edge(
        &self,
        input: NewCausalityEdge,
    ) -> Result<CausalityEdgeView, crate::error::AppError> {
        if !matches!(
            input.from_kind.as_str(),
            "timeline_event" | "plot_thread" | "structured_fact"
        ) || !matches!(
            input.to_kind.as_str(),
            "timeline_event" | "plot_thread" | "structured_fact"
        ) {
            return Err(crate::error::AppError::validation(
                "因果关系只能引用事件、伏笔或已确认事实。",
            ));
        }
        if input.from_kind == input.to_kind && input.from_id == input.to_id {
            return Err(crate::error::AppError::validation("事件不能依赖自身。"));
        }
        if input.relation.trim().is_empty() || input.evidence.trim().is_empty() {
            return Err(crate::error::AppError::validation(
                "因果关系需要关系说明和真实证据。",
            ));
        }
        Ok(self.repository.create_causality_edge(input).await?)
    }

    pub async fn edges(
        &self,
        novel_id: &str,
    ) -> Result<Vec<CausalityEdgeView>, crate::error::AppError> {
        Ok(self.repository.list_causality_edges(novel_id).await?)
    }
}

#[cfg(test)]
mod tests {
    use super::truncate_to_most_recent;

    #[test]
    fn truncation_keeps_the_newest_entries() {
        let mut entries: Vec<i32> = (1..=5).collect();
        assert!(truncate_to_most_recent(&mut entries, 3));
        assert_eq!(entries, vec![3, 4, 5]);

        let mut exact: Vec<i32> = (1..=3).collect();
        assert!(!truncate_to_most_recent(&mut exact, 3));
        assert_eq!(exact, vec![1, 2, 3]);

        let mut empty: Vec<i32> = Vec::new();
        assert!(!truncate_to_most_recent(&mut empty, 3));
    }
}
