import assert from "node:assert/strict";
import test from "node:test";
import {
  requiresRetryConfirmation,
  rewriteBillingOutcome,
  rewriteFailureMessage,
  rewriteRetryWarning,
} from "./chapterSelectionRewriteBilling.ts";

// ---- P17.2 P1.3：计费状态未知的边界 ----

test("超时与连接中断归为结果未知，而不是「一定没扣费」", () => {
  assert.equal(rewriteBillingOutcome({ code: "MODEL_RESULT_UNKNOWN", message: "x" }), "unknown");
  const warning = rewriteRetryWarning("unknown");
  assert.ok(warning, "结果未知时必须给出重试提示");
  assert.match(warning, /可能已经发送/);
  assert.match(warning, /无法确认/);
  assert.doesNotMatch(warning, /没有产生费用|不会计费/);
});

// ---- P17.2 RC1 P1：模型已返回但输出不可用 ----

test("模型已返回但输出不可用时，必须提示再次调用可能收费", () => {
  const error = {
    code: "MODEL_RESULT_UNUSABLE",
    message: "模型返回的改写结果无法解析。这次调用已经产生费用；重新生成会再次调用模型。",
    retryable: false,
    details: { sentState: "SENT_BUT_UNUSABLE" },
  };
  const outcome = rewriteBillingOutcome(error);
  assert.equal(outcome, "billed_failure", "已经调用过模型，不能按「没发出去」处理");

  const message = rewriteFailureMessage(error, outcome);
  assert.match(message, /可能已经产生费用/, "失败提示必须说明这次调用可能已计费");
  assert.match(message, /再次调用模型/, "提示必须说明重新生成会再次调用模型");
  assert.match(rewriteRetryWarning(outcome) ?? "", /再次调用模型/);
  assert.equal(requiresRetryConfirmation(outcome, false), true, "重新生成必须先确认");
});

test("发送状态优先于错误码，无法确认时保守按可能计费处理", () => {
  // 旧内核可能用 VALIDATION 表示「已发送但不可用」：状态字段要能覆盖错误码。
  assert.equal(
    rewriteBillingOutcome({ code: "VALIDATION", details: { sentState: "SENT_BUT_UNUSABLE" } }),
    "billed_failure",
  );
  assert.equal(
    rewriteBillingOutcome({ code: "VALIDATION", details: { sentState: "NOT_SENT" } }),
    "never_sent",
  );
  assert.equal(
    rewriteBillingOutcome({ code: "MODEL_RESULT_UNKNOWN", details: { sentState: "RESULT_UNKNOWN" } }),
    "unknown",
  );
  // 认不出的状态不能当成免费：保守提示可能计费。
  assert.equal(
    rewriteBillingOutcome({ code: "VALIDATION", details: { sentState: "SOMETHING_NEW" } }),
    "billed_failure",
  );
  // 旧错误没有状态字段时不能假设免费：VALIDATION 也可能是已发送后的解析失败。
  assert.equal(rewriteBillingOutcome({ code: "VALIDATION" }), "billed_failure");
  assert.equal(rewriteBillingOutcome({ code: "NOT_FOUND" }), "billed_failure");
  assert.equal(rewriteBillingOutcome({ code: "MODEL_RESULT_UNUSABLE" }), "billed_failure");
});

test("与内核序列化契约一致：错误码与发送状态都能直接读", () => {
  // 形状来自 src-tauri/src/commands/editor.rs 的 AppError 序列化。
  const serialized = JSON.parse(
    '{"code":"MODEL_RESULT_UNUSABLE","message":"模型没有给出可用的改写候选。","retryable":false,"details":{"sentState":"SENT_BUT_UNUSABLE"}}',
  );
  assert.equal(rewriteBillingOutcome(serialized), "billed_failure");

  const notSent = JSON.parse(
    '{"code":"VALIDATION","message":"还没有填写模型密钥。","retryable":false,"details":{"sentState":"NOT_SENT"}}',
  );
  assert.equal(rewriteBillingOutcome(notSent), "never_sent");
});

test("带 NOT_SENT 的发送前错误可以免确认重试", () => {
  for (const code of ["VALIDATION", "NOT_FOUND", "STORAGE_CORRUPT", "INTEGRITY_ERROR", "CONFLICT"]) {
    assert.equal(
      rewriteBillingOutcome({ code, details: { sentState: "NOT_SENT" } }),
      "never_sent",
      `${code} 带 NOT_SENT 时属于发送前拒绝`,
    );
  }
  assert.equal(rewriteRetryWarning("never_sent"), null, "没发出去的请求不需要计费提示");
});

test("没有发送状态的旧错误结构不能声称免费", () => {
  // 旧内核可能把「已发送但解析失败」报成 VALIDATION；无法区分时按可能计费处理。
  assert.equal(rewriteBillingOutcome({ code: "VALIDATION" }), "billed_failure");
  assert.match(rewriteRetryWarning("billed_failure") ?? "", /可能已经产生费用/);
});

test("竞争与限流按可能已计费处理，不能声称免费", () => {
  assert.equal(rewriteBillingOutcome({ code: "CONFLICT" }), "billed_failure");
  assert.equal(rewriteBillingOutcome({ code: "MODEL_RATE_LIMIT" }), "billed_failure");
  assert.match(rewriteRetryWarning("billed_failure"), /可能已经产生费用/);
});

test("未知错误不能默认成免费重试", () => {
  assert.equal(rewriteBillingOutcome(undefined), "billed_failure");
  assert.equal(rewriteBillingOutcome("plain string"), "billed_failure");
  assert.equal(rewriteBillingOutcome({ message: "no code" }), "billed_failure");
});

test("Tauri 普通对象错误要读到真实原因，而不是只剩兜底文案", () => {
  // Tauri invoke 拒绝时抛的是普通对象，不是 Error 实例。
  const message = rewriteFailureMessage(
    { code: "VALIDATION", message: "还没有填写模型密钥。" },
    "never_sent",
  );
  assert.match(message, /还没有填写模型密钥/);
  assert.doesNotMatch(message, /AI 修正失败/);
  // Error 实例仍然按原消息展示。
  assert.match(rewriteFailureMessage(new Error("网络断开"), "never_sent"), /网络断开/);
  // 两者都没有时才用兜底文案。
  assert.match(rewriteFailureMessage(null, "never_sent"), /AI 修正失败/);
});

test("重新生成必须经过明确确认的场景", () => {
  // 结果未知：即使界面已经刷新，也要先确认。
  assert.equal(requiresRetryConfirmation("unknown", false), true);
  // 章节里还有未确认的请求：不允许无感重发。
  assert.equal(requiresRetryConfirmation(null, true), true);
  // 明确没发出：直接重试即可。
  assert.equal(requiresRetryConfirmation("never_sent", false), false);
});
