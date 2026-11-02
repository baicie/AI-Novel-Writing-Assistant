/**
 * 桌面版选区改写的纯逻辑：坐标校验、命令入参拼装、候选与 diff 生成。
 *
 * 这里不碰 Tauri 传输层，也不引用路径别名，便于用 node:test 做确定性验证。
 */

import type {
  ChapterEditorAiRevisionIntent,
  ChapterEditorAiRevisionRequest,
  ChapterEditorCandidate,
  ChapterEditorOperation,
  ChapterEditorTargetRange,
} from "@ai-novel/shared/types/novel";
import {
  buildChapterEditorDiff,
  normalizeChapterText,
  withDiffComplexityNote,
} from "@ai-novel/shared/types/chapterEditor/document";

/** 面向作者的改写意图标签：进提示词，模型据此决定改法。 */
export const REWRITE_OPERATION_LABELS: Record<ChapterEditorOperation, string> = {
  polish: "润色这段",
  expand: "扩写这段",
  compress: "压缩这段",
  emotion: "强化情绪",
  conflict: "强化冲突",
  custom: "按作者要求修改",
};

/** 本地内核回传的候选文本（结构与 `rewrite_chapter_selection` 的返回一致）。 */
export interface RewriteCandidateText {
  label?: string | null;
  content: string;
  summary?: string | null;
  rationale?: string | null;
  riskNotes?: string[];
  semanticTags?: string[];
}

export function rewriteOperationOf(request: ChapterEditorAiRevisionRequest): ChapterEditorOperation {
  return request.presetOperation ?? (request.source === "freeform" ? "custom" : "polish");
}

/**
 * 选区坐标解析：请求里的 selection 是编辑器坐标，必须与当前正文逐字对得上。
 *
 * 对不上就直接报错，不去猜作者想改哪一段 —— 猜错会把候选应用到错误位置。
 */
export function resolveRewriteTargetRange(
  request: ChapterEditorAiRevisionRequest,
): ChapterEditorTargetRange {
  const content = normalizeChapterText(request.contentSnapshot);
  if (request.scope === "chapter") {
    if (content.length === 0) {
      throw new Error("本章还没有正文，无法整章改写。");
    }
    return { from: 0, to: content.length, text: content };
  }
  const selection = request.selection;
  if (!selection) {
    throw new Error("请先选中要改写的正文。");
  }
  const { from, to } = selection;
  if (!Number.isInteger(from) || !Number.isInteger(to) || from < 0 || to <= from || to > content.length) {
    throw new Error("选区超出正文范围，请重新选中后再试。");
  }
  if (content.slice(from, to) !== normalizeChapterText(selection.text)) {
    throw new Error("正文片段已变化，请重新选中内容后再生成改写。");
  }
  return { from, to, text: content.slice(from, to) };
}

/** 本地内核命令的入参：坐标、原文与上下文都在这里定型。 */
export interface RewriteCommandInput {
  requestId: string;
  novelId: string;
  chapterId: string;
  selectionFrom: number;
  selectionTo: number;
  selectedText: string;
  contentSnapshot: string;
  operationLabel: string;
  scopeLabel: string;
  instruction?: string;
  beforeContext: string;
  afterContext: string;
  chapterTitle: string;
  model?: string;
}

export function buildRewriteCommandInput(input: {
  requestId: string;
  novelId: string;
  chapterId: string;
  chapterTitle?: string;
  request: ChapterEditorAiRevisionRequest;
  targetRange: ChapterEditorTargetRange;
}): RewriteCommandInput {
  const { request, targetRange } = input;
  return {
    requestId: input.requestId,
    novelId: input.novelId,
    chapterId: input.chapterId,
    selectionFrom: targetRange.from,
    selectionTo: targetRange.to,
    selectedText: targetRange.text,
    contentSnapshot: normalizeChapterText(request.contentSnapshot),
    operationLabel: REWRITE_OPERATION_LABELS[rewriteOperationOf(request)],
    scopeLabel: request.scope === "selection" ? "选中片段" : "整章",
    instruction: request.instruction?.trim() || undefined,
    beforeContext: (request.context?.beforeParagraphs ?? []).join("\n\n"),
    afterContext: (request.context?.afterParagraphs ?? []).join("\n\n"),
    chapterTitle: input.chapterTitle?.trim() ?? "",
    model: request.model,
  };
}

/**
 * 本地内核只回候选文本，id 与 diff 在这里统一生成。
 *
 * diff 必须用共享实现：`applySelectedChapterEditorDiff` 会按分片重建原文与候选，
 * 两套 diff 会让采纳时整段报「对比不完整」。
 */
export function toEditorCandidates(
  candidates: readonly RewriteCandidateText[],
  originalText: string,
  newId: () => string,
): ChapterEditorCandidate[] {
  const result: ChapterEditorCandidate[] = [];
  const seen = new Set<string>();
  for (const candidate of candidates) {
    // 候选会被整段写回正文，首尾空白必须在生成 diff 之前去掉：
    // 否则 diff 里会出现纯空白分片，作者采纳后正文多出看不见的空格。
    const content = normalizeChapterText(candidate.content).trim();
    if (content.length === 0 || seen.has(content)) continue;
    seen.add(content);
    const diff = buildChapterEditorDiff(originalText, content);
    result.push({
      id: newId(),
      label: candidate.label?.trim() || `方案 ${result.length + 1}`,
      content,
      summary: candidate.summary?.trim() || null,
      rationale: candidate.rationale?.trim() || null,
      riskNotes: withDiffComplexityNote(
        (candidate.riskNotes ?? []).map((note) => note.trim()).filter((note) => note.length > 0),
        diff.degraded,
      ),
      semanticTags: (candidate.semanticTags ?? []).map((tag) => tag.trim()).filter((tag) => tag.length > 0),
      diffChunks: diff.chunks,
    });
  }
  return result;
}

/**
 * 桌面端不跑服务端那套「意图解析」模型调用，避免为一次局部改写多付一次请求。
 * 这里按作者的操作与补充说明直出意图，保持响应结构完整。
 */
export function resolvedIntentFor(request: ChapterEditorAiRevisionRequest): ChapterEditorAiRevisionIntent {
  const operation = rewriteOperationOf(request);
  const instruction = request.instruction?.trim() ?? "";
  return {
    editGoal: instruction || REWRITE_OPERATION_LABELS[operation],
    toneShift: "",
    paceAdjustment: "",
    conflictAdjustment: "",
    emotionAdjustment: "",
    mustPreserve: ["剧情事实", "人称与叙事视角"],
    mustAvoid: ["新增未授权设定"],
    strength: operation === "polish" ? "light" : "medium",
    reasoningSummary: instruction
      ? `按作者要求改写：${instruction}`
      : `按「${REWRITE_OPERATION_LABELS[operation]}」改写${request.scope === "selection" ? "选中片段" : "整章"}。`,
  };
}
