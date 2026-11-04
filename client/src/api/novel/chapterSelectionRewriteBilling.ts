/**
 * 选区改写的计费状态语义（纯逻辑，便于确定性测试）。
 *
 * 选区改写不建 Run，因此「这次调用会不会再扣一次费」必须由界面自己判断：
 * - 请求没有发出 → 重试是免费的；
 * - 结果未知（超时、连接中断、进程退出） → 不能自动重发，必须让作者先确认；
 * - 明确失败但已经产生费用 → 可以重试，但要说明会再次调用模型。
 */

/** 一次选区改写失败时，能确认到什么程度。 */
export type RewriteBillingOutcome =
  /** 请求没有离开本机：可以免确认重试。 */
  | "never_sent"
  /** 请求已经到达模型服务，但结果无法确认。 */
  | "unknown"
  /** 服务端明确报错或返回不可用结果，这次可能已经计费。 */
  | "billed_failure";

/** 内核写进 `error.details.sentState` 的发送状态。 */
const SENT_STATE_OUTCOMES: Record<string, RewriteBillingOutcome> = {
  NOT_SENT: "never_sent",
  SENT_BUT_UNUSABLE: "billed_failure",
  RESULT_UNKNOWN: "unknown",
};

/** 读取机器可读的发送状态；缺失或形状不对时返回 null，交给错误码兜底。 */
function sentStateOutcome(record: Record<string, unknown>): RewriteBillingOutcome | null {
  const details = record.details;
  if (!details || typeof details !== "object") {
    return null;
  }
  const sentState = (details as Record<string, unknown>).sentState;
  if (typeof sentState !== "string") {
    return null;
  }
  // 认不出的发送状态不能当成「没发出去」：按可能计费处理。
  return SENT_STATE_OUTCOMES[sentState] ?? "billed_failure";
}

/**
 * 把内核错误归到计费语义上。
 *
 * 优先读内核给出的 `details.sentState`：它描述的是「请求到底有没有发出去」，
 * 与错误码和中文文案都解耦。只有旧错误没有这个字段时才回退到错误码。
 * 任何无法确认的情况都按「可能已计费」处理，不能声称免费重试。
 */
export function rewriteBillingOutcome(error: unknown): RewriteBillingOutcome {
  const record = error && typeof error === "object" ? (error as Record<string, unknown>) : {};
  const fromSentState = sentStateOutcome(record);
  if (fromSentState) {
    return fromSentState;
  }
  // 模型已经返回但输出不可用：已经调用过模型，不能当成发送前校验失败。
  if (record.code === "MODEL_RESULT_UNUSABLE") {
    return "billed_failure";
  }
  if (record.code === "MODEL_RESULT_UNKNOWN") {
    return "unknown";
  }
  // 发送前的校验、凭据与共用的路由校验：这些错误不会产生模型费用。
  if (
    record.code === "VALIDATION"
    || record.code === "NOT_FOUND"
    || record.code === "STORAGE_CORRUPT"
    || record.code === "INTEGRITY_ERROR"
  ) {
    return "never_sent";
  }
  // 竞争、限流等发生在请求发出之后：保留「可能计费」的保守表述。
  return "billed_failure";
}

/** 重新生成前要展示的提示；结果未知时不能写成「一定没扣费」。 */
export function rewriteRetryWarning(outcome: RewriteBillingOutcome): string | null {
  switch (outcome) {
    case "never_sent":
      return null;
    case "unknown":
      return "上一次改写可能已经发送到模型服务，结果无法确认。重新生成会再次调用模型，可能产生新的费用。";
    case "billed_failure":
      return "上一次改写可能已经产生费用。重新生成会再次调用模型。";
  }
}

/** 请求正在等待结果时，界面不能把它渲染成失败。 */
export function isRewriteInFlight(pendingCount: number): boolean {
  return pendingCount > 0;
}

/**
 * 失败文案必须带上计费语义：结果未知时不能只说「失败，请重试」。
 * 这里不改写内核给出的具体原因，只补上作者需要知道的后果。
 *
 * Tauri 抛出的错误是普通对象（`{ code, message, retryable }`），不是 `Error` 实例；
 * 两种形状都要读到真实原因，否则界面只剩一句兜底文案，排查时看不到内核说了什么。
 */
export function rewriteFailureMessage(error: unknown, outcome: RewriteBillingOutcome): string {
  const record = error && typeof error === "object" ? (error as Record<string, unknown>) : {};
  const fromRecord = typeof record.message === "string" ? record.message.trim() : "";
  const fromError = error instanceof Error ? error.message.trim() : "";
  const base = fromRecord || fromError || "AI 修正失败。";
  if (outcome === "unknown") {
    return `${base} 这次请求可能已经发送到模型服务，结果无法确认；确认后再决定是否重新生成。`;
  }
  if (outcome === "billed_failure") {
    return `${base} 上一次调用可能已经产生费用；重新生成会再次调用模型。`;
  }
  return base;
}

/** 同一章节存在未确认的改写时，重新生成必须先经过作者确认。 */
export function requiresRetryConfirmation(
  outcome: RewriteBillingOutcome | null,
  hasPendingAttempt: boolean,
): boolean {
  if (hasPendingAttempt) return true;
  return outcome === "unknown" || outcome === "billed_failure";
}
