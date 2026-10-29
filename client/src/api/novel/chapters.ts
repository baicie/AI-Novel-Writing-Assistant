import type { ApiResponse } from "@ai-novel/shared/types/api";
import type {
  TimelineCheckReport,
  TimelineContextForChapter,
} from "@ai-novel/shared/types/timeline";
import type {
  ChapterEditorAiRevisionRequest,
  ChapterEditorAiRevisionResponse,
  ChapterEditorCursorContinuationRequest,
  ChapterEditorCursorContinuationResponse,
  Chapter,
  ChapterEditorWorkspaceResponse,
  ChapterEditorRewritePreviewRequest,
  ChapterEditorRewritePreviewResponse,
  ChapterStatus,
} from "@ai-novel/shared/types/novel";
import { apiClient } from "../client";

export async function getNovelChapters(id: string) {
  const { data } = await apiClient.get<ApiResponse<Chapter[]>>(`/novels/${id}/chapters`);
  return data;
}

export async function createNovelChapter(
  id: string,
  payload: {
    title: string;
    order: number;
    content?: string;
    expectation?: string;
    chapterStatus?: ChapterStatus;
    targetWordCount?: number;
    conflictLevel?: number;
    revealLevel?: number;
    mustAvoid?: string;
    taskSheet?: string;
    sceneCards?: string;
    repairHistory?: string;
    qualityScore?: number;
    continuityScore?: number;
    characterScore?: number;
    pacingScore?: number;
    riskFlags?: string;
  },
) {
  const { data } = await apiClient.post<ApiResponse<Chapter>>(`/novels/${id}/chapters`, payload);
  return data;
}

export async function updateNovelChapter(
  id: string,
  chapterId: string,
  payload: Partial<{
    expectedUpdatedAt: string;
    expectedVersion: number;
    title: string;
    order: number;
    content: string;
    expectation: string;
    chapterStatus: ChapterStatus;
    targetWordCount: number;
    conflictLevel: number;
    revealLevel: number;
    mustAvoid: string;
    taskSheet: string;
    sceneCards: string;
    repairHistory: string;
    qualityScore: number;
    continuityScore: number;
    characterScore: number;
    pacingScore: number;
    riskFlags: string;
  }>,
) {
  const { data } = await apiClient.put<ApiResponse<Chapter>>(`/novels/${id}/chapters/${chapterId}`, payload);
  return data;
}

export async function deleteNovelChapter(id: string, chapterId: string) {
  const { data } = await apiClient.delete<ApiResponse<null>>(`/novels/${id}/chapters/${chapterId}`);
  return data;
}

export async function getChapterTraces(novelId: string, chapterId: string) {
  const { data } = await apiClient.get<ApiResponse<import("@ai-novel/shared/types/agent").AgentRun[]>>(
    `/novels/${novelId}/chapters/${chapterId}/traces`,
  );
  return data;
}

export async function getChapterTimeline(novelId: string, chapterId: string) {
  const { data } = await apiClient.get<ApiResponse<{
    context: TimelineContextForChapter;
    latestReport: TimelineCheckReport | null;
  }>>(`/novels/${novelId}/chapters/${chapterId}/timeline`);
  return data;
}

export async function previewChapterRewrite(
  novelId: string,
  chapterId: string,
  payload: ChapterEditorRewritePreviewRequest,
) {
  const { data } = await apiClient.post<ApiResponse<ChapterEditorRewritePreviewResponse>>(
    `/novels/${novelId}/chapters/${chapterId}/editor/rewrite-preview`,
    payload,
  );
  return data;
}

export async function getChapterEditorWorkspace(novelId: string, chapterId: string) {
  const { data } = await apiClient.get<ApiResponse<ChapterEditorWorkspaceResponse>>(
    `/novels/${novelId}/chapters/${chapterId}/editor/workspace`,
  );
  return data;
}

export async function previewChapterAiRevision(
  novelId: string,
  chapterId: string,
  payload: ChapterEditorAiRevisionRequest,
) {
  const { data } = await apiClient.post<ApiResponse<ChapterEditorAiRevisionResponse>>(
    `/novels/${novelId}/chapters/${chapterId}/editor/ai-revision-preview`,
    payload,
  );
  return data;
}

export async function previewChapterCursorContinuation(
  novelId: string,
  chapterId: string,
  payload: ChapterEditorCursorContinuationRequest,
) {
  const { data } = await apiClient.post<ApiResponse<ChapterEditorCursorContinuationResponse>>(
    `/novels/${novelId}/chapters/${chapterId}/editor/continue-preview`,
    payload,
  );
  return data;
}

export async function resolveChapterAuditIssue(novelId: string, issueId: string) {
  const { data } = await apiClient.post<ApiResponse<unknown>>(
    `/novels/${novelId}/audit-issues/${issueId}/resolve`,
    {},
  );
  return data;
}

export async function generateChapterExecutionContract(
  novelId: string,
  chapterId: string,
  payload: Partial<{
    provider: import("@ai-novel/shared/types/llm").LLMProvider;
    model: string;
    temperature: number;
  }> = {},
) {
  const { data } = await apiClient.post<ApiResponse<Chapter>>(
    `/novels/${novelId}/chapters/${chapterId}/execution-contract`,
    payload,
  );
  return data;
}

/**
 * 【新增】章节 AI 修正 - 支持本地和云模式的路由包装函数
 */
export async function previewChapterAiRevisionWithRouting(
  novelId: string,
  chapterId: string,
  payload: ChapterEditorAiRevisionRequest,
) {
  const { shouldUseLocalGeneration, handleLocalAiRevision } = await import("@/api/localCreation");
  
  if (shouldUseLocalGeneration(novelId)) {
    return handleLocalAiRevision(novelId, chapterId, payload);
  }
  return previewChapterAiRevision(novelId, chapterId, payload);
}

/**
 * 【新增】光标续写 - 支持本地和云模式的路由包装函数
 */
export async function previewChapterCursorContinuationWithRouting(
  novelId: string,
  chapterId: string,
  payload: ChapterEditorCursorContinuationRequest,
) {
  const { shouldUseLocalGeneration, handleLocalCursorContinuation } = await import("@/api/localCreation");
  
  if (shouldUseLocalGeneration(novelId)) {
    return handleLocalCursorContinuation(novelId, chapterId, payload);
  }
  return previewChapterCursorContinuation(novelId, chapterId, payload);
}
