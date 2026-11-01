import type { ApiResponse } from "@ai-novel/shared/types/api";
import type {
  ChapterEditorAiRevisionRequest,
  ChapterEditorAiRevisionResponse,
} from "@ai-novel/shared/types/novel";
import { rewriteChapterSelection } from "@/platform/desktopTransport";
import {
  buildRewriteCommandInput,
  resolveRewriteTargetRange,
  resolvedIntentFor,
  toEditorCandidates,
} from "./chapterSelectionRewriteModel";

/**
 * 桌面版选区改写：一次受控模型调用，不建 Run、不写候选表、不动正文。
 *
 * 候选只回到编辑器的 Diff 面板，作者决定采纳哪些修改后才落到正文。
 */
export async function rewriteChapterSelectionOnDesktop(
  novelId: string,
  chapterId: string,
  request: ChapterEditorAiRevisionRequest,
  chapterTitle?: string,
): Promise<ApiResponse<ChapterEditorAiRevisionResponse>> {
  const targetRange = resolveRewriteTargetRange(request);

  const view = await rewriteChapterSelection(
    buildRewriteCommandInput({
      requestId: `chapter-rewrite-${crypto.randomUUID()}`,
      novelId,
      chapterId,
      chapterTitle,
      request,
      targetRange,
    }),
  );

  const candidates = toEditorCandidates(
    view.candidates,
    targetRange.text,
    () => `local-candidate-${crypto.randomUUID()}`,
  );
  if (candidates.length === 0) {
    throw new Error("AI 没有给出可用的改写候选，请重试。");
  }

  return {
    success: true,
    data: {
      sessionId: `local-rewrite-${crypto.randomUUID()}`,
      scope: request.scope,
      resolvedIntent: resolvedIntentFor(request),
      targetRange,
      macroAlignmentNote: view.macroAlignmentNote ?? null,
      candidates,
      activeCandidateId: candidates[0]?.id ?? null,
    },
  };
}
