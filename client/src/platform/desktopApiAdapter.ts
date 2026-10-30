import type {
  AxiosAdapter,
  AxiosError,
  AxiosRequestConfig,
  AxiosResponse,
  InternalAxiosRequestConfig,
} from "axios";
import { AxiosError as AxiosErrorClass, AxiosHeaders } from "axios";
import type { ApiResponse } from "@ai-novel/shared/types/api";
import type { Chapter, Novel } from "@ai-novel/shared/types/novel";
import type { NovelDetailResponse, NovelListItem, NovelListResponse } from "@/api/novel/shared";
import type { QuickSetupProviderOption, QuickSetupStatus } from "@ai-novel/shared/types/onboarding";
import { APP_RUNTIME } from "@/lib/constants";
import * as transport from "./desktopTransport";
import { providerClient } from "./providerTransport";

type JsonRecord = Record<string, unknown>;

function iso(timestamp: number): string {
  return new Date(timestamp).toISOString();
}

function asRecord(value: unknown): JsonRecord {
  if (typeof value === "string") {
    try {
      const parsed: unknown = JSON.parse(value);
      return parsed && typeof parsed === "object" ? parsed as JsonRecord : {};
    } catch {
      return {};
    }
  }
  return value && typeof value === "object" ? value as JsonRecord : {};
}

function appError(error: unknown): { code: string; message: string; retryable: boolean } {
  const record = asRecord(error);
  return {
    code: typeof record.code === "string" ? record.code : "INTEGRITY_ERROR",
    message: typeof record.message === "string" ? record.message : "本地创作内核没有完成这次操作。",
    retryable: record.retryable === true,
  };
}


// 未映射的桌面路由会被记进应用日志，方便从用户日志里直接看到缺哪个接口。
function reportMissingRoute(method: string, path: string): void {
  if (typeof window !== "undefined") {
    const store = (window as unknown as { __missingRoutes?: string[] });
    store.__missingRoutes = [...(store.__missingRoutes ?? []), `${method.toUpperCase()} ${path}`];
  }
  void import("@tauri-apps/api/core")
    .then(({ invoke }) =>
      invoke("report_client_runtime", {
        config: null,
        locationHash: typeof window === "undefined" ? "" : window.location.hash,
        runtimeMode: "desktop",
        message: `NOT_MIGRATED ${method.toUpperCase()} ${path}`,
      }),
    )
    .catch(() => {});
}

function unsupported(config: InternalAxiosRequestConfig): never {
  const message = "这个功能还没有接入桌面本地内核，请先从已迁移的书稿入口继续。";
  reportMissingRoute(methodOf(config), pathOf(config));
  throw new AxiosErrorClass(
    message,
    "ERR_DESKTOP_NOT_MIGRATED",
    config,
    null,
    {
      status: 501,
      statusText: "Not Implemented",
      headers: {},
      config,
      data: {
        success: false,
        error: "这个功能暂未接入桌面本地内核",
        code: "NOT_MIGRATED",
        message,
      },
    },
  );
}


// 桌面端的「厂商列表」来自本地模型来源注册表，不再调用网页版厂商接口。
async function desktopProviders(): Promise<unknown[]> {
  const [profiles, bindings] = await Promise.all([
    providerClient.listProfiles(),
    providerClient.listBindings(),
  ]);
  const modelsByProvider = await Promise.all(
    profiles.map((profile) => providerClient.listModels(profile.id)),
  );
  return profiles.map((profile, index) => {
    const models = modelsByProvider[index]?.map((model) => model.modelId) ?? [];
    const bound = bindings.find((binding) => binding.providerId === profile.id);
    return {
      provider: profile.id,
      kind: "custom",
      name: profile.displayName ?? profile.id,
      displayName: profile.displayName ?? profile.id,
      currentModel: bound?.modelId ?? models[0] ?? "",
      currentImageModel: null,
      currentBaseURL: profile.baseUrl,
      currentAuthMode: "api_key",
      models,
      imageModels: [],
      defaultModel: models[0] ?? "",
      defaultImageModel: null,
      defaultBaseURL: profile.baseUrl,
      requiresApiKey: true,
      isConfigured: profile.credentialPresent,
      isActive: profile.credentialPresent,
    };
  });
}

async function desktopLlmSelection(): Promise<unknown> {
  const route = await providerClient.getEffectiveRoute("writer");
  if (!route) return null;
  return { provider: route.providerId, model: route.modelId, temperature: 0.7 };
}

// 桌面版没有网页版的成长引导与自动导演跟进队列，返回中性空数据，不报「未接入」。
function desktopOnboarding(): unknown {
  return {
    graduated: true,
    currentMilestone: "first_novel_created",
    completedCount: 0,
    totalCount: 0,
    headline: "",
    description: "",
    reason: "desktop",
    primaryAction: { label: "", route: "/", kind: "navigate" },
    novel: null,
    directorTask: null,
    firstReadableChapter: null,
    milestones: [],
    optionalEnhancements: [],
  };
}

function emptyFollowUps(): unknown {
  return { totalCount: 0, countersByReason: {}, countersBySection: {} };
}


// 桌面端的任务模型路由来自本地绑定表；没绑定的任务不返回路由。
const DESKTOP_TASK_TYPES = [
  "planner", "writer", "review", "light_review", "critical_review", "repair",
  "replan", "state_resolution", "summary", "fact_extraction", "chat",
];

async function desktopModelRoutes(): Promise<unknown> {
  const bindings = await providerClient.listBindings();
  return {
    taskTypes: DESKTOP_TASK_TYPES,
    routes: bindings.map((binding) => ({
      taskType: binding.taskType,
      provider: binding.providerId,
      model: binding.modelId,
      temperature: 0.7,
      maxTokens: null,
      requestProtocol: "auto",
      structuredResponseFormat: "auto",
    })),
  };
}


// 桌面端的连通性检查只做本地事实核对，不发真实生成请求，避免产生费用。
async function desktopConnectivity(): Promise<unknown[]> {
  const [profiles, bindings] = await Promise.all([
    providerClient.listProfiles(),
    providerClient.listBindings(),
  ]);
  return bindings.map((binding) => {
    const profile = profiles.find((item) => item.id === binding.providerId);
    const ok = Boolean(profile?.credentialPresent);
    return {
      taskType: binding.taskType,
      provider: binding.providerId,
      model: binding.modelId,
      ok,
      latency: null,
      error: ok ? null : "这个来源还没有可用的密钥",
      requestProtocol: null,
      plain: { ok, latency: null, error: ok ? null : "缺少可用密钥", requestProtocol: null },
      structured: null,
    };
  });
}


// 网页版的「预览自定义厂商模型」在桌面端就是本地发现：同样只取候选，不落库。
async function desktopPreviewModels(payload: JsonRecord): Promise<unknown> {
  const baseUrl = String(payload.baseURL ?? "").trim();
  if (!baseUrl) return { models: [], defaultModel: "" };
  const preview = await providerClient.discover({
    protocol: "openai-compatible",
    baseUrl,
    apiKey: typeof payload.key === "string" && payload.key.trim() ? payload.key.trim() : null,
    allowLocalhostHttp: /^http:\/\//i.test(baseUrl),
  });
  const models = preview.models.map((item) => item.modelId);
  return { models, defaultModel: models[0] ?? "" };
}

// 网页版的「刷新某厂商模型列表」在桌面端用已保存的地址与密钥重新发现。
async function desktopRefreshModels(providerId: string): Promise<unknown> {
  const profiles = await providerClient.listProfiles();
  const profile = profiles.find((item) => item.id === providerId);
  if (!profile) return { provider: providerId, models: [], currentModel: "" };
  const preview = await providerClient.discover({
    providerId: profile.id,
    protocol: "openai-compatible",
    baseUrl: profile.baseUrl,
    apiKey: null,
    allowLocalhostHttp: profile.allowLocalhostHttp,
  });
  const models = preview.models.map((item) => item.modelId);
  return { provider: providerId, models, currentModel: models[0] ?? "" };
}

function ok<T>(data: T): ApiResponse<T> {
  return { success: true, data };
}

// 桌面内核只支持一套「地址 + 模型 + 密钥」配置，这里把内核状态映射成界面需要的形状。
// 内核返回的字段比 Web 侧精简（没有 providers / routeCoverage），缺字段会让界面直接崩，必须补齐。
function toQuickSetupStatus(raw: unknown): QuickSetupStatus {
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
function pathOf(config: AxiosRequestConfig): string {
  const raw = config.url ?? "";
  try {
    return new URL(raw, "http://desktop.local").pathname;
  } catch {
    return raw;
  }
}

function methodOf(config: AxiosRequestConfig): string {
  return (config.method ?? "get").toLowerCase();
}

function queryOf(config: AxiosRequestConfig): URLSearchParams {
  const search = new URLSearchParams();
  Object.entries(asRecord(config.params)).forEach(([key, value]) => {
    if (value !== undefined && value !== null && value !== "") {
      search.set(key, String(value));
    }
  });
  return search;
}

function summaryToListItem(novel: transport.NovelSummary): NovelListItem {
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

function detailToNovel(
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

async function loadNovel(id: string): Promise<NovelDetailResponse> {
  const [detail, chapters, volumes, characters] = await Promise.all([
    transport.getNovel(id),
    transport.listChapters(id),
    transport.listVolumes(id),
    transport.listCharacters(id),
  ]);
  const response = detailToNovel(detail, chapters, volumes);
  response.characters = characters.map((character) => ({
    id: character.id,
    name: character.name,
    role: character.role,
    personality: character.personality ?? null,
    development: character.arc ?? null,
    novelId: character.novelId,
    createdAt: iso(character.createdAt),
    updatedAt: iso(character.updatedAt),
  }));
  const list = summaryToListItem({
    ...detail,
    creationExperience: detail.creationExperience,
    narrativeForm: detail.narrativeForm,
    writingMode: detail.writingMode,
    projectMode: detail.projectMode,
  });
  response.workspaceSourceRoute = `/novels/${list.id}`;
  return response;
}

async function listNovelPage(config: AxiosRequestConfig): Promise<ApiResponse<NovelListResponse>> {
  const query = queryOf(config);
  const all = await transport.listNovels();
  const search = (query.get("search") ?? "").trim().toLocaleLowerCase();
  const status = query.get("status");
  const narrativeForm = query.get("narrativeForm");
  const writingMode = query.get("writingMode");
  const filtered = all.filter((novel) => {
    if (status && novel.status !== status) return false;
    if (narrativeForm && novel.narrativeForm !== narrativeForm) return false;
    if (writingMode && novel.writingMode !== writingMode) return false;
    if (search && !`${novel.title}\n${novel.description}`.toLocaleLowerCase().includes(search)) return false;
    return true;
  });
  const page = Math.max(1, Number(query.get("page") ?? 1) || 1);
  const limit = Math.max(1, Number(query.get("limit") ?? 20) || 20);
  const start = (page - 1) * limit;
  const items = filtered.slice(start, start + limit);
  const chapterCounts = await Promise.all(items.map((novel) => transport.listChapters(novel.id).then((chapters) => chapters.length).catch(() => 0)));
  const mapped = items.map((novel, index) => {
    const item = summaryToListItem(novel);
    item._count.chapters = chapterCounts[index] ?? 0;
    return item;
  });
  return ok({
    items: mapped,
    page,
    limit,
    total: filtered.length,
    totalPages: Math.max(1, Math.ceil(filtered.length / limit)),
  });
}

function taskStatusForRun(state: string): "queued" | "running" | "waiting_approval" | "succeeded" | "failed" | "cancelled" {
  switch (state) {
    case "queued":
      return "queued";
    case "running":
      return "running";
    case "waiting_approval":
    case "paused_for_approval":
      return "waiting_approval";
    case "completed":
    case "succeeded":
      return "succeeded";
    case "cancelled":
      return "cancelled";
    default:
      return "failed";
  }
}

function runToTaskSummary(run: transport.RunSummary) {
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

async function listTaskRuns(config: InternalAxiosRequestConfig): Promise<ApiResponse<unknown>> {
  const query = queryOf(config);
  const kind = query.get("kind");
  const status = query.get("status");
  const keyword = (query.get("keyword") ?? "").trim().toLocaleLowerCase();
  if (kind && kind !== "novel_workflow") return unsupported(config);
  const runs = await transport.listAllRuns();
  const items = runs
    .filter((run) => !status || taskStatusForRun(run.state) === status)
    .filter((run) => !keyword || `${run.chapterTitle}\n${run.novelId}`.toLocaleLowerCase().includes(keyword))
    .map(runToTaskSummary);
  return ok({ items, nextCursor: null });
}

async function getTaskDetail(config: InternalAxiosRequestConfig, runId: string): Promise<ApiResponse<unknown>> {
  const runs = await transport.listAllRuns();
  const run = runs.find((candidate) => candidate.id === runId);
  if (!run) return ok(null);
  return ok({
    ...runToTaskSummary(run),
    provider: run.provider,
    model: run.model,
    startedAt: null,
    finishedAt: null,
    retryCountLabel: "0 次",
    meta: {},
    steps: [],
    failureDetails: null,
  });
}

async function updateChapter(config: AxiosRequestConfig, novelId: string, chapterId: string): Promise<ApiResponse<Chapter>> {
  const payload = asRecord(config.data);
  const current = await transport.getChapter({ novelId, chapterId });
  const expectedVersion = typeof payload.expectedVersion === "number" ? payload.expectedVersion : current.officialVersion;
  const hasContent = typeof payload.content === "string";
  let saved = current;
  if (hasContent) {
    saved = await transport.saveChapter({
      novelId,
      chapterId,
      expectedVersion,
      title: typeof payload.title === "string" ? payload.title : current.title,
      content: payload.content as string,
    });
  }
  const hasMetaChange = typeof payload.title === "string"
    || typeof payload.order === "number"
    || typeof payload.volumeId === "string";
  if (hasMetaChange) {
    saved = await transport.updateChapterMeta({
      novelId,
      chapterId,
      expectedVersion: saved.officialVersion,
      title: typeof payload.title === "string" ? payload.title : saved.title,
      sortOrder: typeof payload.order === "number" ? payload.order : saved.sortOrder,
      volumeId: typeof payload.volumeId === "string" ? payload.volumeId : undefined,
    });
  }
  return ok({
    id: saved.id,
    novelId: saved.novelId,
    title: saved.title,
    content: saved.content,
    order: saved.sortOrder,
    officialVersion: saved.officialVersion,
    protection: saved.protection,
    chapterStatus: saved.content.trim() ? "completed" : "unplanned",
    createdAt: iso(saved.updatedAt),
    updatedAt: iso(saved.updatedAt),
  });
}

async function exportNovelFile(config: InternalAxiosRequestConfig, novelId: string): Promise<AxiosResponse> {
  const query = queryOf(config);
  const format = (query.get("format") ?? "txt").toLowerCase();
  const scope = query.get("scope") ?? "full";
  if (scope !== "full" || !["txt", "markdown", "json"].includes(format)) {
    unsupported(config);
  }
  const text = await transport.exportNovel({
    novelId,
    format: format as "txt" | "markdown" | "json",
  });
  const mime = format === "json" ? "application/json" : format === "markdown" ? "text/markdown" : "text/plain";
  const extension = format === "json" ? "json" : format === "markdown" ? "md" : "txt";
  return {
    data: new Blob([text], { type: `${mime};charset=utf-8` }),
    status: 200,
    statusText: "OK",
    headers: new AxiosHeaders({
      "content-type": `${mime};charset=utf-8`,
      "content-disposition": `attachment; filename="novel.${extension}"`,
    }),
    config,
    request: null,
  };
}

async function route(config: InternalAxiosRequestConfig): Promise<unknown> {
  const method = methodOf(config);
  const path = pathOf(config);
  const taskDetail = path.match(/^\/tasks\/novel_workflow\/([^/]+)$/);
  if (taskDetail && method === "get") {
    return getTaskDetail(config, decodeURIComponent(taskDetail[1]!));
  }
  if (path === "/tasks" && method === "get") return listTaskRuns(config);
  if (path === "/settings/api-keys" && method === "get") return desktopProviders();
  if (path === "/settings/llm-selection" && method === "get") return desktopLlmSelection();
  if (path === "/settings/llm-selection" && method === "put") return asRecord(config.data);
  if (path === "/onboarding/first-novel" && method === "get") return desktopOnboarding();
  if (path === "/auto-director/follow-ups/overview" && method === "get") return emptyFollowUps();
  if (path === "/settings/custom-providers/models" && method === "post") {
    return desktopPreviewModels(asRecord(config.data));
  }
  {
    const refresh = path.match(/^\/settings\/api-keys\/([^/]+)\/refresh-models$/);
    if (refresh && method === "post") return desktopRefreshModels(decodeURIComponent(refresh[1]!));
  }
  if (path === "/llm/model-routes" && method === "get") return desktopModelRoutes();
  if (path === "/llm/model-routes/connectivity" && method === "post") return desktopConnectivity();
  if (path === "/settings/auto-director/issue-policy" && method === "get") {
    return { policy: "completion_first", updatedAt: null };
  }
  if (path === "/settings/auto-director/approval-preferences" && method === "get") {
    return { mode: "auto", updatedAt: null };
  }
  if (path === "/style-profiles" && method === "get") return [];
  if (path === "/tasks/recovery-candidates" && method === "get") return [];
  if (path === "/rag/health" && method === "get") return { ready: false, documents: 0, chunks: 0, embeddingProvider: "none" };
  if (path === "/rag/jobs" && method === "get") return [];
  if (path === "/rag/jobs/finished" && method === "get") return [];
  if (path === "/settings/rag" && method === "get") return { embeddingProvider: "none", chunkSize: 0, chunkOverlap: 0, topK: 0 };
  if (path === "/settings/style-engine-runtime" && method === "get") return { styleExtractionTimeoutMs: 0 };
  if (path === "/novels" && method === "get") return listNovelPage(config);
  if (path === "/novels" && method === "post") {
    const payload = asRecord(config.data);
    if (
      payload.writingMode === "continuation"
      || Boolean(payload.sourceNovelId)
      || Boolean(payload.sourceKnowledgeDocumentId)
      || Boolean(payload.continuationBookAnalysisId)
    ) {
      throw new AxiosErrorClass(
        "续写来源导入还没有接入桌面本地内核，现有来源资料不会被改动。",
        "ERR_DESKTOP_NOT_MIGRATED",
        config,
        null,
        {
          status: 501,
          statusText: "Not Implemented",
          headers: {},
          config,
          data: {
            success: false,
            error: "续写来源导入暂未接入桌面本地内核",
            code: "NOT_MIGRATED",
            message: "续写来源导入还没有接入桌面本地内核，现有来源资料不会被改动。",
          },
        },
      );
    }
    const created = await transport.createNovelWithSetup({
      title: String(payload.title ?? "").trim(),
      description: typeof payload.description === "string" ? payload.description : "",
      targetAudience: typeof payload.targetAudience === "string" ? payload.targetAudience : undefined,
      bookSellingPoint: typeof payload.bookSellingPoint === "string" ? payload.bookSellingPoint : undefined,
      competingFeel: typeof payload.competingFeel === "string" ? payload.competingFeel : undefined,
      first30ChapterPromise: typeof payload.first30ChapterPromise === "string" ? payload.first30ChapterPromise : undefined,
      commercialTags: Array.isArray(payload.commercialTags) ? payload.commercialTags.filter((item): item is string => typeof item === "string") : undefined,
      genreId: typeof payload.genreId === "string" ? payload.genreId : undefined,
      primaryStoryModeId: typeof payload.primaryStoryModeId === "string" ? payload.primaryStoryModeId : undefined,
      secondaryStoryModeId: typeof payload.secondaryStoryModeId === "string" ? payload.secondaryStoryModeId : undefined,
      worldId: typeof payload.worldId === "string" ? payload.worldId : undefined,
      writingMode: typeof payload.writingMode === "string" ? payload.writingMode : undefined,
      projectMode: typeof payload.projectMode === "string" ? payload.projectMode : undefined,
      creationExperience: typeof payload.creationExperience === "string" ? payload.creationExperience : undefined,
      narrativeForm: typeof payload.narrativeForm === "string" ? payload.narrativeForm : undefined,
      styleTone: typeof payload.styleTone === "string" ? payload.styleTone : undefined,
      defaultChapterLength: typeof payload.defaultChapterLength === "number" ? payload.defaultChapterLength : undefined,
      estimatedChapterCount: typeof payload.estimatedChapterCount === "number" ? payload.estimatedChapterCount : undefined,
      postGenerationStyleReviewEnabled: payload.postGenerationStyleReviewEnabled === true,
    });
    return ok(detailToNovel(created, [], []));
  }
  if (path === "/knowledge/documents" && method === "get") {
    const query = queryOf(config);
    const keyword = (query.get("keyword") ?? "").trim().toLocaleLowerCase();
    const documents = await transport.listKnowledgeDocuments();
    return ok(documents
      .filter((document) => !keyword || document.title.toLocaleLowerCase().includes(keyword))
      .map((document) => ({
        id: document.id,
        title: document.title,
        fileName: document.title,
        kind: "user_upload",
        sourceAnalysisId: null,
        status: "enabled",
        activeVersionId: null,
        activeVersionNumber: document.version,
        latestIndexStatus: document.indexStatus === "ready" ? "succeeded" : document.indexStatus === "failed" ? "failed" : "idle",
        latestIndexError: null,
        lastIndexedAt: null,
        createdAt: iso(document.createdAt),
        updatedAt: iso(document.createdAt),
        versionCount: 1,
        bookAnalysisCount: 0,
      })));
  }
  if (path === "/genres" && method === "get") {
    const genres = await transport.listGenres();
    const now = new Date().toISOString();
    return ok(genres.map((genre) => ({
      id: genre.id,
      name: genre.name,
      slug: genre.slug,
      description: genre.description,
      template: null,
      parentId: null,
      createdAt: now,
      updatedAt: now,
      childCount: 0,
      novelCount: 0,
      children: [],
    })));
  }
  if (path === "/story-modes" && method === "get") {
    const storyModes = await transport.listStoryModes();
    const now = new Date().toISOString();
    const emptyProfile = {
      coreDrive: "",
      readerReward: "",
      progressionUnits: [],
      allowedConflictForms: [],
      forbiddenConflictForms: [],
      conflictCeiling: "medium" as const,
      resolutionStyle: "",
      chapterUnit: "",
      volumeReward: "",
      mandatorySignals: [],
      antiSignals: [],
    };
    return ok(storyModes.map((mode) => ({
      id: mode.id,
      name: mode.name,
      slug: mode.slug,
      description: mode.description,
      template: null,
      parentId: null,
      profile: emptyProfile,
      createdAt: now,
      updatedAt: now,
      childCount: 0,
      novelCount: 0,
      children: [],
    })));
  }
  if (path === "/tasks/overview" && method === "get") {
    return ok(await transport.getTaskOverview());
  }
  if (path === "/tasks/recovery-candidates" && method === "get") {
    return ok(await transport.listRecoveryCandidates());
  }
  if (path === "/worlds" && method === "get") {
    const worlds = await transport.listWorlds();
    return ok(worlds.map((world) => ({
      id: world.id,
      name: world.name,
      description: world.description,
      overviewSummary: world.overviewSummary,
      status: world.status,
      version: 1,
      createdAt: iso(world.createdAt),
      updatedAt: iso(world.updatedAt),
    })));
  }
  const novelDetail = path.match(/^\/novels\/([^/]+)$/);
  if (novelDetail) {
    const id = decodeURIComponent(novelDetail[1]!);
    if (method === "get") return ok(await loadNovel(id));
    if (method === "put") {
      const payload = asRecord(config.data);
      await transport.updateNovel({
        novelId: id,
        title: typeof payload.title === "string" ? payload.title : undefined,
        description: typeof payload.description === "string" ? payload.description : undefined,
        targetAudience: typeof payload.targetAudience === "string" || payload.targetAudience === null ? payload.targetAudience : undefined,
        bookSellingPoint: typeof payload.bookSellingPoint === "string" || payload.bookSellingPoint === null ? payload.bookSellingPoint : undefined,
        competingFeel: typeof payload.competingFeel === "string" || payload.competingFeel === null ? payload.competingFeel : undefined,
        first30ChapterPromise: typeof payload.first30ChapterPromise === "string" || payload.first30ChapterPromise === null ? payload.first30ChapterPromise : undefined,
        commercialTags: Array.isArray(payload.commercialTags) ? payload.commercialTags.filter((item): item is string => typeof item === "string") : payload.commercialTags === null ? null : undefined,
        genreId: typeof payload.genreId === "string" || payload.genreId === null ? payload.genreId : undefined,
        primaryStoryModeId: typeof payload.primaryStoryModeId === "string" || payload.primaryStoryModeId === null ? payload.primaryStoryModeId : undefined,
        secondaryStoryModeId: typeof payload.secondaryStoryModeId === "string" || payload.secondaryStoryModeId === null ? payload.secondaryStoryModeId : undefined,
        worldId: typeof payload.worldId === "string" || payload.worldId === null ? payload.worldId : undefined,
        narrativeForm: typeof payload.narrativeForm === "string" ? payload.narrativeForm : undefined,
        writingMode: typeof payload.writingMode === "string" ? payload.writingMode : undefined,
        projectMode: typeof payload.projectMode === "string" ? payload.projectMode : undefined,
        narrativePov: typeof payload.narrativePov === "string" || payload.narrativePov === null ? payload.narrativePov : undefined,
        pacePreference: typeof payload.pacePreference === "string" || payload.pacePreference === null ? payload.pacePreference : undefined,
        styleTone: typeof payload.styleTone === "string" || payload.styleTone === null ? payload.styleTone : undefined,
        emotionIntensity: typeof payload.emotionIntensity === "string" || payload.emotionIntensity === null ? payload.emotionIntensity : undefined,
        aiFreedom: typeof payload.aiFreedom === "string" || payload.aiFreedom === null ? payload.aiFreedom : undefined,
        defaultChapterLength: typeof payload.defaultChapterLength === "number" || payload.defaultChapterLength === null ? payload.defaultChapterLength : undefined,
        estimatedChapterCount: typeof payload.estimatedChapterCount === "number" || payload.estimatedChapterCount === null ? payload.estimatedChapterCount : undefined,
        projectStatus: typeof payload.projectStatus === "string" ? payload.projectStatus : undefined,
        storylineStatus: typeof payload.storylineStatus === "string" ? payload.storylineStatus : undefined,
        outlineStatus: typeof payload.outlineStatus === "string" ? payload.outlineStatus : undefined,
        resourceReadyScore: typeof payload.resourceReadyScore === "number" ? payload.resourceReadyScore : undefined,
        status: typeof payload.status === "string" ? payload.status : undefined,
        postGenerationStyleReviewEnabled: typeof payload.postGenerationStyleReviewEnabled === "boolean"
          ? payload.postGenerationStyleReviewEnabled
          : undefined,
      });
      return ok(await loadNovel(id));
    }
    if (method === "delete") {
      await transport.deleteNovel(id);
      return ok(null);
    }
  }
  const chapters = path.match(/^\/novels\/([^/]+)\/chapters$/);
  if (chapters) {
    const novelId = decodeURIComponent(chapters[1]!);
    if (method === "get") {
      const rows = await transport.listChapters(novelId);
      return ok(rows.map<Chapter>((chapter) => ({
        id: chapter.id,
        novelId: chapter.novelId,
        title: chapter.title,
        content: chapter.content,
        order: chapter.sortOrder,
        officialVersion: chapter.officialVersion,
        protection: chapter.protection,
        createdAt: iso(chapter.updatedAt),
        updatedAt: iso(chapter.updatedAt),
      })));
    }
    if (method === "post") {
      const payload = asRecord(config.data);
      const created = await transport.createChapter({
        novelId,
        title: String(payload.title ?? "未命名章节").trim() || "未命名章节",
        sortOrder: Number(payload.order ?? 1) || 1,
        volumeId: typeof payload.volumeId === "string" ? payload.volumeId : undefined,
      });
      if (typeof payload.content === "string" && payload.content.length > 0) {
        const saved = await transport.saveChapter({
          novelId,
          chapterId: created.id,
          expectedVersion: 0,
          title: created.title,
          content: payload.content,
        });
        return ok({
          id: saved.id,
          novelId: saved.novelId,
          title: saved.title,
          content: saved.content,
          order: saved.sortOrder,
          officialVersion: saved.officialVersion,
          protection: saved.protection,
          updatedAt: iso(saved.updatedAt),
          createdAt: iso(saved.updatedAt),
        } satisfies Chapter);
      }
      return ok({
        id: created.id,
        novelId,
        title: created.title,
        content: "",
        order: created.sortOrder,
        officialVersion: 0,
        protection: created.protection,
        createdAt: iso(created.updatedAt),
        updatedAt: iso(created.updatedAt),
      } satisfies Chapter);
    }
  }
  const chapter = path.match(/^\/novels\/([^/]+)\/chapters\/([^/]+)$/);
  if (chapter) {
    const novelId = decodeURIComponent(chapter[1]!);
    const chapterId = decodeURIComponent(chapter[2]!);
    if (method === "get") {
      const row = await transport.getChapter({ novelId, chapterId });
      return ok({
        id: row.id,
        novelId: row.novelId,
        title: row.title,
        content: row.content,
        order: row.sortOrder,
        officialVersion: row.officialVersion,
        protection: row.protection,
        createdAt: iso(row.updatedAt),
        updatedAt: iso(row.updatedAt),
      } satisfies Chapter);
    }
    if (method === "put") return updateChapter(config, novelId, chapterId);
    if (method === "delete") {
      const payload = asRecord(config.data);
      const current = await transport.getChapter({ novelId, chapterId });
      await transport.deleteChapter({
        novelId,
        chapterId,
        expectedVersion: typeof payload.expectedVersion === "number" ? payload.expectedVersion : current.officialVersion,
      });
      return ok(null);
    }
  }
  const workspace = path.match(/^\/novels\/([^/]+)\/chapters\/([^/]+)\/editor\/workspace$/);
  if (workspace && method === "get") {
    const novelId = decodeURIComponent(workspace[1]!);
    const chapterId = decodeURIComponent(workspace[2]!);
    return ok(await transport.getChapterEditorWorkspace({ novelId, chapterId }));
  }
  // 首次上手面板要读的就绪状态：桌面版直接问本地内核，未映射会让面板一直报错。
  if (path === "/settings/quick-setup/status" && method === "get") {
    return ok(toQuickSetupStatus(await transport.getQuickSetupStatus()));
  }
  if (path === "/settings/model-setup" && method === "get") {
    return ok(await transport.getModelSetup());
  }
  if (path === "/settings/model-setup" && method === "post") {
    const payload = asRecord(config.data);
    return ok(
      await transport.saveModelSetup({
        endpoint: typeof payload.endpoint === "string" ? payload.endpoint : "",
        model: typeof payload.model === "string" ? payload.model : "",
        allowLocalhostHttp: payload.allowLocalhostHttp === true,
        apiKey: typeof payload.apiKey === "string" ? payload.apiKey : "",
      }),
    );
  }
  return unsupported(config);
}

export const desktopApiAdapter: AxiosAdapter = async (config) => {
  try {
    const exportMatch = pathOf(config).match(/^\/novels\/([^/]+)\/export$/);
    if (exportMatch && methodOf(config) === "get") {
      return await exportNovelFile(config, decodeURIComponent(exportMatch[1]!));
    }
    const data = await route(config);
    return {
      data,
      status: 200,
      statusText: "OK",
      headers: new AxiosHeaders(),
      config,
      request: null,
    } satisfies AxiosResponse;
  } catch (error) {
    if (error instanceof AxiosErrorClass) throw error;
    const normalized = appError(error);
    throw new AxiosErrorClass(
      normalized.message,
      normalized.code,
      config,
      null,
      {
        status: normalized.code === "NOT_FOUND" ? 404 : normalized.code === "CONFLICT" ? 409 : 400,
        statusText: normalized.code,
        headers: {},
        config,
        data: { success: false, error: normalized.code, message: normalized.message },
      },
    );
  }
};

export function shouldUseDesktopApiAdapter(): boolean {
  return APP_RUNTIME === "desktop" && transport.hasDesktopShell();
}