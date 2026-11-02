import assert from "node:assert/strict";
import test from "node:test";
import {
  requiresRetryConfirmation,
  rewriteBillingOutcome,
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

test("发送前的校验与凭据错误可以免确认重试", () => {
  for (const code of ["VALIDATION", "NOT_FOUND", "STORAGE_CORRUPT", "INTEGRITY_ERROR"]) {
    assert.equal(rewriteBillingOutcome({ code }), "never_sent", `${code} 属于发送前拒绝`);
  }
  assert.equal(rewriteRetryWarning("never_sent"), null, "没发出去的请求不需要计费提示");
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

test("重新生成必须经过明确确认的场景", () => {
  // 结果未知：即使界面已经刷新，也要先确认。
  assert.equal(requiresRetryConfirmation("unknown", false), true);
  // 章节里还有未确认的请求：不允许无感重发。
  assert.equal(requiresRetryConfirmation(null, true), true);
  // 明确没发出：直接重试即可。
  assert.equal(requiresRetryConfirmation("never_sent", false), false);
});
