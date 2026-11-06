import { useRef } from "react";
import { useMutation, type QueryClient } from "@tanstack/react-query";
import type { Chapter, PipelineRepairMode, PipelineRunMode, VolumePlanDocument } from "@ai-novel/shared/types/novel";
import type { LLMProvider } from "@ai-novel/shared/types/llm";
import {
  createNovelChapter,
  deleteNovelChapter,
  generateChapterHook,
  optimizeNovelOutlinePreview,
  optimizeNovelStructuredOutlinePreview,
  reviewNovelChapter,
  runNovelPipeline,
  updateNovel,
  syncNovelVolumeChapters,
  updateNovelVolumes,
  recommendNovelWritingPlatform,
  updateNovelWritingPlatform,
} from "@/api/novel";
import { queryKeys } from "@/api/queryKeys";
import { buildNovelUpdatePayload, type NovelBasicFormState } from "../novelBasicInfo.shared";
import { buildNewChapterDraft } from "../chapterCreationPolicy";
import type { ChapterReviewResult } from "../chapterPlanning.shared";
import type { StructuredSyncOptions } from "../novelEdit.utils";
import { syncNovelWorkflowStageSilently } from "../novelWorkflow.client";

interface LlmSettings {
  provider?: LLMProvider;
  model?: string;
  temperature?: number;
}

interface PipelineFormState {
  startOrder: number;
  endOrder: number;
  maxRetries: number;
  runMode: PipelineRunMode;
  autoReview: boolean;
  autoRepair: boolean;
  skipCompleted: boolean;
  qualityThreshold: number;
  repairMode: PipelineRepairMode;
}

interface UseNovelEditMutationsArgs {
  id: string;
  basicForm: NovelBasicFormState;
  hasCharacters: boolean;
  outlineText: string;
  outlineOptimizeInstruction: string;
  setOutlineOptimizePreview: (value: string) => void;
  setOutlineOptimizeMode: (value: "full" | "selection") => void;
  setOutlineOptimizeSourceText: (value: string) => void;
  structuredDraftText: string;
  structuredOptimizeInstruction: string;
  setStructuredOptimizePreview: (value: string) => void;
  setStructuredOptimizeMode: (value: "full" | "selection") => void;
  setStructuredOptimizeSourceText: (value: string) => void;
  volumeDocument: VolumePlanDocument;
  llm: LlmSettings;
  pipelineForm: PipelineFormState;
  selectedChapterId: string;
  chapters: Chapter[];
  setActiveTab: (value: string) => void;
  setSelectedChapterId: (value: string) => void;
  setCurrentJobId: (value: string) => void;
  setPipelineMessage: (value: string) => void;
  setStructuredMessage: (value: string) => void;
  setChapterOperationMessage: (value: string) => void;
  setReviewResult: (value: ChapterReviewResult | null) => void;
  queryClient: QueryClient;
  invalidateNovelDetail: () => Promise<void>;
}

export function useNovelEditMutations({
  id,
  basicForm,
  hasCharacters,
  outlineText,
  outlineOptimizeInstruction,
  setOutlineOptimizePreview,
  setOutlineOptimizeMode,
  setOutlineOptimizeSourceText,
  structuredDraftText,
  structuredOptimizeInstruction,
  setStructuredOptimizePreview,
  setStructuredOptimizeMode,
  setStructuredOptimizeSourceText,
  volumeDocument,
  llm,
  pipelineForm,
  selectedChapterId,
  chapters,
  setActiveTab,
  setSelectedChapterId,
  setCurrentJobId,
  setPipelineMessage,
  setStructuredMessage,
  setChapterOperationMessage,
  setReviewResult,
  queryClient,
  invalidateNovelDetail,
}: UseNovelEditMutationsArgs) {
  // 同步闸门：isPending 要等下一次渲染才反映到按钮上，同一任务里的连续点击
  // 仍可能重复进入 mutation；这里在调用瞬间拦住第二次创建。
  const createRequestPendingRef = useRef(false);
  // mutation 的 onSuccess 闭包可能属于上一本书；切书后不能把旧章节选中到新书里。
  const currentNovelIdRef = useRef(id);
  currentNovelIdRef.current = id;
  // 创建成功到详情刷新完成之间列表可能还是旧数据，记录刚发出的顺序防止重复编号。
  // 顺序必须按书记忆，切到另一本书后不能沿用上一本书的章节号。
  const lastCreatedChapterRef = useRef<{ novelId: string; order: number } | null>(null);
  const saveBasicMutation = useMutation({
    mutationFn: async () => {
      const updated = await updateNovel(id, buildNovelUpdatePayload(basicForm));
      const platform = basicForm.writingPlatformPreference === "ai_recommend"
        ? (await recommendNovelWritingPlatform(id)).data?.platform
        : basicForm.writingPlatformPreference;
      if (!platform) throw new Error("AI 未返回可用的平台建议，请重试。");
      await updateNovelWritingPlatform(id, platform);
      return updated;
    },
    onSuccess: async () => {
      await syncNovelWorkflowStageSilently({
        novelId: id,
        stage: "project_setup",
        itemLabel: "项目设定已保存",
        status: "waiting_approval",
      });
      await invalidateNovelDetail();
      if (!hasCharacters) {
        setActiveTab("character");
      }
    },
  });

  const saveOutlineMutation = useMutation({
    mutationFn: () => updateNovelVolumes(id, volumeDocument),
    onSuccess: async () => {
      await syncNovelWorkflowStageSilently({
        novelId: id,
        stage: "volume_strategy",
        itemLabel: "卷战略 / 卷骨架已保存",
        checkpointType: "volume_strategy_ready",
        checkpointSummary: "当前卷战略与卷骨架已保存到工作区。",
        status: "waiting_approval",
      });
      await invalidateNovelDetail();
    },
  });

  const saveStructuredMutation = useMutation({
    mutationFn: () => updateNovelVolumes(id, {
      ...volumeDocument,
      syncToChapterExecution: true,
    }),
    onSuccess: async () => {
      setStructuredMessage("节奏拆章已保存，章节执行区会直接使用同一批章节。");
      await syncNovelWorkflowStageSilently({
        novelId: id,
        stage: "structured_outline",
        itemLabel: "节奏 / 拆章已保存",
        status: "waiting_approval",
      });
      await invalidateNovelDetail();
    },
  });

  const optimizeOutlineMutation = useMutation({
    mutationFn: (payload: { mode: "full" | "selection"; selectedText?: string }) =>
      optimizeNovelOutlinePreview(id, {
        currentDraft: outlineText,
        instruction: outlineOptimizeInstruction,
        mode: payload.mode,
        selectedText: payload.selectedText,
        provider: llm.provider,
        model: llm.model,
        temperature: llm.temperature,
      }),
    onSuccess: (response) => {
      setOutlineOptimizePreview(response.data?.optimizedDraft ?? "");
      setOutlineOptimizeMode(response.data?.mode ?? "full");
      setOutlineOptimizeSourceText(response.data?.selectedText ?? "");
    },
  });

  const optimizeStructuredMutation = useMutation({
    mutationFn: (payload: { mode: "full" | "selection"; selectedText?: string }) =>
      optimizeNovelStructuredOutlinePreview(id, {
        currentDraft: structuredDraftText,
        instruction: structuredOptimizeInstruction,
        mode: payload.mode,
        selectedText: payload.selectedText,
        provider: llm.provider,
        model: llm.model,
        temperature: llm.temperature,
      }),
    onSuccess: (response) => {
      setStructuredOptimizePreview(response.data?.optimizedDraft ?? "");
      setStructuredOptimizeMode(response.data?.mode ?? "full");
      setStructuredOptimizeSourceText(response.data?.selectedText ?? "");
    },
  });

  const syncStructuredChaptersMutation = useMutation({
    mutationFn: (options: StructuredSyncOptions) => syncNovelVolumeChapters(id, {
      volumes: volumeDocument.volumes,
      preserveContent: options.preserveContent,
      applyDeletes: options.applyDeletes,
    }),
    onSuccess: async (response) => {
      const preview = response.data;
      setStructuredMessage(
        `连接修复完成：新增 ${preview?.createCount ?? 0}，更新 ${preview?.updateCount ?? 0}，删除 ${preview?.deleteCount ?? 0}。`,
      );
      await syncNovelWorkflowStageSilently({
        novelId: id,
        stage: "structured_outline",
        itemLabel: "卷级拆章已连接到章节执行",
        checkpointType: "chapter_batch_ready",
        checkpointSummary: "章节列表、任务单和执行入口已准备好，可继续进入章节执行。",
        status: "waiting_approval",
      });
      await invalidateNovelDetail();
    },
    onError: (error) => {
      const message = error instanceof Error ? error.message : "章节同步失败。";
      setStructuredMessage(message);
    },
  });

  const createChapterMutation = useMutation({
    mutationFn: async () => {
      const novelId = id;
      const lastCreated = lastCreatedChapterRef.current;
      const draft = buildNewChapterDraft(
        chapters,
        lastCreated?.novelId === novelId ? lastCreated.order : 0,
      );
      const response = await createNovelChapter(novelId, {
        title: draft.title,
        order: draft.order,
        content: "",
      });
      return { response, novelId, order: draft.order };
    },
    onMutate: () => {
      setChapterOperationMessage("");
    },
    onSuccess: ({ response, novelId, order }) => {
      // 创建请求在切书后才返回时，只刷新原书数据，不把旧章节选中到新书。
      if (novelId === currentNovelIdRef.current) {
        lastCreatedChapterRef.current = { novelId, order };
        if (response.data?.id) {
          setSelectedChapterId(response.data.id);
        }
      }
      // 章节已经创建成功，先结束按钮的“创建中”状态。详情、质量报告等十几个查询
      // 会在这里依次刷新；如果等它们全部完成，新建按钮会被锁住几十秒。
      void syncNovelWorkflowStageSilently({
        novelId,
        stage: "chapter_execution",
        itemLabel: "已创建新的章节执行项",
        chapterId: response.data?.id,
        status: "waiting_approval",
      });
      void invalidateNovelDetail().catch(() => undefined);
    },
    onError: (error) => {
      setChapterOperationMessage(
        error instanceof Error ? error.message : "新建章节失败，请稍后重试。",
      );
    },
    onSettled: () => {
      createRequestPendingRef.current = false;
    },
  });

  const requestCreateChapter = () => {
    if (createRequestPendingRef.current) {
      return;
    }
    createRequestPendingRef.current = true;
    createChapterMutation.mutate();
  };

  const deleteManualChapterMutation = useMutation({
    mutationFn: (chapterId: string) => deleteNovelChapter(id, chapterId),
    onSuccess: async (_response, chapterId) => {
      const deletedIndex = chapters.findIndex((chapter) => chapter.id === chapterId);
      const fallbackChapter = chapters[deletedIndex + 1] ?? chapters[deletedIndex - 1] ?? null;
      if (selectedChapterId === chapterId) {
        setSelectedChapterId(fallbackChapter?.id ?? "");
      }
      setPipelineMessage("已移除未开始的空白章节。");
      await invalidateNovelDetail();
    },
    onError: (error) => {
      setPipelineMessage(error instanceof Error ? error.message : "移除章节失败，请稍后重试。");
    },
  });

  const runPipelineMutation = useMutation({
    mutationFn: (override?: Partial<PipelineFormState>) =>
      runNovelPipeline(id, {
        startOrder: override?.startOrder ?? pipelineForm.startOrder,
        endOrder: override?.endOrder ?? pipelineForm.endOrder,
        maxRetries: override?.maxRetries ?? pipelineForm.maxRetries,
        runMode: override?.runMode ?? pipelineForm.runMode,
        autoReview: override?.autoReview ?? pipelineForm.autoReview,
        autoRepair: override?.autoRepair ?? pipelineForm.autoRepair,
        skipCompleted: override?.skipCompleted ?? pipelineForm.skipCompleted,
        qualityThreshold: override?.qualityThreshold ?? pipelineForm.qualityThreshold,
        repairMode: override?.repairMode ?? pipelineForm.repairMode,
        provider: llm.provider,
        model: llm.model,
        temperature: llm.temperature,
      }),
    onSuccess: async (response) => {
      if (response.data?.id) {
        setCurrentJobId(response.data.id);
      }
      setPipelineMessage(response.message ?? "Pipeline started.");
      await syncNovelWorkflowStageSilently({
        novelId: id,
        stage: "quality_repair",
        itemLabel: "章节流水线运行中",
        status: "running",
      });
      await queryClient.invalidateQueries({ queryKey: queryKeys.novels.pipelineJob(id, response.data?.id ?? "none") });
    },
  });

  const reviewMutation = useMutation({
    mutationFn: () =>
      reviewNovelChapter(id, selectedChapterId, {
        provider: llm.provider,
        model: llm.model,
        temperature: 0.1,
      }),
    onSuccess: async (response) => {
      setReviewResult(response.data ?? null);
      setPipelineMessage("Chapter reviewed.");
      await syncNovelWorkflowStageSilently({
        novelId: id,
        stage: "quality_repair",
        itemLabel: "章节审校已完成",
        status: "waiting_approval",
      });
      await queryClient.invalidateQueries({ queryKey: queryKeys.novels.qualityReport(id) });
    },
  });

  const hookMutation = useMutation({
    mutationFn: () =>
      generateChapterHook(id, {
        chapterId: selectedChapterId || undefined,
        provider: llm.provider,
        model: llm.model,
        temperature: llm.temperature,
      }),
    onSuccess: async () => {
      setPipelineMessage("Chapter hook generated.");
      await syncNovelWorkflowStageSilently({
        novelId: id,
        stage: "chapter_execution",
        itemLabel: "章节钩子已生成",
        chapterId: selectedChapterId || undefined,
        status: "waiting_approval",
      });
      await invalidateNovelDetail();
    },
  });

  return {
    saveBasicMutation,
    saveOutlineMutation,
    saveStructuredMutation,
    optimizeOutlineMutation,
    optimizeStructuredMutation,
    syncStructuredChaptersMutation,
    createChapterMutation,
    requestCreateChapter,
    deleteManualChapterMutation,
    runPipelineMutation,
    reviewMutation,
    hookMutation,
  };
}
