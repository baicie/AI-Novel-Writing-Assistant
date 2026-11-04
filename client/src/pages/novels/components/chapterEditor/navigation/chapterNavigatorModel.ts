import type { Chapter } from "@ai-novel/shared/types/novel";

/** 导航里每一章的最小契约：只带显示需要的字段，不再把半成品强转成 Chapter。 */
export interface ChapterNavItem {
  id: string;
  order: number;
  title: string;
  volumeId: string | null;
  officialVersion: number;
  wordCount: number;
  hasContent: boolean;
}

export interface ChapterNavGroup {
  key: string;
  label: string;
  chapters: ChapterNavItem[];
}

export interface ChapterNavVolume {
  id: string;
  title: string;
}

/** 章节的 Run 关注等级：action=需要用户处理，running=正在跑，done=已结束但不打扰。 */
export type ChapterRunAttention = "action" | "running" | "done" | null;

export function runAttentionFor(status: string | null | undefined): ChapterRunAttention {
  if (!status) return null;
  if (status === "waiting_approval" || status === "paused" || status === "failed") return "action";
  if (status === "running" || status === "queued") return "running";
  if (status === "succeeded" || status === "cancelled") return "done";
  return null;
}

/** 把「章节 → 最近一次 Run 状态」压成关注等级表，供导航渲染小标记。 */
export function buildRunAttention(
  runs: Array<{ chapterId: string; state: string; updatedAt: number }>,
): Record<string, ChapterRunAttention> {
  const latest = new Map<string, { state: string; updatedAt: number }>();
  for (const run of runs) {
    const current = latest.get(run.chapterId);
    if (!current || run.updatedAt > current.updatedAt) {
      latest.set(run.chapterId, { state: run.state, updatedAt: run.updatedAt });
    }
  }
  const attention: Record<string, ChapterRunAttention> = {};
  for (const [chapterId, run] of latest.entries()) {
    const level = runAttentionFor(run.state);
    if (level) attention[chapterId] = level;
  }
  return attention;
}

/** 章节字数：纯空白按「空」处理，避免把缩进当成正文。 */
export function formatChapterWordCount(content: string | null | undefined): string {
  const length = (content ?? "").trim().length;
  return length === 0 ? "空" : `${length} 字`;
}

/** 数字字数 → 展示文案，和 formatChapterWordCount 保持同一口径。 */
export function formatWordCount(words: number): string {
  return words > 0 ? `${words} 字` : "空";
}

export function toChapterNavItem(chapter: Chapter): ChapterNavItem {  const raw = chapter as Chapter & { volumeId?: string | null };
  const content = chapter.content ?? "";
  const wordCount = content.trim().length;
  return {
    id: chapter.id,
    order: chapter.order,
    title: chapter.title?.trim() || "未命名章节",
    volumeId: raw.volumeId ?? null,
    officialVersion: chapter.officialVersion ?? 0,
    wordCount,
    hasContent: wordCount > 0,
  };
}

/** 章节按卷分组；没有卷信息的章节归入「未分卷」，保证总能导航到。 */
export function buildChapterGroups(chapters: Chapter[], volumes: ChapterNavVolume[] = []): ChapterNavGroup[] {
  const items = chapters.map(toChapterNavItem).sort((left, right) => left.order - right.order);
  const known = new Map(volumes.map((volume, index) => [volume.id, { title: volume.title, index }]));
  const groups = new Map<string, ChapterNavGroup>();
  // 卷内最小章号在归组时顺带记下：排序比较器里只做常数次查表，
  // 避免按章节数反复展开数组（大部头小说下既慢也有栈溢出风险）。
  const minOrder = new Map<string, number>();
  const volumeIndex = new Map<string, number>();

  for (const item of items) {
    const key = item.volumeId && known.has(item.volumeId) ? item.volumeId : "__none__";
    const volume = known.get(key);
    if (!groups.has(key)) {
      groups.set(key, {
        key,
        label: volume ? volume.title.trim() || `第 ${volume.index + 1} 卷` : "未分卷",
        chapters: [],
      });
      minOrder.set(key, item.order);
      volumeIndex.set(key, volume?.index ?? Number.MAX_SAFE_INTEGER);
    } else if (item.order < minOrder.get(key)!) {
      minOrder.set(key, item.order);
    }
    groups.get(key)!.chapters.push(item);
  }

  return [...groups.values()].sort((left, right) => {
    const leftIndex = volumeIndex.get(left.key)!;
    const rightIndex = volumeIndex.get(right.key)!;
    return leftIndex === rightIndex
      ? minOrder.get(left.key)! - minOrder.get(right.key)!
      : leftIndex - rightIndex;
  });
}

/** 章节搜索：编号或标题命中都算，空关键词返回全部。 */
export function filterChapters(chapters: Chapter[], keyword: string): Chapter[] {
  const query = keyword.trim().toLowerCase();
  if (!query) return chapters;
  return chapters.filter((chapter) => {
    const title = chapter.title?.trim().toLowerCase() ?? "";
    return title.includes(query) || String(chapter.order).includes(query);
  });
}

/** 上一章 / 下一章：按 order 排序后的相邻项，边界返回 null。 */
export function neighbourOf(
  chapters: Chapter[],
  currentId: string,
): { previous: Chapter | null; next: Chapter | null } {
  const ordered = [...chapters].sort((left, right) => left.order - right.order);
  const index = ordered.findIndex((chapter) => chapter.id === currentId);
  if (index < 0) return { previous: null, next: null };
  return {
    previous: index > 0 ? ordered[index - 1] : null,
    next: index < ordered.length - 1 ? ordered[index + 1] : null,
  };
}

/**
 * 统一的导航意图：切章 + 是否顺带打开运行页签。
 * 目标章节就是当前章时不需要切章，只切页签，因此不需要未保存确认。
 */
export interface ChapterNavigationIntent {
  targetChapterId: string;
  openRunTab: boolean;
  requiresSwitchGuard: boolean;
}

export function chapterNavigationIntent(
  targetChapterId: string,
  currentChapterId: string,
  openRunTab = false,
): ChapterNavigationIntent {
  return {
    targetChapterId,
    openRunTab,
    requiresSwitchGuard: targetChapterId !== currentChapterId,
  };
}

/** 保存快捷键判定：⌘S / Ctrl+S，且不能带其他修饰键。 */
export function isSaveShortcut(event: { key: string; metaKey?: boolean; ctrlKey?: boolean; altKey?: boolean }): boolean {
  if (event.altKey) return false;
  if (!event.metaKey && !event.ctrlKey) return false;
  return event.key.toLowerCase() === "s";
}

/**
 * 离开当前编辑状态会让作者损失什么。
 * 候选稿永远无法找回；未保存正文能否恢复取决于去向，由各出口的文案分别说明。
 */
export type ChapterSwitchBlockReason = "candidate_pending" | "draft_write_busy" | "unsaved_content";

/** 切章守卫的状态输入：每一项都对应一种会丢失的作者成果。 */
export interface ChapterSwitchGuardState {
  /** 正文有未保存的修改。 */
  hasUnsavedContent: boolean;
  /** 正文正在保存，或本地草稿写入失败。 */
  isDraftWriteBusy: boolean;
  /** AI 候选稿已生成但还没有采用或放弃。 */
  hasPendingCandidate: boolean;
}

/** AI 修改会话里与「候选稿待确认」有关的最小形状。 */
export interface ChapterCandidateSessionLike {
  status: "idle" | "loading" | "ready" | "error";
  candidates?: Array<{ id: string }> | null;
}

/**
 * 候选稿是否还等着作者采用或放弃。
 * 只有 ready 且确实拿到候选稿才算：loading 还没有结果，error 与 idle 没有可丢的东西，
 * 采用或放弃后 Shell 会把会话重置回 idle，所以这里不需要额外的已应用标记。
 */
export function hasPendingCandidate(session: ChapterCandidateSessionLike | null | undefined): boolean {
  if (!session || session.status !== "ready") return false;
  return (session.candidates?.length ?? 0) > 0;
}

export interface ChapterSwitchConfirmation {
  /** 最需要让作者知道的那一项损失，用于埋点与测试断言。 */
  reason: ChapterSwitchBlockReason;
  /** 同时命中的全部损失，按严重程度排序。 */
  reasons: ChapterSwitchBlockReason[];
  message: string;
}

/**
 * 作者要去哪里。三种去向都会丢弃候选稿，但未保存正文的下场不同：
 * - switch_chapter / leave_editor：编辑器重建，正文仍留在本章的本地草稿里，可以恢复。
 * - load_version：历史版本会写进编辑区并覆盖同一个本地草稿槽位，未保存的正文无法找回。
 */
export type ChapterLeaveIntent = "switch_chapter" | "leave_editor" | "load_version";

const CANDIDATE_SENTENCE: Record<ChapterLeaveIntent, string> = {
  switch_chapter: "AI 候选稿还没有采用，切换章节会丢弃它，正文里不会留下这段改写。",
  leave_editor: "AI 候选稿还没有采用，离开正文编辑会丢弃它，正文里不会留下这段改写。",
  load_version: "AI 候选稿还没有采用，载入历史版本会丢弃它，正文里不会留下这段改写。",
};
const DRAFT_WRITE_SENTENCE: Record<ChapterLeaveIntent, string> = {
  switch_chapter: "本章正文的保存还没有完成，切换可能让这次修改写不进去。",
  leave_editor: "本章正文的保存还没有完成，离开可能让这次修改写不进去。",
  load_version: "本章正文的保存还没有完成，载入历史版本可能让这次修改写不进去。",
};
/** 未保存正文可以找回的去向：只有编辑器重建类的出口才敢这么承诺。 */
const UNSAVED_RECOVERABLE_SENTENCE: Record<"switch_chapter" | "leave_editor", string> = {
  switch_chapter: "本章正文有未保存的修改，切换后可以从本地草稿恢复。",
  leave_editor: "本章正文有未保存的修改，离开后可以从本地草稿恢复。",
};
/** 载入版本会覆盖本地草稿，未保存正文从此找不回来，必须如实说明。 */
const UNSAVED_OVERWRITTEN_SENTENCE = "本章正文有未保存的修改，载入历史版本会替换编辑区内容，这些修改无法找回。";
const UNSAVED_PLAIN_SENTENCE = "本章正文有未保存的修改。";
const QUESTION: Record<ChapterLeaveIntent, string> = {
  switch_chapter: "确定切换到其他章节吗？",
  leave_editor: "确定离开正文编辑吗？",
  load_version: "确定载入这个版本吗？",
};

/** 未保存正文该用哪句：写入未落地时不承诺恢复，载入版本如实说明会被覆盖。 */
function unsavedSentenceFor(intent: ChapterLeaveIntent, isDraftWriteBusy: boolean): string {
  if (intent === "load_version") return UNSAVED_OVERWRITTEN_SENTENCE;
  // 写入还没落地时不承诺本地草稿能恢复，避免给出做不到的保证。
  if (isDraftWriteBusy) return UNSAVED_PLAIN_SENTENCE;
  return UNSAVED_RECOVERABLE_SENTENCE[intent];
}

/**
 * 切章、离开正文编辑或载入历史版本前要不要问作者，以及问什么。
 * 返回 null 表示没有任何待处理成果，直接放行；否则给出按严重程度排序的原因与确认文案。
 * 候选稿排在最前：它无法从本地草稿找回，必须让作者先知道这一点。
 */
export function chapterSwitchConfirmation(
  state: ChapterSwitchGuardState,
  intent: ChapterLeaveIntent = "switch_chapter",
): ChapterSwitchConfirmation | null {
  const reasons: ChapterSwitchBlockReason[] = [];
  const sentences: string[] = [];

  if (state.hasPendingCandidate) {
    reasons.push("candidate_pending");
    sentences.push(CANDIDATE_SENTENCE[intent]);
  }
  if (state.isDraftWriteBusy) {
    reasons.push("draft_write_busy");
    sentences.push(DRAFT_WRITE_SENTENCE[intent]);
  }
  if (state.hasUnsavedContent) {
    reasons.push("unsaved_content");
    sentences.push(unsavedSentenceFor(intent, state.isDraftWriteBusy));
  }

  if (reasons.length === 0) return null;
  return {
    reason: reasons[0],
    reasons,
    message: [...sentences, QUESTION[intent]].join(""),
  };
}

/**
 * 离开当前正文前的保护判定：有未保存正文、未完成写入或未确认候选稿时都必须得到作者明确同意。
 * 抽成纯函数，便于确定性验证，也保证章节行、待处理徽标、上/下一章、快捷键、返回入口
 * 与版本载入共用同一套规则。
 */
export function shouldBlockChapterSwitch(
  state: ChapterSwitchGuardState,
  confirm: (message: string) => boolean,
  intent: ChapterLeaveIntent = "switch_chapter",
): boolean {
  const confirmation = chapterSwitchConfirmation(state, intent);
  if (!confirmation) return false;
  return !confirm(confirmation.message);
}

/**
 * 异步结果是否仍属于当前章节。
 *
 * 作者可以在 AI 请求进行中切章：晚到的候选或错误如果直接写进会话，
 * 会出现在另一章的编辑器里，甚至被采纳到错误位置。
 * `origin` 是发起请求时记下的章节，`current` 是响应到达时的章节。
 */
export function isResultForCurrentChapter(
  origin: string | null | undefined,
  current: string | null | undefined,
): boolean {
  // 两边都没有章节时没有可污染的目标；否则必须严格相等。
  if (!origin && !current) return true;
  return origin === current;
}
