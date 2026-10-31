/** 右侧工作区的纯逻辑：只做投影与文案，不碰数据请求。 */

export interface CastItem {
  id: string;
  name: string;
  role?: string | null;
  storyFunction?: string | null;
  currentGoal?: string | null;
}

export type RunActionKey = "approve" | "resume" | "resume_debt" | "cancel" | "regenerate";

export interface RunActionOption {
  key: RunActionKey;
  label: string;
}

const ACTION_LABELS: Record<string, RunActionKey | null> = {
  approve: "approve",
  confirm: "approve",
  confirm_after_edit: "approve",
  resume: "resume",
  resume_with_quality_debt: "resume_debt",
  resume_quality_debt: "resume_debt",
  cancel: "cancel",
  regenerate: "regenerate",
};

const RUN_ACTION_TEXT: Record<RunActionKey, string> = {
  approve: "确认这一步",
  resume: "继续这次创作",
  resume_debt: "接受质量提醒并继续",
  cancel: "取消这次创作",
  regenerate: "重新生成",
};

/** 把后端 legalActions 映射成可直接展示的动作，未知动作一律忽略，绝不臆造入口。 */
export function availableRunActions(legalActions: string[] | null | undefined): RunActionOption[] {
  const seen = new Set<RunActionKey>();
  const options: RunActionOption[] = [];
  for (const raw of legalActions ?? []) {
    const key = ACTION_LABELS[raw.trim()];
    if (!key || seen.has(key)) continue;
    seen.add(key);
    options.push({ key, label: RUN_ACTION_TEXT[key] });
  }
  return options;
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
export function summarizeCast(
  characters: Array<{ role?: string | null; storyFunction?: string | null }> = [],
): CastSummary {
  const summary: CastSummary = { total: characters.length, protagonists: 0, allies: 0, antagonists: 0 };
  for (const character of characters) {
    const text = `${character.role ?? ""} ${character.storyFunction ?? ""}`.toLowerCase();
    if (text.includes("主角") || text.includes("protagonist")) summary.protagonists += 1;
    else if (text.includes("对手") || text.includes("反派") || text.includes("antagonist")) summary.antagonists += 1;
    else if (text.includes("同伴") || text.includes("盟友") || text.includes("ally")) summary.allies += 1;
  }
  return summary;
}
