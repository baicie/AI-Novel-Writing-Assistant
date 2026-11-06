import type { Chapter } from "@ai-novel/shared/types/novel";

/**
 * 新建空白章节时的默认顺序与标题。
 *
 * 这里是唯一权威：所有“新建章节”入口都必须通过它取默认值，页面不要再各自拼接标题。
 *
 * 章节号取现有最大顺序 + 1，而不是列表长度 + 1。删除或重排章节后，列表长度会和
 * 最大顺序脱节；继续用长度会生成与现有章节重复的顺序。
 *
 * 默认标题只用于刚创建的空白章节，不会回写已有章节名称。当前客户端只有中文界面，
 * 因此默认标题固定为中文；以后引入语言设置时，只需在这里按语言返回对应写法。
 */
export const DEFAULT_CHAPTER_TITLE_PREFIX = "第";
export const DEFAULT_CHAPTER_TITLE_SUFFIX = "章";

export interface NewChapterDraft {
  order: number;
  title: string;
}

export function buildNewChapterDraft(
  chapters: ReadonlyArray<Pick<Chapter, "order">>,
  lastIssuedOrder = 0,
): NewChapterDraft {
  const maxOrder = chapters.reduce((max, chapter) => {
    const order = Number(chapter.order);
    return Number.isFinite(order) && order > max ? order : max;
  }, Math.max(0, Math.floor(lastIssuedOrder)));
  const order = Math.max(1, Math.floor(maxOrder) + 1);
  return { order, title: defaultChapterTitle(order) };
}

export function defaultChapterTitle(order: number): string {
  return `${DEFAULT_CHAPTER_TITLE_PREFIX}${order}${DEFAULT_CHAPTER_TITLE_SUFFIX}`;
}
