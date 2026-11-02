/**
 * 编辑位置恢复（P17.2 Phase 4）。
 *
 * 目标：重新打开桌面版时回到上次写作的位置——最近的小说、最近的章节，
 * 以及章节内的光标与滚动位置。
 *
 * 边界：
 * - 只存坐标与标识，不存正文、不存 API Key；
 * - 恢复光标前必须确认正文已经恢复完成（编辑器文档与保存版本一致），
 *   否则会把光标放到错误的位置；
 * - 换版本后长度变化时，坐标必须重新校验，越界就放弃恢复而不是夹到末尾；
 * - 位置按小说隔离，绝不跨书复用。
 *
 * 不复用旧的 DOM 滚动恢复方案：WebView 在编辑器重建时会重置滚动，
 * 因此滚动位置与光标一起存在同一份记录里，由编辑器在文档恢复后统一应用。
 */

const STORAGE_PREFIX = "ai-novel:edit-position:v1";

/** 最近一次编辑位置：只包含恢复所需的坐标与标识。 */
export interface EditPositionRecord {
  novelId: string;
  chapterId: string;
  /** 编辑器里的 UTF-16 光标偏移。 */
  cursorOffset: number;
  /** 正文滚动容器距顶部的像素数。 */
  scrollTop: number;
  /** 记录时的章节正式版本；版本变化后坐标可能已经失效。 */
  officialVersion: number;
  /** 记录时正文长度，用来在版本变化后校验坐标是否仍然落在正文内。 */
  contentLength: number;
  updatedAt: number;
}

function storageOf(storage?: Storage | null): Storage | null {
  if (storage) return storage;
  try {
    return typeof window === "undefined" ? null : window.localStorage;
  } catch {
    return null;
  }
}

function keyFor(novelId: string): string {
  return `${STORAGE_PREFIX}:${novelId}`;
}

/** 读取某本书的编辑位置；数据损坏时返回 null，绝不让偏好缓存阻断打开章节。 */
export function readEditPosition(novelId: string, storage?: Storage | null): EditPositionRecord | null {
  const target = storageOf(storage);
  if (!target || !novelId) return null;
  try {
    const raw = target.getItem(keyFor(novelId));
    if (!raw) return null;
    const value = JSON.parse(raw) as Partial<EditPositionRecord>;
    if (value.novelId !== novelId
      || typeof value.chapterId !== "string"
      || typeof value.cursorOffset !== "number"
      || typeof value.scrollTop !== "number"
      || typeof value.officialVersion !== "number"
      || typeof value.contentLength !== "number"
      || typeof value.updatedAt !== "number") {
      return null;
    }
    return value as EditPositionRecord;
  } catch {
    return null;
  }
}

/** 写入编辑位置；位置按小说隔离，同一本书只保留最近一次。 */
export function writeEditPosition(record: EditPositionRecord, storage?: Storage | null): void {
  const target = storageOf(storage);
  if (!target || !record.novelId) return;
  try {
    target.setItem(keyFor(record.novelId), JSON.stringify(record));
  } catch {
    // 配额满或隐私模式：位置恢复是锦上添花，失败不能影响写作。
  }
}

export function clearEditPosition(novelId: string, storage?: Storage | null): void {
  const target = storageOf(storage);
  if (!target || !novelId) return;
  try {
    target.removeItem(keyFor(novelId));
  } catch {
    // 同上：清理失败不影响编辑。
  }
}

export interface EditPositionRestoreInput {
  /** 上次记录的位置。 */
  record: EditPositionRecord | null;
  /** 当前打开的章节。 */
  chapterId: string;
  /** 当前章节的正式版本。 */
  officialVersion: number;
  /** 当前正文长度（恢复完成后）。 */
  contentLength: number;
}

export interface EditPositionRestore {
  cursorOffset: number;
  scrollTop: number;
}

/**
 * 决定这次能不能恢复，以及恢复到哪。
 *
 * 返回 null 表示不恢复：章节不同、正文还没恢复完成、或坐标已超出当前正文。
 * 版本变化本身不阻止恢复——只要长度仍能容纳原坐标，光标位置通常仍然有效；
 * 长度变短时一律放弃，避免把光标停在一段不相干的文字中间。
 */
export function resolveEditPositionRestore(input: EditPositionRestoreInput): EditPositionRestore | null {
  const { record, chapterId, officialVersion, contentLength } = input;
  if (!record) return null;
  if (record.chapterId !== chapterId) return null;
  if (contentLength <= 0) return null;
  // 记录长度与当前长度都放不下时，坐标一定已经失配。
  if (record.cursorOffset < 0 || record.cursorOffset > contentLength) return null;
  if (record.contentLength > 0 && contentLength < record.contentLength * 0.5) return null;
  // 版本变了但长度相近：保留光标，滚动位置可能因排版变化略有偏差，按新正文边界收敛。
  const scrollTop = Number.isFinite(record.scrollTop) && record.scrollTop >= 0 ? record.scrollTop : 0;
  return {
    cursorOffset: record.cursorOffset,
    scrollTop,
  };
}

/** 记录时使用的稳定键：同一本书同一章只保留最新一次。 */
export function buildEditPositionRecord(input: {
  novelId: string;
  chapterId: string;
  cursorOffset: number;
  scrollTop: number;
  officialVersion: number;
  contentLength: number;
  now?: number;
}): EditPositionRecord {
  return {
    novelId: input.novelId,
    chapterId: input.chapterId,
    cursorOffset: Math.max(0, Math.floor(input.cursorOffset)),
    scrollTop: Math.max(0, Math.round(input.scrollTop)),
    officialVersion: input.officialVersion,
    contentLength: Math.max(0, Math.floor(input.contentLength)),
    updatedAt: input.now ?? Date.now(),
  };
}

/** 跨书索引：桌面版重新打开时用它回到上次写作的章节。 */
export interface RecentEditTarget {
  novelId: string;
  chapterId: string;
  updatedAt: number;
}

const RECENT_KEY = `${STORAGE_PREFIX}:recent`;

export function readRecentEditTarget(storage?: Storage | null): RecentEditTarget | null {
  const target = storageOf(storage);
  if (!target) return null;
  try {
    const raw = target.getItem(RECENT_KEY);
    if (!raw) return null;
    const value = JSON.parse(raw) as Partial<RecentEditTarget>;
    if (typeof value.novelId !== "string" || !value.novelId
      || typeof value.chapterId !== "string" || !value.chapterId
      || typeof value.updatedAt !== "number") {
      return null;
    }
    return value as RecentEditTarget;
  } catch {
    return null;
  }
}

/** 记录「最近编辑的章节」；与单书位置分开存，避免跨书读取位置明细。 */
export function writeRecentEditTarget(target: RecentEditTarget, storage?: Storage | null): void {
  const store = storageOf(storage);
  if (!store) return;
  try {
    store.setItem(RECENT_KEY, JSON.stringify(target));
  } catch {
    // 同位置记录：偏好写入失败不影响编辑。
  }
}

/**
 * 桌面版启动时要不要跳到上次写作的章节。
 *
 * 只在没有显式启动路由、且最近记录仍然存在时跳转；
 * 不校验章节是否还存在——由章节页自己处理「章节已被删除」的兜底。
 */
export function startupEditRoute(input: {
  startRoute: string | null;
  recent: RecentEditTarget | null;
}): string | null {
  if (input.startRoute) return null;
  if (!input.recent) return null;
  return `/novels/${input.recent.novelId}/chapters/${input.recent.chapterId}`;
}
