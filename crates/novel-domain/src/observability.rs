//! LLM 调用观测的稳定契约。
//!
//! 一次物理 Provider 请求产生一条调用记录，记录「这次调用走到哪一步」：
//! 成功、明确未计费的失败、结果未知但可能已计费的失败，以及被重试覆盖的失败。
//!
//! 这里只描述计费可追溯性，不承载正文、提示词或业务质量判断。

use serde::{Deserialize, Serialize};

/// 一条调用记录的结果分类。
///
/// 与 `status` 的区别：`status` 是给用户看的结果摘要（succeeded / unknown / …），
/// `audit_status` 保留更细的工程分类，便于排查「为什么这条调用没有正文」。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvocationOutcome {
    /// 物理调用成功返回。
    Succeeded,
    /// 结构化格式修复后的重试成功。
    Repaired,
    /// 结构化格式修复后仍然失败。
    RepairFailed,
    /// 鉴权失败。请求没有被服务端受理，不产生费用。
    Unauthorized,
    /// 限流。本轮不会继续消耗预算。
    RateLimited,
    /// 超时。请求可能已经到达服务端。
    TimedOut,
    /// 调用方取消。
    Cancelled,
    /// 请求已经发出但结果不可用（连接中断、服务端错误等）。
    ProviderFailed,
    /// 请求在发出前就被拒绝（参数、预算或端点校验）。
    Rejected,
}

impl InvocationOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Repaired => "repaired",
            Self::RepairFailed => "repair_failed",
            Self::Unauthorized => "unauthorized",
            Self::RateLimited => "rate_limited",
            Self::TimedOut => "timed_out",
            Self::Cancelled => "cancelled",
            Self::ProviderFailed => "provider_failed",
            Self::Rejected => "rejected",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "succeeded" => Some(Self::Succeeded),
            "repaired" => Some(Self::Repaired),
            "repair_failed" => Some(Self::RepairFailed),
            "unauthorized" => Some(Self::Unauthorized),
            "rate_limited" => Some(Self::RateLimited),
            "timed_out" => Some(Self::TimedOut),
            "cancelled" => Some(Self::Cancelled),
            "provider_failed" => Some(Self::ProviderFailed),
            "rejected" => Some(Self::Rejected),
            _ => None,
        }
    }

    /// 用量是否已经由 Provider 确认。
    pub fn usage_state(self) -> InvocationUsageState {
        match self {
            Self::Succeeded | Self::Repaired => InvocationUsageState::Known,
            // 请求可能已经到达服务端：费用未知，不能当 0。
            Self::TimedOut | Self::ProviderFailed => InvocationUsageState::Unknown,
            // 明确没有被受理。
            Self::Unauthorized | Self::RateLimited | Self::Rejected => {
                InvocationUsageState::Unbilled
            }
            // 取消可能发生在请求已经发出之后：无法证明未计费，只能标记未知。
            Self::Cancelled => InvocationUsageState::Unknown,
            // 已明知失败，但修复请求可能已经计费。
            Self::RepairFailed => InvocationUsageState::Unknown,
        }
    }
}

/// 一次物理调用的费用可见性。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvocationUsageState {
    /// Provider 返回了用量，可以参与费用汇总。
    Known,
    /// 请求可能已计费，但结果未知；不能计为 0。
    Unknown,
    /// 明确没有产生费用。
    Unbilled,
}

impl InvocationUsageState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Known => "known",
            Self::Unknown => "unknown",
            Self::Unbilled => "unbilled",
        }
    }

    /// 旧数据没有这条信息时，按是否拿到用量推断。
    ///
    /// 旧版本只在成功路径写记录，所以「有记录但没有用量」说明当时 Provider
    /// 没有返回 usage，仍然是未知，不能当成 0 计费。
    pub fn for_legacy_row(prompt_tokens: Option<i64>, completion_tokens: Option<i64>) -> Self {
        if prompt_tokens.is_some() || completion_tokens.is_some() {
            Self::Known
        } else {
            Self::Unknown
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_receipts_that_may_have_billed_are_unknown() {
        assert_eq!(
            InvocationOutcome::ProviderFailed.usage_state(),
            InvocationUsageState::Unknown
        );
        assert_eq!(
            InvocationOutcome::TimedOut.usage_state(),
            InvocationUsageState::Unknown
        );
        assert_eq!(
            InvocationOutcome::RepairFailed.usage_state(),
            InvocationUsageState::Unknown
        );
        assert_eq!(
            InvocationOutcome::Unauthorized.usage_state(),
            InvocationUsageState::Unbilled
        );
        // 取消可能发生在请求已发出之后，不能断言未计费。
        assert_eq!(
            InvocationOutcome::Cancelled.usage_state(),
            InvocationUsageState::Unknown
        );
        assert_eq!(
            InvocationOutcome::Rejected.usage_state(),
            InvocationUsageState::Unbilled
        );
        assert_eq!(
            InvocationOutcome::Succeeded.usage_state(),
            InvocationUsageState::Known
        );
        assert_eq!(
            InvocationOutcome::Repaired.usage_state(),
            InvocationUsageState::Known
        );
    }

    #[test]
    fn legacy_rows_without_usage_are_not_silently_zero() {
        assert_eq!(
            InvocationUsageState::for_legacy_row(None, None),
            InvocationUsageState::Unknown
        );
        assert_eq!(
            InvocationUsageState::for_legacy_row(Some(0), Some(0)),
            InvocationUsageState::Known
        );
    }

    #[test]
    fn outcome_strings_round_trip() {
        for outcome in [
            InvocationOutcome::Succeeded,
            InvocationOutcome::Repaired,
            InvocationOutcome::RepairFailed,
            InvocationOutcome::Unauthorized,
            InvocationOutcome::RateLimited,
            InvocationOutcome::TimedOut,
            InvocationOutcome::Cancelled,
            InvocationOutcome::ProviderFailed,
            InvocationOutcome::Rejected,
        ] {
            assert_eq!(InvocationOutcome::parse(outcome.as_str()), Some(outcome));
        }
        assert_eq!(InvocationOutcome::parse("completed"), None);
    }
}
