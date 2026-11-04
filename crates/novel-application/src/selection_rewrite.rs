//! 选区改写的请求尝试记录：只表达「发没发出去、结果是否可确认」。
//!
//! 选区改写不建 Run、不写候选表，因此无法借用 RunContract 的计费语义。
//! 这里用最小独立记录区分三种状态，供命令层与界面判断能否重试：
//! - `NotSent`：请求没有发出去，重试不会产生新费用；
//! - `Sending` / `Unknown`：请求可能已经到达模型服务，不能自动重发；
//! - `Failed`：服务端明确报错，结果可确认，但通常仍可能已经计费；
//! - `Succeeded`：已经拿到候选。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectionRewriteAttemptState {
    /// 请求已提交，可能已经发出。
    Sending,
    /// 请求确定没有发出（模型调用在发送前就被拒绝）。
    NotSent,
    /// 已发出且拿到明确失败结果。
    Failed,
    /// 已发出但没有拿到可确认的结果（超时、连接中断、进程退出）。
    Unknown,
    /// 成功拿到候选。
    Succeeded,
}

impl SelectionRewriteAttemptState {
    pub fn parse(raw: &str) -> Self {
        match raw {
            "not_sent" => Self::NotSent,
            "failed" => Self::Failed,
            "unknown" => Self::Unknown,
            "succeeded" => Self::Succeeded,
            _ => Self::Sending,
        }
    }

    /// 计费状态无法确认：界面不能把它描述成「一定没扣费」。
    pub fn may_have_been_billed(&self) -> bool {
        matches!(self, Self::Sending | Self::Unknown | Self::Failed)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionRewriteAttemptView {
    pub request_id: String,
    pub novel_id: String,
    pub chapter_id: String,
    pub state: SelectionRewriteAttemptState,
    /// 面向排查的简短原因，不含正文、提示词或凭据。
    pub error_summary: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_definitely_unsubmitted_attempts_are_free_to_retry() {
        assert!(!SelectionRewriteAttemptState::NotSent.may_have_been_billed());
        assert!(SelectionRewriteAttemptState::Sending.may_have_been_billed());
        assert!(SelectionRewriteAttemptState::Unknown.may_have_been_billed());
        assert!(SelectionRewriteAttemptState::Failed.may_have_been_billed());
    }

    #[test]
    fn unknown_state_parsing_falls_back_to_conservative_sending() {
        assert_eq!(
            SelectionRewriteAttemptState::parse("unknown"),
            SelectionRewriteAttemptState::Unknown
        );
        assert_eq!(
            SelectionRewriteAttemptState::parse("something-new"),
            SelectionRewriteAttemptState::Sending
        );
    }
}
