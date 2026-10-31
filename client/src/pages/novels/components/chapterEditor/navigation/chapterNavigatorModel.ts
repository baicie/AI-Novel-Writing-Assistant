import type { Chapter } from "@ai-novel/shared/types/novel";

export interface ChapterNavGroup {
  key: string;
  label: string;
  chapters: Chapter[];
}

export interface ChapterNavItem {
  id: string;
  order: number;
  title: string;
  volumeId: string | null;
  hasContent: boolean;
  officialVersion: number;
}

export interface ChapterNavVolume {
  id: string;
  title: string;
}

/** 章节的 Run 关注等级：u=需要用户处理，running=正在跑，done=已结束但不打扰。 */
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

function toItem(chapter: Chapter): ChapterNavItem {
  const raw = chapter as Chapter & { volumeId?: string | null };
  return {
    id: chapter.id,
    order: chapter.order,
    title: chapter.title?.trim() || "未命名章节",
    volumeId: raw.volumeId ?? null,
    hasContent: Boolean(chapter.content && chapter.content.trim().length > 0),
    officialVersion: chapter.officialVersion ?? 0,
  };
}

/** 章节按卷分组；没有卷信息的章节归入「未分卷」，保证总能导航到。 */
export function buildChapterGroups(chapters: Chapter[], volumes: ChapterNavVolume[] = []): ChapterNavGroup[] {
  const items = chapters
    .map(toItem)
    .sort((left, right) => left.order - right.order);
  const known = new Map(volumes.map((volume, index) => [volume.id, { title: volume.title, index }]));
  const groups = new Map<string, ChapterNavGroup>();

  for (const item of items) {
    const key = item.volumeId && known.has(item.volumeId) ? item.volumeId : "__none__";
    const volume = known.get(key);
    if (!groups.has(key)) {
      groups.set(key, {
        key,
        label: volume ? volume.title.trim() || `第 ${volume.index + 1} 卷` : "未分卷",
        chapters: [],
      });
    }
    groups.get(key)!.chapters.push({
      id: item.id,
      order: item.order,
      title: item.title,
      volumeId: item.volumeId,
      hasContent: item.hasContent,
      officialVersion: item.officialVersion,
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
    } as unknown as Chapter);
  }

  return [...groups.entries()]
    .sort((left, right) => {
      const leftIndex = known.get(left[0])?.index ?? Number.MAX_SAFE_INTEGER;
      const rightIndex = known.get(right[0])?.index ?? Number.MAX_SAFE_INTEGER;
      const leftMin = Math.min(...left[1].chapters.map((chapter) => (chapter as unknown as ChapterNavItem).order ?? 0));
      const rightMin = Math.min(...right[1].chapters.map((chapter) => (chapter as unknown as ChapterNavItem).order ?? 0));
      return leftIndex === rightIndex ? leftMin - rightMin : leftIndex - rightIndex;
    })
    .map(([, group]) => group);
}

/** 保存快捷键判定：⌘S / Ctrl+S，且不能带其他修饰键。 */
export function isSaveShortcut(event: { key: string; metaKey?: boolean; ctrlKey?: boolean; altKey?: boolean }): boolean {
  if (event.altKey) return false;
  if (!event.metaKey && !event.ctrlKey) return false;
  return event.key.toLowerCase() === "s";
}

/**
 * 切换章节前的保护判定：有未保存修改时必须得到用户明确同意。
 * 抽成纯函数，便于确定性验证，也保证按钮与快捷键共用同一套规则。
 */
export function shouldBlockChapterSwitch(isDirty: boolean, confirm: (message: string) => boolean): boolean {
  if (!isDirty) return false;
  return !confirm("本章还有未保存的修改。切换后未保存的内容仍可从本地草稿恢复，确定切换吗？");
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
export function neighbourOf(chapters: Chapter[], currentId: string): { previous: Chapter | null; next: Chapter | null } {
  const ordered = [...chapters].sort((left, right) => left.order - right.order);
  const index = ordered.findIndex((chapter) => chapter.id === currentId);
  if (index < 0) return { previous: null, next: null };
  return {
    previous: index > 0 ? ordered[index - 1] : null,
    next: index < ordered.length - 1 ? ordered[index + 1] : null,
  };
}
