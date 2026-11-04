import type { ChapterEditorDiffChunk } from "../novel";

/** Editor coordinates are UTF-16 offsets in this exact text, never a trimmed copy. */
export function normalizeChapterText(text: string | null | undefined): string {
  return (text ?? "").replace(/\r\n/g, "\n");
}

export function chapterParagraphs(text: string | null | undefined) {
  const normalized = normalizeChapterText(text);
  let cursor = 0;
  return normalized.split("\n\n").map((text, index) => {
    const from = cursor;
    cursor += text.length + 2;
    return { index: index + 1, text, from, to: from + text.length };
  });
}

export function replaceChapterSelection(
  content: string,
  selection: { from: number; to: number; text: string },
  replacement: string,
): string {
  const text = normalizeChapterText(content);
  if (!Number.isInteger(selection.from) || !Number.isInteger(selection.to)
    || selection.from < 0 || selection.to <= selection.from || selection.to > text.length
    || text.slice(selection.from, selection.to) !== selection.text) {
    throw new Error("正文片段已变化，请重新选中内容并生成修改建议。");
  }
  return text.slice(0, selection.from) + normalizeChapterText(replacement) + text.slice(selection.to);
}

export function insertChapterContinuation(content: string, offset: number, insertion: string): string {
  const text = normalizeChapterText(content);
  if (!Number.isInteger(offset) || offset < 0 || offset > text.length) {
    throw new Error("正文光标位置已变化，请重新定位光标并生成续写建议。");
  }
  return text.slice(0, offset) + normalizeChapterText(insertion) + text.slice(offset);
}

/**
 * Chunk ids and boundaries must match whatever produced the candidate, because
 * `applySelectedChapterEditorDiff` rejects chunks that do not rebuild both texts
 * exactly. Cloud and desktop therefore share this one implementation instead of
 * each carrying its own differ.
 */
export function buildChapterEditorDiffChunks(original: string, rewritten: string): ChapterEditorDiffChunk[] {
  return buildChapterEditorDiff(original, rewritten).chunks;
}

/** Diff 结果的完整形态：调用方需要知道这次对比是否退化成了整段替换。 */
export interface ChapterEditorDiffBuild {
  chunks: ChapterEditorDiffChunk[];
  /**
   * true 表示改动范围超出可细分的预算，某些区域只能按「整段替换」呈现。
   * 此时分片仍然能精确重建原文与候选，作者可以整段采纳或拒绝，但不能逐项勾选。
   */
  degraded: boolean;
}

/** 细粒度对比的工作量上限（单元格数），用于把长章节的耗时与内存限制在可预期范围。 */
const TOKEN_DIFF_CELL_BUDGET = 4_000_000;
/** 段落级对齐的单元格上限：段落数远小于字符数，几十万单元格足够覆盖超长章节。 */
const SEGMENT_DIFF_CELL_BUDGET = 250_000;
/** 单个段落对内部再细分时的单元格上限；超出则这一段整体替换。 */
const PAIR_DIFF_CELL_BUDGET = 250_000;
/** 一次对比中所有段落对内部分析的总预算，避免「段落很多且每段都不同」时累积过慢。 */
const TOTAL_PAIR_DIFF_CELL_BUDGET = 4_000_000;

/**
 * 有界 diff：先按码点裁掉公共前后缀，再在预算内做精确 LCS；
 * 超出预算时退到段落级对齐，仍超预算的区域明确标记为整段替换。
 *
 * 三条不变量：
 * - 任何路径产出的分片都能逐字重建原文与候选（采纳路径会再次校验）；
 * - 小文本与旧实现语义一致：equal/insert/delete 分片 + 相邻合并；
 * - 细粒度不可行时通过 `degraded` 明确告知调用方，而不是悄悄给出粗糙对比。
 */
export function buildChapterEditorDiff(original: string, rewritten: string): ChapterEditorDiffBuild {
  const source = normalizeChapterText(original);
  const target = normalizeChapterText(rewritten);
  const chunks: ChapterEditorDiffChunk[] = [];
  const state = { degraded: false };

  const { prefix, middleSource, middleTarget, suffix } = trimCommonEdges(source, target);
  pushDiffChunk(chunks, "equal", prefix);
  diffMiddle(middleSource, middleTarget, chunks, state);
  pushDiffChunk(chunks, "equal", suffix);

  return { chunks, degraded: state.degraded };
}

function diffMiddle(
  source: string,
  target: string,
  chunks: ChapterEditorDiffChunk[],
  state: { degraded: boolean },
): void {
  if (!source && !target) return;
  if (!source) {
    pushDiffChunk(chunks, "insert", target);
    return;
  }
  if (!target) {
    pushDiffChunk(chunks, "delete", source);
    return;
  }
  const sourceTokens = tokenizeForDiff(source);
  const targetTokens = tokenizeForDiff(target);
  if (sourceTokens.length * targetTokens.length <= TOKEN_DIFF_CELL_BUDGET) {
    diffTokens(sourceTokens, targetTokens, chunks);
    return;
  }
  diffSegments(source, target, chunks, state);
}

/** 段落级对齐：整章改写通常保留段落结构，先按段落定位，再在段落内部细看。 */
function diffSegments(
  source: string,
  target: string,
  chunks: ChapterEditorDiffChunk[],
  state: { degraded: boolean },
): void {
  const sourceSegments = splitDiffSegments(source);
  const targetSegments = splitDiffSegments(target);
  if (sourceSegments.length * targetSegments.length > SEGMENT_DIFF_CELL_BUDGET) {
    // 段落数量本身已超出预算：只能整段替换，并明确标记。
    pushDiffChunk(chunks, "delete", source);
    pushDiffChunk(chunks, "insert", target);
    state.degraded = true;
    return;
  }

  const { ops } = diffSequences(sourceSegments, targetSegments);
  let pairWork = 0;
  let deleteRun: string[] = [];
  let insertRun: string[] = [];
  const flushRuns = () => {
    const paired = Math.min(deleteRun.length, insertRun.length);
    for (let index = 0; index < paired; index += 1) {
      pairWork = diffSegmentPair(
        deleteRun[index]!,
        insertRun[index]!,
        chunks,
        state,
        pairWork,
      );
    }
    for (let index = paired; index < deleteRun.length; index += 1) {
      pushDiffChunk(chunks, "delete", deleteRun[index]!);
    }
    for (let index = paired; index < insertRun.length; index += 1) {
      pushDiffChunk(chunks, "insert", insertRun[index]!);
    }
    deleteRun = [];
    insertRun = [];
  };

  for (const op of ops) {
    if (op.type === "equal") {
      flushRuns();
      pushDiffChunk(chunks, "equal", op.text);
    } else if (op.type === "delete") {
      deleteRun.push(op.text);
    } else {
      insertRun.push(op.text);
    }
  }
  flushRuns();
}

/** 段落对内部再细分；预算不够时这一段整体替换，而不是给出错位的字符对比。 */
function diffSegmentPair(
  source: string,
  target: string,
  chunks: ChapterEditorDiffChunk[],
  state: { degraded: boolean },
  pairWork: number,
): number {
  const sourceTokens = tokenizeForDiff(source);
  const targetTokens = tokenizeForDiff(target);
  const work = sourceTokens.length * targetTokens.length;
  if (work > PAIR_DIFF_CELL_BUDGET || pairWork + work > TOTAL_PAIR_DIFF_CELL_BUDGET) {
    pushDiffChunk(chunks, "delete", source);
    pushDiffChunk(chunks, "insert", target);
    state.degraded = true;
    return pairWork;
  }
  diffTokens(sourceTokens, targetTokens, chunks);
  return pairWork + work;
}

type DiffOp =
  | { type: "equal"; text: string }
  | { type: "delete"; text: string }
  | { type: "insert"; text: string };

/**
 * 预算内的精确 LCS，用一维 Int32Array 存表：
 * 与旧的二维 number[][] 相比，同样的单元格数只占约四分之一内存，且仍能逐格回溯。
 */
function diffTokens(
  source: string[],
  target: string[],
  chunks: ChapterEditorDiffChunk[],
): void {
  const { ops } = diffSequences(source, target);
  for (const op of ops) {
    pushDiffChunk(chunks, op.type, op.text);
  }
}

function diffSequences(source: string[], target: string[]): { ops: DiffOp[] } {
  const sourceLength = source.length;
  const targetLength = target.length;
  const width = targetLength + 1;
  const table = new Int32Array((sourceLength + 1) * width);

  for (let sourceIndex = sourceLength - 1; sourceIndex >= 0; sourceIndex -= 1) {
    for (let targetIndex = targetLength - 1; targetIndex >= 0; targetIndex -= 1) {
      table[sourceIndex * width + targetIndex] = source[sourceIndex] === target[targetIndex]
        ? table[(sourceIndex + 1) * width + targetIndex + 1]! + 1
        : Math.max(
            table[(sourceIndex + 1) * width + targetIndex]!,
            table[sourceIndex * width + targetIndex + 1]!,
          );
    }
  }

  const ops: DiffOp[] = [];
  let sourceIndex = 0;
  let targetIndex = 0;
  while (sourceIndex < sourceLength && targetIndex < targetLength) {
    if (source[sourceIndex] === target[targetIndex]) {
      ops.push({ type: "equal", text: source[sourceIndex]! });
      sourceIndex += 1;
      targetIndex += 1;
      continue;
    }
    if (table[(sourceIndex + 1) * width + targetIndex]! >= table[sourceIndex * width + targetIndex + 1]!) {
      ops.push({ type: "delete", text: source[sourceIndex]! });
      sourceIndex += 1;
      continue;
    }
    ops.push({ type: "insert", text: target[targetIndex]! });
    targetIndex += 1;
  }
  while (sourceIndex < sourceLength) {
    ops.push({ type: "delete", text: source[sourceIndex]! });
    sourceIndex += 1;
  }
  while (targetIndex < targetLength) {
    ops.push({ type: "insert", text: target[targetIndex]! });
    targetIndex += 1;
  }
  return { ops };
}

/**
 * 按码点裁掉公共前后缀。
 *
 * 不能按 UTF-16 单元裁：Emoji 由代理对组成，按单元裁会把半个代理对分到两边，
 * 产出无法用字符串切片还原的边界。返回的前后缀与中间段拼接后必须等于原文本。
 */
function trimCommonEdges(
  source: string,
  target: string,
): { prefix: string; middleSource: string; middleTarget: string; suffix: string } {
  const sourcePoints = Array.from(source);
  const targetPoints = Array.from(target);
  const limit = Math.min(sourcePoints.length, targetPoints.length);

  let prefixLength = 0;
  while (prefixLength < limit && sourcePoints[prefixLength] === targetPoints[prefixLength]) {
    prefixLength += 1;
  }

  let suffixLength = 0;
  while (
    suffixLength < limit - prefixLength
    && sourcePoints[sourcePoints.length - 1 - suffixLength] === targetPoints[targetPoints.length - 1 - suffixLength]
  ) {
    suffixLength += 1;
  }

  return {
    prefix: sourcePoints.slice(0, prefixLength).join(""),
    middleSource: sourcePoints.slice(prefixLength, sourcePoints.length - suffixLength).join(""),
    middleTarget: targetPoints.slice(prefixLength, targetPoints.length - suffixLength).join(""),
    suffix: sourcePoints.slice(sourcePoints.length - suffixLength).join(""),
  };
}

/** 按空行切段并保留分隔符：`splitDiffSegments(text).join("") === text`。 */
function splitDiffSegments(text: string): string[] {
  const segments: string[] = [];
  let start = 0;
  for (let index = 0; index + 1 < text.length; index += 1) {
    if (text[index] === "\n" && text[index + 1] === "\n") {
      segments.push(text.slice(start, index + 2));
      start = index + 2;
      index += 1;
    }
  }
  if (start < text.length) segments.push(text.slice(start));
  return segments;
}

/** 长章节对比退化时必须出现在候选上的提示：作者据此知道只能整段确认。 */
export const DIFF_DEGRADED_NOTE = "这段正文较长，对比只能按整段替换呈现；请整段确认候选后再写回正文。";

/** 把退化提示并入候选的风险提示，避免同一句重复出现。 */
export function withDiffComplexityNote(riskNotes: readonly string[], degraded: boolean): string[] {
  if (!degraded || riskNotes.includes(DIFF_DEGRADED_NOTE)) return [...riskNotes];
  return [...riskNotes, DIFF_DEGRADED_NOTE];
}

/** Split on CJK characters, words, whitespace runs and punctuation so Chinese prose diffs per character. */
function tokenizeForDiff(text: string): string[] {
  // `u` \u6807\u5fd7\u8ba9 Emoji \u7b49\u4ee3\u7406\u5bf9\u6309\u4e00\u4e2a\u7801\u70b9\u6210\u8bcd\uff0c\u907f\u514d\u5206\u7247\u8fb9\u754c\u843d\u5728\u534a\u4e2a\u4ee3\u7406\u5bf9\u4e0a\u3002
  return normalizeChapterText(text).match(/[\u4e00-\u9fff]|[A-Za-z0-9_]+|\s+|[^\sA-Za-z0-9_\u4e00-\u9fff]/gu) ?? [];
}

function pushDiffChunk(chunks: ChapterEditorDiffChunk[], type: ChapterEditorDiffChunk["type"], text: string): void {
  if (!text) return;
  const last = chunks[chunks.length - 1];
  if (last && last.type === type) {
    last.text += text;
    return;
  }
  chunks.push({ id: `chunk-${chunks.length + 1}`, type, text });
}

export interface ChapterEditorDiffChange {
  id: string;
  kind: "insert" | "delete" | "replace";
  from: number;
  to: number;
  originalText: string;
  candidateText: string;
  chunkIds: string[];
}

/**
 * Adjacent insert/delete chunks are one replacement, so accepting a rewrite
 * never leaves duplicated source text behind.
 */
export function groupChapterEditorDiffChanges(chunks: readonly ChapterEditorDiffChunk[]): ChapterEditorDiffChange[] {
  const changes: ChapterEditorDiffChange[] = [];
  let cursor = 0;
  let pending: ChapterEditorDiffChunk[] = [];
  let pendingFrom = 0;
  const flush = () => {
    if (!pending.length) return;
    const originalText = pending.filter((chunk) => chunk.type === "delete").map((chunk) => chunk.text).join("");
    const candidateText = pending.filter((chunk) => chunk.type === "insert").map((chunk) => chunk.text).join("");
    changes.push({
      id: `change-${changes.length + 1}`,
      kind: originalText && candidateText ? "replace" : originalText ? "delete" : "insert",
      from: pendingFrom,
      to: cursor,
      originalText,
      candidateText,
      chunkIds: pending.map((chunk) => chunk.id),
    });
    pending = [];
  };
  for (const chunk of chunks) {
    if (chunk.type === "equal") {
      flush();
      cursor += chunk.text.length;
      continue;
    }
    if (!pending.length) pendingFrom = cursor;
    pending.push(chunk);
    if (chunk.type === "delete") cursor += chunk.text.length;
  }
  flush();
  return changes;
}

export function applySelectedChapterEditorDiff(
  originalText: string,
  candidateText: string,
  chunks: readonly ChapterEditorDiffChunk[],
  selectedChangeIds: ReadonlySet<string>,
): string {
  if (chunks.some((chunk) => !["equal", "insert", "delete"].includes(chunk.type))
    || chunks.filter((chunk) => chunk.type !== "insert").map((chunk) => chunk.text).join("") !== originalText
    || chunks.filter((chunk) => chunk.type !== "delete").map((chunk) => chunk.text).join("") !== candidateText) {
    throw new Error("候选与原文的对比不完整，请重新生成后再采纳。");
  }
  const changes = groupChapterEditorDiffChanges(chunks);
  if ([...selectedChangeIds].some((id) => !changes.some((change) => change.id === id))) {
    throw new Error("候选选择已变化，请重新选择要采纳的修改。");
  }
  const changesByChunkId = new Map(changes.flatMap((change) => change.chunkIds.map((chunkId) => [chunkId, change] as const)));
  const renderedChanges = new Set<string>();
  return chunks.map((chunk) => {
    if (chunk.type === "equal") return chunk.text;
    const change = changesByChunkId.get(chunk.id);
    if (!change || renderedChanges.has(change.id)) return "";
    renderedChanges.add(change.id);
    return selectedChangeIds.has(change.id) ? change.candidateText : change.originalText;
  }).join("");
}
