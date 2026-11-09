//! Sync-A1-R2：Provider 失败的计费语义必须由错误来源决定，而不是错误字符串。
//!
//! `LlmError::InvalidResponse` 在三个完全不同的阶段被构造：
//!   * HTTP 状态码不可处理（请求已到达服务端，可能已计费）；
//!   * SSE 行不是 JSON（流已开始，服务端必然已经处理）；
//!   * 上游 error 事件（服务端已开始应答）。
//!
//! 旧映射把它们与「空密钥 / 预算预检」一起归为 `Rejected`（unbilled），
//! 于是真实发生过的请求被当成没花钱。本套件锁定七个场景的分类。

use novel_domain::{InvocationOutcome, InvocationUsageState};
use novel_llm::LlmError;

/// 与 `worker.rs` 中的映射保持一致的可测入口。
fn outcome(error: &LlmError) -> InvocationOutcome {
    novel_application::invocation_outcome_for_error(error)
}

#[test]
fn request_never_sent_is_unbilled() {
    // 空密钥、预算越界：请求没有离开进程。
    let case = LlmError::Unavailable {
        billed: false,
        reason: "还没有填写模型密钥".into(),
    };
    assert_eq!(outcome(&case), InvocationOutcome::Rejected);
    assert_eq!(outcome(&case).usage_state(), InvocationUsageState::Unbilled);
}

#[test]
fn preflight_rejections_are_unbilled() {
    for case in [
        LlmError::BudgetExceeded("本次生成超出预算".into()),
        LlmError::EndpointRejected("端点不被允许".into()),
    ] {
        assert_eq!(
            outcome(&case).usage_state(),
            InvocationUsageState::Unbilled,
            "发出前的拒绝不产生费用：{case:?}"
        );
    }
}

#[test]
fn http_auth_failure_is_unbilled() {
    let case = LlmError::Unauthorized;
    assert_eq!(outcome(&case), InvocationOutcome::Unauthorized);
    assert_eq!(outcome(&case).usage_state(), InvocationUsageState::Unbilled);
}

#[test]
fn unhandled_http_status_may_have_billed() {
    // 例如 404 / 400：请求确实到达了服务端，只是状态码没有被专门处理。
    let case = LlmError::InvalidResponse("模型返回了无法处理的状态 404".into());
    assert_eq!(
        outcome(&case),
        InvocationOutcome::ProviderFailed,
        "HTTP 层失败不能当作发出前拒绝"
    );
    assert_eq!(outcome(&case).usage_state(), InvocationUsageState::Unknown);
}

#[test]
fn sse_json_corruption_after_stream_start_may_have_billed() {
    // 流已经开始逐 token 返回，此时 JSON 破损：服务端必然已经处理过请求。
    let case = LlmError::StreamCorrupted("模型流不是 JSON".into());
    assert_eq!(outcome(&case), InvocationOutcome::ProviderFailed);
    assert_eq!(outcome(&case).usage_state(), InvocationUsageState::Unknown);
}

#[test]
fn provider_error_event_may_have_billed() {
    let case = LlmError::ProviderError("upstream overloaded".into());
    assert_eq!(outcome(&case), InvocationOutcome::ProviderFailed);
    assert_eq!(outcome(&case).usage_state(), InvocationUsageState::Unknown);
}

#[test]
fn partial_stream_then_break_may_have_billed() {
    let case = LlmError::Unavailable {
        billed: true,
        reason: "模型流在返回中途断开".into(),
    };
    assert_eq!(outcome(&case), InvocationOutcome::ProviderFailed);
    assert_eq!(outcome(&case).usage_state(), InvocationUsageState::Unknown);
}

#[test]
fn user_cancel_is_unknown_not_unbilled() {
    let case = LlmError::Cancelled;
    assert_eq!(outcome(&case), InvocationOutcome::Cancelled);
    assert_eq!(
        outcome(&case).usage_state(),
        InvocationUsageState::Unknown,
        "取消可能发生在请求已发出之后"
    );
}

#[test]
fn timeout_is_unknown() {
    let case = LlmError::Timeout;
    assert_eq!(outcome(&case), InvocationOutcome::TimedOut);
    assert_eq!(outcome(&case).usage_state(), InvocationUsageState::Unknown);
}

#[test]
fn rate_limit_is_unbilled() {
    let case = LlmError::RateLimit;
    assert_eq!(outcome(&case), InvocationOutcome::RateLimited);
    assert_eq!(outcome(&case).usage_state(), InvocationUsageState::Unbilled);
}

/// 分类必须来自错误变体，而不是错误文本。
///
/// 这段文本本身不含任何计费线索。把它放进「发出前拒绝」与「已发送后失败」
/// 两个变体，结论必须不同——如果实现改成对 message 做字符串匹配，这个断言会失败。
#[test]
fn classification_follows_variant_not_message_text() {
    let text = "模型流不是 JSON";
    // 同一句话，但一个明确表示请求没发出去，一个表示流已经开始。
    let not_sent = LlmError::Unavailable {
        billed: false,
        reason: text.into(),
    };
    let already_streaming = LlmError::StreamCorrupted(text.into());
    assert_eq!(
        outcome(&not_sent).usage_state(),
        InvocationUsageState::Unbilled
    );
    assert_eq!(
        outcome(&already_streaming).usage_state(),
        InvocationUsageState::Unknown
    );
    assert_ne!(
        outcome(&not_sent).usage_state(),
        outcome(&already_streaming).usage_state(),
        "同样的文本在不同阶段必须给出不同结论"
    );
}

/// 反过来：不同文本、同一个变体必须得到同一个结论。
#[test]
fn same_variant_classifies_identically_regardless_of_text() {
    for text in ["模型流不是 JSON", "完全不同的另一句话", ""] {
        assert_eq!(
            outcome(&LlmError::StreamCorrupted(text.into())).usage_state(),
            InvocationUsageState::Unknown
        );
    }
}
