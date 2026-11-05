/** 右侧工作区的纯逻辑：Run 动作契约、状态投影与页签偏好，不碰数据请求。 */

export interface CastItem {
  id: string;
  name: string;
  role?: string | null;
  storyFunction?: string | null;
  currentGoal?: string | null;
}

export type PanelTab = "revise" | "review" | "cast" | "plot" | "context" | "run";

const PANEL_TABS: PanelTab[] = ["revise", "review", "cast", "plot", "context", "run"];

/** 页签选择要跨章节保持：同一本书里切章不该把作者丢回默认页签。 */
export function readStoredPanelTab(novelId: string, storage?: Storage | null): PanelTab {
  try {
    const raw = storage?.getItem(`chapter-workbench-tab:${novelId}`);
    return PANEL_TABS.find((tab) => tab === raw) ?? "revise";
  } catch {
    return "revise";
  }
}

export function storePanelTab(novelId: string, tab: PanelTab, storage?: Storage | null): void {
  try {
    storage?.setItem(`chapter-workbench-tab:${novelId}`, tab);
  } catch {
    // 隐私模式或配额满时静默降级：页签偏好丢了不影响编辑。
  }
}

/** 与后端 Director 命令一一对应的动作；命令名就是真实 Tauri command。 */
export type RunActionKey =
  | "approve"
  | "reject"
  | "regenerate"
  | "confirm_candidate"
  | "resume"
  | "resume_quality_debt"
  | "cancel";

export type RunCommandName =
  | "approve_run"
  | "reject_run"
  | "regenerate_run"
  | "confirm_candidate_after_edit_run"
  | "resume_run"
  | "resume_run_with_quality_debt"
  | "cancel_run";

/**
 * 一个可以真的执行的下一步。
 *
 * 这里不用布尔量声明「需要哪些参数」：那种声明不参与发请求，改了请求忘了改声明也测不出来。
 * 每个命令实际发出的参数由 `buildRunCommandRequest` 按 `command` 派生，单测直接断言派生结果。
 */
export interface RunActionOption {
  key: RunActionKey;
  label: string;
  command: RunCommandName;
  destructive: boolean;
}

export interface RunCandidateSummary {
  id: string;
  content?: string | null;
  contentSha256?: string | null;
}

/** 候选正文齐备的形态：三项 CAS 参数都拿得到，才允许发确认命令。 */
export interface RunReadyCandidate extends RunCandidateSummary {
  content: string;
  contentSha256: string;
}

export interface RunActionContext {
  legalActions?: string[] | null;
  status?: string | null;
  gateId?: string | null;
  candidate?: RunCandidateSummary | null;
  controlVersion?: number | null;
}

const LABELS: Record<RunActionKey, string> = {
  approve: "确认这一步",
  reject: "退回重写",
  regenerate: "重新生成",
  confirm_candidate: "确认候选正文",
  resume: "继续这次创作",
  resume_quality_debt: "接受质量提醒并继续",
  cancel: "取消这次创作",
};

function candidateReady(candidate?: RunCandidateSummary | null): candidate is RunReadyCandidate {
  return Boolean(candidate && candidate.id && candidate.contentSha256 && (candidate.content ?? "").length > 0);
}

/**
 * 把后端 legalActions 翻译成「可以真的执行」的动作。
 *
 * 规则全部来自 Director 的取值：
 * - waiting_approval + gate → confirm / reject / regenerate / confirm_after_edit
 * - paused（安全停点） → cancel（+ resume_quality_debt）
 * - paused（普通） → resume / cancel
 * - running / queued → cancel
 *
 * 候选稿存在且当前是章节确认门点时，confirm / confirm_after_edit 一律走候选 CAS 命令，
 * 不能用 approve 绕过正文 CAS。
 */
export function availableRunActions(context: RunActionContext): RunActionOption[] {
  const actions = context.legalActions ?? [];
  const hasCandidate = candidateReady(context.candidate);
  const atCandidateGate = hasCandidate && (context.gateId ?? "") === "approval";
  const options: RunActionOption[] = [];
  const seen = new Set<RunActionKey>();

  const push = (key: RunActionKey, overrides: Partial<RunActionOption> = {}) => {
    if (seen.has(key)) return;
    seen.add(key);
    options.push({
      key,
      label: LABELS[key],
      command:
        key === "approve"
          ? "approve_run"
          : key === "reject"
            ? "reject_run"
            : key === "regenerate"
              ? "regenerate_run"
              : key === "confirm_candidate"
                ? "confirm_candidate_after_edit_run"
                : key === "resume"
                  ? "resume_run"
                  : key === "resume_quality_debt"
                    ? "resume_run_with_quality_debt"
                    : "cancel_run",
      destructive: key === "cancel" || key === "reject",
      ...overrides,
    });
  };

  for (const raw of actions) {
    switch (raw.trim()) {
      case "confirm":
        if (atCandidateGate) push("confirm_candidate");
        else push("approve");
        break;
      case "confirm_after_edit":
        // 没有候选就没有「编辑后确认」，不渲染不可执行的按钮。
        if (hasCandidate) push("confirm_candidate");
        break;
      case "reject":
        push("reject");
        break;
      case "regenerate":
        push("regenerate");
        break;
      case "resume":
        push("resume");
        break;
      case "resume_quality_debt":
        push("resume_quality_debt");
        break;
      case "cancel":
        push("cancel");
        break;
      default:
        // 后端未来新增的动作不认识就不渲染，避免出现点了会失败的按钮。
        break;
    }
  }
  return options;
}

/**
 * 每个动作实际发出的请求。参数形状与 `src-tauri/src/commands/run.rs` 的命令签名一一对应：
 * - approve / reject / regenerate / confirm_candidate_after_edit 接 `expected_version`（Option）
 * - resume 只接 `accept_candidate`，cancel 只接 run 与 command——两者都**不**接 `expected_version`
 * - resume_run_with_quality_debt 必须带 reason
 */
export type RunCommandRequest =
  | { command: "approve_run"; payload: { runId: string; commandId: string; expectedVersion?: number } }
  | { command: "reject_run"; payload: { runId: string; commandId: string; expectedVersion?: number } }
  | { command: "regenerate_run"; payload: { runId: string; commandId: string; expectedVersion?: number } }
  | {
      command: "confirm_candidate_after_edit_run";
      payload: {
        runId: string;
        commandId: string;
        expectedVersion?: number;
        baseCandidateId: string;
        baseContentSha256: string;
        content: string;
      };
    }
  | { command: "resume_run"; payload: { runId: string; commandId: string; acceptCandidate: boolean } }
  | { command: "resume_run_with_quality_debt"; payload: { runId: string; commandId: string; reason: string } }
  | { command: "cancel_run"; payload: { runId: string; commandId: string } };

export interface RunCommandInput {
  runId: string;
  commandId: string;
  controlVersion?: number | null;
  candidate?: RunCandidateSummary | null;
  /** 质量债恢复要写进运行记录的原因，由调用方给出。 */
  qualityDebtReason: string;
}

/** 候选正文没齐备时抛这个：缺 CAS 参数就绝不发请求，避免覆盖别人改过的正文。 */
export const CANDIDATE_NOT_READY_MESSAGE = "候选正文还没准备好，请刷新后再确认。";

/**
 * 把动作翻译成真实请求。这是「发什么参数」的唯一来源：
 * 界面只负责把结果交给对应的 transport 函数，不再各自手写参数。
 */
export function buildRunCommandRequest(action: RunActionOption, input: RunCommandInput): RunCommandRequest {
  const { runId, commandId } = input;
  const expectedVersion = input.controlVersion ?? undefined;

  switch (action.command) {
    case "approve_run":
      return { command: "approve_run", payload: { runId, commandId, expectedVersion } };
    case "reject_run":
      return { command: "reject_run", payload: { runId, commandId, expectedVersion } };
    case "regenerate_run":
      return { command: "regenerate_run", payload: { runId, commandId, expectedVersion } };
    case "confirm_candidate_after_edit_run": {
      const candidate = input.candidate;
      if (!candidateReady(candidate)) throw new Error(CANDIDATE_NOT_READY_MESSAGE);
      return {
        command: "confirm_candidate_after_edit_run",
        payload: {
          runId,
          commandId,
          expectedVersion,
          baseCandidateId: candidate.id,
          baseContentSha256: candidate.contentSha256,
          content: candidate.content,
        },
      };
    }
    case "resume_run":
      return { command: "resume_run", payload: { runId, commandId, acceptCandidate: false } };
    case "resume_run_with_quality_debt":
      return {
        command: "resume_run_with_quality_debt",
        payload: { runId, commandId, reason: input.qualityDebtReason },
      };
    case "cancel_run":
      return { command: "cancel_run", payload: { runId, commandId } };
  }
}

const STATUS_TEXT: Record<string, string> = {
  queued: "排队中",
  running: "生成中",
  waiting_approval: "等待你的确认",
  paused: "已暂停",
  succeeded: "已完成",
  failed: "失败",
  cancelled: "已取消",
};

export function formatRunStatus(status: string | null | undefined): string {
  if (!status) return "暂无运行";
  return STATUS_TEXT[status] ?? status;
}

export function isRecoverableStatus(status: string | null | undefined): boolean {
  return status === "paused" || status === "waiting_approval" || status === "failed";
}

export interface CastSummary {
  total: number;
  protagonists: number;
  allies: number;
  antagonists: number;
}

/** 只做只读统计，用于右侧「人物与世界观」的概览。 */
export function summarizeCast(characters: Array<{ role?: string | null; storyFunction?: string | null }> = []): CastSummary {
  const summary: CastSummary = { total: characters.length, protagonists: 0, allies: 0, antagonists: 0 };
  for (const character of characters) {
    const text = `${character.role ?? ""} ${character.storyFunction ?? ""}`.toLowerCase();
    if (text.includes("主角") || text.includes("protagonist")) summary.protagonists += 1;
    else if (text.includes("对手") || text.includes("反派") || text.includes("antagonist")) summary.antagonists += 1;
    else if (text.includes("同伴") || text.includes("盟友") || text.includes("ally")) summary.allies += 1;
  }
  return summary;
}
