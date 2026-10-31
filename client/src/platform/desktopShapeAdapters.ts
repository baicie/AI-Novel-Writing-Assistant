//! 桌面适配层使用的纯映射：把本地内核的返回形状整理成界面需要的 DTO。
//!
//! 这些函数只做字段搬运与兜底，不发起任何 IO；路由分发与调用留在 desktopApiAdapter。

import type { ApiResponse } from "@ai-novel/shared/types/api";
import type { Chapter, Novel } from "@ai-novel/shared/types/novel";
import type { NovelDetailResponse, NovelListItem } from "@/api/novel/shared";
import type { QuickSetupProviderOption, QuickSetupStatus } from "@ai-novel/shared/types/onboarding";
import { APP_RUNTIME } from "@/lib/constants";
import * as transport from "./desktopTransport";

import { asRecord, iso, taskStatusForRun } from "./desktopPrimitives";
import type { JsonRecord } from "./desktopPrimitives";

export { appError, asRecord, iso, ok, taskStatusForRun } from "./desktopPrimitives";




// 桌面内核只支持一套「地址 + 模型 + 密钥」配置，这里把内核状态映射成界面需要的形状。
// 内核返回的字段比 Web 侧精简（没有 providers / routeCoverage），缺字段会让界面直接崩，必须补齐。
export function toQuickSetupStatus(raw: unknown): QuickSetupStatus {
  const record = asRecord(raw);
  const ready = record.readyForCreation === true;
  const endpoint = typeof record.endpoint === "string" ? record.endpoint : "";
  const model = typeof record.model === "string" ? record.model : "";
  const provider: QuickSetupProviderOption = {
    id: "openai-compatible",
    kind: "custom",
    name: "自定义接口",
    requiresApiKey: true,
    configured: ready,
    active: ready,
    currentModel: model,
    defaultModel: "",
    currentBaseURL: endpoint,
    defaultBaseURL: "",
    models: [],
  };
  return {
    readyForCreation: ready,
    providers: [provider],
    selectedProvider: ready ? provider.id : null,
    selectedModel: ready && model ? model : null,
    routeCoverage: { configured: 0, total: 0, missingTaskTypes: [] },
    blockingReasons: ready ? [] : ["先在设置里填好模型地址和 API Key。"],
    recommendedAction: ready ? "start_creating" : "configure_provider",
  };
}
export function summaryToListItem(novel: transport.NovelSummary): NovelListItem {
  const updatedAt = iso(novel.updatedAt);
  return {
    id: novel.id,
    title: novel.title,
    description: novel.description,
    commercialTags: [],
    status: novel.status as NovelListItem["status"],
    writingMode: novel.writingMode as NovelListItem["writingMode"],
    creationExperience: novel.creationExperience as NovelListItem["creationExperience"],
    narrativeForm: novel.narrativeForm as NovelListItem["narrativeForm"],
    postGenerationStyleReviewEnabled: (novel as transport.NovelSummary & { postGenerationStyleReviewEnabled?: boolean }).postGenerationStyleReviewEnabled ?? false,
    projectMode: novel.projectMode as NovelListItem["projectMode"],
    createdAt: updatedAt,
    updatedAt,
    _count: { chapters: 0, characters: 0 },
    latestAutoDirectorTask: null,
    latestCreationStudioTask: null,
  };
}

export function detailToNovel(
  detail: transport.NovelDetail,
  chapters: transport.ChapterSnapshot[],
  volumes: transport.VolumeSnapshot[],
): NovelDetailResponse {
  const createdAt = iso(detail.createdAt);
  const updatedAt = iso(detail.updatedAt);
  const chapterItems: Chapter[] = chapters.map((chapter) => ({
    id: chapter.id,
    novelId: chapter.novelId,
    title: chapter.title,
    content: chapter.content,
    order: chapter.sortOrder,
    officialVersion: chapter.officialVersion,
    protection: chapter.protection,
    generationState: chapter.content.trim() ? "drafted" : "planned",
    chapterStatus: chapter.content.trim() ? "completed" : "unplanned",
    createdAt: iso(chapter.updatedAt),
    updatedAt: iso(chapter.updatedAt),
  }));
  return {
    id: detail.id,
    title: detail.title,
    description: detail.description,
    status: detail.status as NovelDetailResponse["status"],
    writingMode: detail.writingMode as NovelDetailResponse["writingMode"],
    creationExperience: detail.creationExperience as NovelDetailResponse["creationExperience"],
    narrativeForm: detail.narrativeForm as NovelDetailResponse["narrativeForm"],
    commercialTags: detail.commercialTags,
    projectMode: detail.projectMode as NovelDetailResponse["projectMode"],
    postGenerationStyleReviewEnabled: detail.postGenerationStyleReviewEnabled,
    targetAudience: detail.targetAudience,
    bookSellingPoint: detail.bookSellingPoint,
    competingFeel: detail.competingFeel,
    first30ChapterPromise: detail.first30ChapterPromise,
    genreId: detail.genreId,
    primaryStoryModeId: detail.primaryStoryModeId,
    secondaryStoryModeId: detail.secondaryStoryModeId,
    worldId: detail.worldId,
    narrativePov: detail.narrativePov as NovelDetailResponse["narrativePov"],
    pacePreference: detail.pacePreference as NovelDetailResponse["pacePreference"],
    styleTone: detail.styleTone,
    emotionIntensity: detail.emotionIntensity as NovelDetailResponse["emotionIntensity"],
    aiFreedom: detail.aiFreedom as NovelDetailResponse["aiFreedom"],
    defaultChapterLength: detail.defaultChapterLength,
    estimatedChapterCount: detail.estimatedChapterCount,
    projectStatus: detail.projectStatus as NovelDetailResponse["projectStatus"],
    storylineStatus: detail.storylineStatus as NovelDetailResponse["storylineStatus"],
    outlineStatus: detail.outlineStatus as NovelDetailResponse["outlineStatus"],
    resourceReadyScore: detail.resourceReadyScore,
    createdAt,
    updatedAt,
    chapters: chapterItems,
    characters: [],
    bible: null,
    plotBeats: [],
    genre: null,
    primaryStoryMode: null,
    secondaryStoryMode: null,
    world: null,
    volumes: volumes.map((volume) => ({
      id: volume.id,
      novelId: volume.novelId,
      sortOrder: volume.sortOrder,
      title: volume.title,
      summary: volume.summary,
      openPayoffs: [],
      status: "draft",
      chapters: [],
      createdAt,
      updatedAt,
    })),
  };
}


export function runToTaskSummary(run: transport.RunSummary) {
  return {
    id: run.id,
    kind: "novel_workflow",
    title: run.chapterTitle || "未命名章节",
    status: taskStatusForRun(run.state),
    pendingManualRecovery: false,
    progress: taskStatusForRun(run.state) === "succeeded" ? 100 : 0,
    currentStage: run.state,
    currentItemKey: run.chapterId || null,
    currentItemLabel: run.chapterTitle || null,
    executionScopeLabel: run.novelId,
    blockingReason: null,
    resumeAction: null,
    lastHealthyStage: null,
    attemptCount: 0,
    maxAttempts: 1,
    lastError: null,
    createdAt: iso(run.createdAt),
    updatedAt: iso(run.updatedAt),
    heartbeatAt: iso(run.updatedAt),
    ownerId: "desktop",
    ownerLabel: "本地创作内核",
    sourceRoute: `/novels/${run.novelId}/chapters/${run.chapterId}`,
    checkpointType: null,
    checkpointSummary: null,
    noticeCode: null,
    noticeSummary: null,
    failureCode: null,
    failureSummary: null,
    recoveryHint: null,
    tokenUsage: null,
  } as const;
}

