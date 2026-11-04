import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type {
  ChapterEditorDiagnosticCard,
  ChapterEditorCursorContinuationRequest,
  ChapterEditorCursorOperation,
  ChapterEditorOperation,
  ChapterEditorRecommendedTask,
  ChapterEditorRevisionScope,
  ChapterEditorTargetRange,
} from "@ai-novel/shared/types/novel";
import { useCandidateAcceptance, useCandidateReview } from "./review";
import {
  readChapterQualityDebtDetails,
  type ChapterQualityDebtDetails,
} from "@ai-novel/shared/types/chapterQualityLoop";
import { AlertTriangle, Loader2 } from "lucide-react";
import { createNovelSnapshot, previewChapterAiRevisionWithRouting, previewChapterCursorContinuationWithRouting, resolveChapterAuditIssue, reviewNovelChapter, updateNovelChapter } from "@/api/novel";
import {
  rewriteFailureMessage,
  rewriteBillingOutcome,
  rewriteRetryWarning,
  requiresRetryConfirmation,
  type RewriteBillingOutcome,
} from "@/api/novel/chapterSelectionRewriteBilling";
import { listPendingSelectionRewrites } from "@/platform/desktopTransport";
import { queryKeys } from "@/api/queryKeys";import { Button } from "@/components/ui/button";
import { toast } from "@/components/ui/toast";
import { useLLMStore } from "@/store/llmStore";
import ChapterEditorDirectorPanel from "./ChapterEditorDirectorPanel";
import ChapterRightPanel from "./rightPanel/ChapterRightPanel";
import { readStoredPanelTab, storePanelTab, type PanelTab } from "./rightPanel/panelModel";
import { hasPendingCandidate, isResultForCurrentChapter, isSaveShortcut, shouldBlockChapterSwitch } from "./navigation/chapterNavigatorModel";
import ChapterEditorSidebar from "./ChapterEditorSidebar";
import ChapterVersionDrawer from "./ChapterVersionDrawer";
import ChapterTextEditor from "./ChapterTextEditor";
import SelectionAIFloatingToolbar from "./SelectionAIFloatingToolbar";
import CursorAIFloatingToolbar from "./CursorAIFloatingToolbar";
import type {
  ChapterEditorSelectionRange,
  ChapterEditorSessionState,
  ChapterEditorShellProps,
  SelectionToolbarPosition,
} from "./chapterEditorTypes";
import {
  CHAPTER_EDITOR_CURSOR_OPERATION_LABELS,
  CHAPTER_EDITOR_OPERATION_LABELS,
  buildAiRevisionRequest,
  countEditorWords,
  getSaveStatusLabel,
  getParagraphWindow,
  normalizeChapterContent,
  useChapterDraft,
  buildEditPositionRecord,
  readEditPosition,
  resolveEditPositionRestore,
  writeEditPosition,
  writeRecentEditTarget,
} from "./document";

const EMPTY_SESSION: ChapterEditorSessionState = {
  sessionId: "",
  scope: "selection",
  targetRange: {
    from: 0,
    to: 0,
    text: "",
  },
  candidates: [],
  activeCandidateId: null,
  status: "idle",
  viewMode: "block",
  mode: "revision",
};

function formatQualityDebtSource(source: string | null): string {
  if (source === "repair_recheck") return "自动修复后的复审";
  if (source === "pipeline_review") return "AI 自动审校";
  if (source === "manual_review") return "手动审校";
  return "历史质量记录";
}

function formatQualityDebtAttempts(details: ChapterQualityDebtDetails): string {
  if (details.repairAttemptsUsed === null) {
    return "历史记录未保存自动修复次数";
  }
  if (details.repairAttemptsAllowed === 0) {
    return `${details.repairAttemptsUsed} 次，本次未启用自动修复`;
  }
  return `${details.repairAttemptsUsed}/${details.repairAttemptsAllowed} 次`;
}

function toSelectionFromRange(
  content: string,
  range?: Pick<ChapterEditorTargetRange, "from" | "to"> | null,
): ChapterEditorSelectionRange | null {
  if (!range) {
    return null;
  }
  if (range.from < 0 || range.to <= range.from || range.to > content.length) {
    return null;
  }
  const text = content.slice(range.from, range.to);
  if (!text.trim()) {
    return null;
  }
  return {
    from: range.from,
    to: range.to,
    text,
  };
}

export default function ChapterEditorShell(props: ChapterEditorShellProps) {
  const {
    chapters = [],
    volumes = [],
    onSelectChapter,
    runAttention = {},
    world = null,
    characters = [],
    novelId,
    chapter,
    workspace,
    workspaceStatus,
    onBack,
    onOpenVersionHistory,
    onRefreshWorkspace,
    isRefreshingWorkspace,
  } = props;
  const llm = useLLMStore();
  const queryClient = useQueryClient();
  const lastPreviewRequestRef = useRef<ReturnType<typeof buildAiRevisionRequest> | null>(null);
  const lastContinuationRequestRef = useRef<ChapterEditorCursorContinuationRequest | null>(null);
  // 发起请求时所在的章节：晚到的响应必须与当前章节一致才能写入会话。
  const previewChapterRef = useRef<string | null>(null);
  const continuationChapterRef = useRef<string | null>(null);
  const qualityDebtDetails = useMemo(
    () => readChapterQualityDebtDetails(chapter?.riskFlags),
    [chapter?.riskFlags],
  );

  const [saveStatus, setSaveStatus] = useState<"idle" | "saving" | "saved" | "error">("idle");
  const [selection, setSelection] = useState<ChapterEditorSelectionRange | null>(null);
  const [selectionToolbarPosition, setSelectionToolbarPosition] = useState<SelectionToolbarPosition | null>(null);
  const [cursorOffset, setCursorOffset] = useState<number | null>(null);
  const [cursorToolbarPosition, setCursorToolbarPosition] = useState<SelectionToolbarPosition | null>(null);
  const [session, setSession] = useState<ChapterEditorSessionState>(EMPTY_SESSION);
  const [revisionScope, setRevisionScope] = useState<ChapterEditorRevisionScope>("selection");
  const [revisionInstruction, setRevisionInstruction] = useState("");
  const [selectedDiagnosticId, setSelectedDiagnosticId] = useState<string | null>(null);
  const [isVersionDrawerOpen, setIsVersionDrawerOpen] = useState(false);
  const [loadedSnapshotId, setLoadedSnapshotId] = useState<string | null>(null);
  const [savedOfficialVersion, setSavedOfficialVersion] = useState(chapter?.officialVersion ?? 0);

  // 本章仍处于「可能已计费、结果未确认」的改写：重新生成前要提示。
  const pendingRewritesQuery = useQuery({
    queryKey: queryKeys.novels.pendingSelectionRewrites(novelId, chapter?.id ?? ""),
    queryFn: () => listPendingSelectionRewrites({ novelId, chapterId: chapter?.id ?? "" }),
    enabled: Boolean(chapter?.id),
    refetchOnWindowFocus: true,
  });
  const pendingRewriteCount = pendingRewritesQuery.data?.length ?? 0;

  const draft = useChapterDraft({
    novelId,
    chapter: chapter ? { id: chapter.id, content: chapter.content, updatedAt: String(chapter.updatedAt), officialVersion: chapter.officialVersion } : undefined,
    busy: session.status !== "idle",
  });
  const { contentDraft, savedContent, isDirty, hasExternalChange, restored, storageError } = draft;

  useEffect(() => {
    setSaveStatus("idle");
    setSelection(null);
    setSelectionToolbarPosition(null);
    setCursorOffset(null);
    setCursorToolbarPosition(null);
    setSession(EMPTY_SESSION);
    setRevisionInstruction("");
    setRevisionScope("selection");
    setIsVersionDrawerOpen(false);
    setLoadedSnapshotId(null);
    setSavedOfficialVersion(chapter?.officialVersion ?? 0);
    lastPreviewRequestRef.current = null;
    lastContinuationRequestRef.current = null;
    previewChapterRef.current = null;
    continuationChapterRef.current = null;
  }, [chapter?.id]);

  useEffect(() => {
    if (!workspace) {
      setSelectedDiagnosticId(null);
      return;
    }
    if (selectedDiagnosticId && !workspace.diagnosticCards.some((card) => card.id === selectedDiagnosticId)) {
      setSelectedDiagnosticId(null);
    }
  }, [selectedDiagnosticId, workspace]);

  const wordCount = useMemo(() => countEditorWords(contentDraft), [contentDraft]);
  const activeCandidate = useMemo(
    () => session.candidates?.find((candidate) => candidate.id === session.activeCandidateId) ?? session.candidates?.[0] ?? null,
    [session.activeCandidateId, session.candidates],
  );
  // 候选稿待确认：切章或离开正文编辑都会丢弃它，导航与离开守卫共用这一个判定。
  const isCandidatePending = useMemo(() => hasPendingCandidate(session), [session]);
  const selectedDiagnosticCard = useMemo(
    () => workspace?.diagnosticCards.find((card) => card.id === selectedDiagnosticId) ?? null,
    [selectedDiagnosticId, workspace],
  );
  const selectedDiagnosticSelection = useMemo(
    () => toSelectionFromRange(contentDraft, selectedDiagnosticCard?.anchorRange ?? null),
    [contentDraft, selectedDiagnosticCard?.anchorRange],
  );
  const recommendedTaskSelection = useMemo(
    () => toSelectionFromRange(contentDraft, workspace?.recommendedTask?.anchorRange ?? null),
    [contentDraft, workspace?.recommendedTask?.anchorRange],
  );

  const invalidateChapterQueries = async () => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: queryKeys.novels.detail(novelId) }),
      queryClient.invalidateQueries({ queryKey: queryKeys.novels.simpleShelf(novelId) }),
      queryClient.invalidateQueries({ queryKey: queryKeys.novels.qualityReport(novelId) }),
      queryClient.invalidateQueries({ queryKey: queryKeys.novels.snapshots(novelId) }),
      chapter?.id
        ? queryClient.invalidateQueries({ queryKey: queryKeys.novels.chapterPlan(novelId, chapter.id) })
        : Promise.resolve(),
      chapter?.id
        ? queryClient.invalidateQueries({ queryKey: queryKeys.novels.chapterSnapshots(novelId, chapter.id) })
        : Promise.resolve(),
      chapter?.id
        ? queryClient.invalidateQueries({ queryKey: queryKeys.novels.chapterAuditReports(novelId, chapter.id) })
        : Promise.resolve(),
      queryClient.invalidateQueries({ queryKey: queryKeys.novels.latestStateSnapshot(novelId) }),
    ]);
  };

  const leaveEditor = (action?: () => void) => {
    const blocked = shouldBlockChapterSwitch(
      {
        hasUnsavedContent: isDirty,
        isDraftWriteBusy,
        hasPendingCandidate: isCandidatePending,
      },
      (message) => window.confirm(message),
      "leave_editor",
    );
    if (blocked) {
      return;
    }
    action?.();
  };

  const saveMutation = useMutation({
    mutationFn: async (nextContent: string) => {
      if (!chapter) {
        throw new Error("当前未选中章节。");
      }
      if (loadedSnapshotId) {
        await createNovelSnapshot(novelId, {
          triggerType: "manual",
          label: `chapter-editor:before-version-load:${chapter.order}:${Date.now()}`,
        });
      }
      return updateNovelChapter(novelId, chapter.id, {
        content: nextContent,
        expectedUpdatedAt: draft.savedUpdatedAt,
        expectedVersion: savedOfficialVersion,
      });
    },
    onMutate: () => {
      setSaveStatus("saving");
    },
    onSuccess: async (response, nextContent) => {
      draft.acknowledgeSave(nextContent, { content: response.data?.content, updatedAt: String(response.data?.updatedAt ?? Date.now()) });
      if (typeof response.data?.officialVersion === "number") {
        setSavedOfficialVersion(response.data.officialVersion);
      }
      setLoadedSnapshotId(null);
      setSaveStatus("saved");
      await invalidateChapterQueries();
      toast.success("章节正文已保存。");
    },
    onError: (error) => {
      setSaveStatus("error");
      toast.error(error instanceof Error ? error.message : "章节保存失败。");
    },
  });

  // 正文写入还没落地：保存进行中，或本地草稿写入失败。
  // 切章、离开正文编辑与载入版本三个出口共用这一个判定。
  const isDraftWriteBusy = saveMutation.isPending || Boolean(draft.storageError);

  useEffect(() => {
    const handleSaveShortcut = (event: KeyboardEvent) => {
      if (!(event.ctrlKey || event.metaKey) || event.key.toLowerCase() !== "s") {
        return;
      }
      event.preventDefault();
      if (chapter && isDirty && !saveMutation.isPending) {
        saveMutation.mutate(contentDraft);
      }
    };
    window.addEventListener("keydown", handleSaveShortcut);
    return () => window.removeEventListener("keydown", handleSaveShortcut);
  }, [chapter, contentDraft, isDirty, saveMutation]);

  const reviewMutation = useMutation({
    mutationFn: async () => {
      if (!chapter) {
        throw new Error("当前未选中章节。");
      }
      if (isDirty || saveMutation.isPending) {
        throw new Error("请先保存正文，再重新审校。");
      }
      return reviewNovelChapter(novelId, chapter.id, {
        provider: llm.provider,
        model: llm.model,
        temperature: 0.1,
        content: savedContent,
      });
    },
    onSuccess: async (response) => {
      await invalidateChapterQueries();
      const assessment = response.data?.qualityAssessment;
      toast.success(assessment?.recommendedAction === "continue"
        ? "重新审校通过，本章待优化项已关闭。"
        : "重新审校完成，AI 仍发现需要处理的内容。");
    },
    onError: (error) => {
      toast.error(error instanceof Error ? error.message : "重新审校失败，请稍后重试。");
    },
  });

  const resolveDiagnosticMutation = useMutation({
    mutationFn: async (issueId: string) => resolveChapterAuditIssue(novelId, issueId),
    onSuccess: async () => {
      setSelectedDiagnosticId(null);
      await invalidateChapterQueries();
      onRefreshWorkspace?.();
      toast.success("问题已标记为处理完成，重新分析后会确认正文状态。");
    },
    onError: (error) => {
      toast.error(error instanceof Error ? error.message : "问题状态更新失败，请稍后重试。");
    },
  });

  const previewMutation = useMutation({
    mutationFn: async (request: ReturnType<typeof buildAiRevisionRequest>) => {
      if (!chapter) {
        throw new Error("当前未选中章节。");
      }
      return previewChapterAiRevisionWithRouting(novelId, chapter.id, request);
    },
    onMutate: (request) => {
      lastPreviewRequestRef.current = request;
      // 记住发起时的章节：响应回来时章节可能已经切走，晚到的结果只能丢弃。
      previewChapterRef.current = chapter?.id ?? null;
      const label = request.source === "freeform"
        ? (request.scope === "chapter" ? "正在生成整章自然语言修正方案" : "正在按你的意见改写片段")
        : request.presetOperation
          ? `正在生成${CHAPTER_EDITOR_OPERATION_LABELS[request.presetOperation]}方案`
          : "正在生成修正方案";
      setSession((current) => ({
        ...current,
        mode: "revision",
        status: "loading",
        requestLabel: label,
        customInstruction: request.instruction,
        scope: request.scope,
        targetRange: request.selection ?? {
          from: 0,
          to: contentDraft.length,
          text: contentDraft,
        },
        candidates: [],
        activeCandidateId: null,
        errorMessage: undefined,
        billingOutcome: undefined,
      }));
    },
    onSuccess: (response: unknown) => {
      // 慢响应可能在作者切章之后才回来：不能把上一章的候选写进当前章的编辑器。
      if (!isResultForCurrentChapter(previewChapterRef.current, chapter?.id)) {
        return;
      }
      // 【类型处理】处理来自本地或云 API 的响应
      const data = (response as any)?.data;
      if (!data) {
        setSession((current) => ({
          ...current,
          status: "error",
          errorMessage: "AI 未返回改写结果，请重试。",
        }));
        return;
      }
      setSession((current) => ({
        ...data,
        mode: "revision",
        status: "ready",
        viewMode: "block",
        activeCandidateId: data.activeCandidateId ?? data.candidates[0]?.id ?? null,
        requestLabel: current.requestLabel,
        errorMessage: undefined,
        billingOutcome: undefined,
      }));
      setSelection(null);
      setSelectionToolbarPosition(null);
      // 成功后把「待确认改写」刷新掉，避免下一次重新生成继续提示旧请求。
      void queryClient.invalidateQueries({
        queryKey: queryKeys.novels.pendingSelectionRewrites(novelId, chapter?.id ?? ""),
      });
    },
    onError: (error) => {
      // 上一章的失败同样不能出现在当前章的错误面板里。
      if (!isResultForCurrentChapter(previewChapterRef.current, chapter?.id)) {
        return;
      }
      // 失败原因决定这条错误能不能说成「一定没扣费」：结果未知要单独呈现。
      const outcome = rewriteBillingOutcome(error);
      setSession((current) => ({
        ...current,
        status: "error",
        errorMessage: rewriteFailureMessage(error, outcome),
        billingOutcome: outcome,
      }));
      // 未确认的记录此刻才落库，刷新后重新生成按钮才能给出准确提示。
      void queryClient.invalidateQueries({
        queryKey: queryKeys.novels.pendingSelectionRewrites(novelId, chapter?.id ?? ""),
      });
    },
  });

  const continuationMutation = useMutation({
    mutationFn: async (request: ChapterEditorCursorContinuationRequest) => {
      if (!chapter) {
        throw new Error("当前未选中章节。");
      }
      return previewChapterCursorContinuationWithRouting(novelId, chapter.id, request);
    },
    onMutate: (request) => {
      lastContinuationRequestRef.current = request;
      // 续写同样记录发起章节：晚到的候选不能落到别的章。
      continuationChapterRef.current = chapter?.id ?? null;
      setSession((current) => ({
        ...current,
        mode: "continuation",
        status: "loading",
        requestLabel: `正在生成${CHAPTER_EDITOR_CURSOR_OPERATION_LABELS[request.operation]}方案`,
        continuationOperation: request.operation,
        scope: "selection",
        targetRange: {
          from: request.cursorOffset,
          to: request.cursorOffset,
          text: "",
        },
        candidates: [],
        activeCandidateId: null,
        errorMessage: undefined,
      }));
    },
    onSuccess: (response: unknown) => {
      if (!isResultForCurrentChapter(continuationChapterRef.current, chapter?.id)) {
        return;
      }
      // 【类型处理】处理来自本地或云 API 的响应
      const data = (response as any)?.data;
      if (!data) {
        setSession((current) => ({ ...current, status: "error", errorMessage: "AI 未返回续写结果，请重试。" }));
        return;
      }
      setSession((current) => ({
        ...current,
        ...data,
        mode: "continuation",
        continuationOperation: data.operation,
        status: "ready",
        viewMode: "block",
        activeCandidateId: data.activeCandidateId ?? data.candidates[0]?.id ?? null,
        errorMessage: undefined,
      }));
      setSelection(null);
      setSelectionToolbarPosition(null);
      setCursorOffset(null);
      setCursorToolbarPosition(null);
    },
    onError: (error) => {
      if (!isResultForCurrentChapter(continuationChapterRef.current, chapter?.id)) {
        return;
      }
      setSession((current) => ({
        ...current,
        status: "error",
        errorMessage: error instanceof Error ? error.message : "AI 续写失败，请重试。",
      }));
    },
  });

  const review = useCandidateReview(session.sessionId ?? "", session.targetRange?.text ?? "", activeCandidate);
  const acceptMutation = useCandidateAcceptance({
    novelId, chapter, session, candidate: activeCandidate,
    selectedChangeIds: review.selectedChangeIds,
    content: contentDraft,
    sourceSnapshot: session.mode === "continuation"
      ? lastContinuationRequestRef.current?.contentSnapshot : lastPreviewRequestRef.current?.contentSnapshot,
    savedUpdatedAt: draft.savedUpdatedAt,
    acknowledgeSave: draft.acknowledgeSave,
    onAccepted: async () => {
      setSaveStatus("saved");
      setSession(EMPTY_SESSION);
      setRevisionInstruction("");
      await invalidateChapterQueries();
      setLoadedSnapshotId(null);
    },
  });

  const previewPayload = session.status === "loading" && session.targetRange
    ? {
      mode: "loading" as const,
      from: session.targetRange.from,
      to: session.targetRange.to,
      originalText: session.targetRange.text,
    }
    : session.status === "ready" && activeCandidate && session.targetRange && !review.error
      ? {
        mode: session.viewMode,
        from: session.targetRange.from,
        to: session.targetRange.to,
        diffChunks: activeCandidate.diffChunks,
        selectedChangeIds: review.selectedChangeIds,
        originalText: session.targetRange.text,
        candidateText: activeCandidate.content,
      }
      : null;

  if (!chapter) {
    return (
      <div className="rounded-3xl border border-dashed border-border/70 bg-muted/10 p-10 text-center text-sm text-muted-foreground">
        请选择一个章节后开始编辑正文。
      </div>
    );
  }

  const getSelectionTarget = (
    overrideSelection?: ChapterEditorSelectionRange | null,
    task?: ChapterEditorRecommendedTask | null,
  ) => overrideSelection
    ?? selection
    ?? selectedDiagnosticSelection
    ?? toSelectionFromRange(contentDraft, task?.anchorRange ?? null)
    ?? recommendedTaskSelection
    ?? null;

  const runRevision = (
    source: "preset" | "freeform",
    scope: ChapterEditorRevisionScope,
    options?: {
      presetOperation?: ChapterEditorOperation;
      instruction?: string;
      selectionOverride?: ChapterEditorSelectionRange | null;
      task?: ChapterEditorRecommendedTask | null;
    },
  ) => {
    const resolvedSelection = scope === "selection"
      ? getSelectionTarget(options?.selectionOverride, options?.task)
      : null;

    if (scope === "selection" && !resolvedSelection) {
      toast.error("请先选中正文片段，或先从问题卡定位到对应片段。");
      return;
    }

    const request = buildAiRevisionRequest({
      source,
      scope,
      presetOperation: options?.presetOperation,
      instruction: options?.instruction,
      selection: resolvedSelection,
      content: contentDraft,
      provider: llm.provider,
      model: llm.model,
      temperature: llm.temperature,
    });
    previewMutation.mutate(request);
  };

  const handleRunOperation = (operation: ChapterEditorOperation, customInstruction?: string) => {
    runRevision(
      operation === "custom" ? "freeform" : "preset",
      "selection",
      {
        presetOperation: operation === "custom" ? undefined : operation,
        instruction: customInstruction,
        selectionOverride: selection,
      },
    );
  };

  const handleRunCursorOperation = (operation: ChapterEditorCursorOperation, instruction?: string) => {
    if (cursorOffset === null) {
      toast.error("请先把光标放在正文中，再选择续写方向。");
      return;
    }
    const normalizedContent = normalizeChapterContent(contentDraft);
    const cursorRange: ChapterEditorSelectionRange = {
      from: cursorOffset,
      to: cursorOffset,
      text: "",
    };
    const request: ChapterEditorCursorContinuationRequest = {
      operation,
      contentSnapshot: normalizedContent,
      cursorOffset,
      context: getParagraphWindow(normalizedContent, cursorRange),
      instruction: instruction?.trim() || undefined,
      provider: llm.provider,
      model: llm.model,
      temperature: llm.temperature,
    };
    continuationMutation.mutate(request);
  };

  const handleRegenerate = () => {
    // 上一次结果未知、可能已计费，或本章还有未确认的请求时，重新生成必须先确认；
    // 不允许在作者不知情的情况下再发一次可能产生费用的请求。
    if (requiresRetryConfirmation(session.billingOutcome ?? null, pendingRewriteCount > 0)) {
      const warning = rewriteRetryWarning(session.billingOutcome ?? "unknown")
        ?? rewriteRetryWarning("billed_failure");
      if (warning && !window.confirm(warning)) return;
    }
    if (session.mode === "continuation" && lastContinuationRequestRef.current) {
      continuationMutation.mutate(lastContinuationRequestRef.current);
      return;
    }
    if (!lastPreviewRequestRef.current) {
      return;
    }
    previewMutation.mutate(lastPreviewRequestRef.current);
  };

  const handleReject = () => {
    setSession(EMPTY_SESSION);
  };

  const handleLoadSnapshotContent = (content: string, snapshotId: string) => {
    draft.setContentDraft(content);
    setLoadedSnapshotId(snapshotId);
    setSaveStatus("idle");
    setSession(EMPTY_SESSION);
    setSelection(null);
    setSelectionToolbarPosition(null);
    setCursorOffset(null);
    setCursorToolbarPosition(null);
    setSelectedDiagnosticId(null);
  };

  const handleFocusDiagnostic = (card: ChapterEditorDiagnosticCard) => {
    if (selectedDiagnosticId === card.id) {
      setSelectedDiagnosticId(null);
      return;
    }
    setSelectedDiagnosticId(card.id);
    setSelection(null);
    setSelectionToolbarPosition(null);
  };

  const handleRunDiagnostic = (card: ChapterEditorDiagnosticCard) => {
    setSelectedDiagnosticId(card.id);
    runRevision("preset", card.recommendedScope, {
      presetOperation: card.recommendedAction,
      selectionOverride: toSelectionFromRange(contentDraft, card.anchorRange ?? null),
    });
  };

  const handleRunRecommended = () => {
    if (!workspace?.recommendedTask) {
      return;
    }
    runRevision("preset", workspace.recommendedTask.recommendedScope, {
      presetOperation: workspace.recommendedTask.recommendedAction,
      task: workspace.recommendedTask,
    });
  };

  const handleRunSelectedDiagnostic = () => {
    if (!selectedDiagnosticCard) {
      return;
    }
    handleRunDiagnostic(selectedDiagnosticCard);
  };

  const handleRunFreeform = () => {
    runRevision("freeform", revisionScope, {
      instruction: revisionInstruction.trim(),
    });
  };

  const currentTargetDescription = revisionScope === "chapter"
    ? "整章正文"
    : selection
      ? "你手动选中的正文片段"
      : selectedDiagnosticCard?.paragraphLabel
        ? `${selectedDiagnosticCard.paragraphLabel} 对应片段`
        : workspace?.recommendedTask?.paragraphLabel
          ? `${workspace.recommendedTask.paragraphLabel} 对应片段`
          : "尚未选中片段";
  const canRunSelectionRevision = Boolean(getSelectionTarget());
  const headerSaveLabel = getSaveStatusLabel(saveStatus, isDirty);
  const gridClassName = "xl:grid-cols-[320px_minmax(0,1fr)_400px]";
  // 页签偏好按书持久化：重启桌面版后仍停在上次的页签上。
  const storage = typeof window === "undefined" ? null : window.localStorage;
  const [rightTab, setRightTab] = useState<PanelTab>(() => readStoredPanelTab(novelId, storage));
  const [rightPanelOpen, setRightPanelOpen] = useState(true);
  // 编辑位置：从偏好缓存读取上次离开时的光标与滚动位置，文档恢复完成后再应用。
  const [restorePosition, setRestorePosition] = useState<{ cursorOffset: number; scrollTop: number } | null>(null);
  useEffect(() => {
    const current = chapter;
    if (!current?.id) {
      setRestorePosition(null);
      return;
    }
    const record = readEditPosition(novelId, typeof window === "undefined" ? null : window.localStorage);
    setRestorePosition(
      resolveEditPositionRestore({
        record,
        chapterId: current.id,
        officialVersion: current.officialVersion ?? 0,
        // 正文恢复完成后编辑器才会应用；这里用初始正文长度做第一道校验。
        contentLength: normalizeChapterContent(current.content ?? "").length,
      }),
    );
  }, [chapter, novelId]);
  // 位置上报只依赖章节身份；正文长度通过 ref 读取，避免每次输入都重建回调
  // 进而让编辑器里的位置监听反复解绑重绑。
  const contentLengthRef = useRef(0);
  contentLengthRef.current = normalizeChapterContent(contentDraft).length;
  const rememberPosition = useCallback((position: { cursorOffset: number; scrollTop: number }) => {
    if (!chapter?.id) return;
    writeRecentEditTarget({ novelId, chapterId: chapter.id, updatedAt: Date.now() });
    writeEditPosition(
      buildEditPositionRecord({
        novelId,
        chapterId: chapter.id,
        cursorOffset: position.cursorOffset,
        scrollTop: position.scrollTop,
        officialVersion: chapter.officialVersion ?? 0,
        contentLength: contentLengthRef.current,
      }),
    );
  }, [chapter?.id, chapter?.officialVersion, novelId]);
  const saveShortcutRef = useRef<() => void>(() => {});
  saveShortcutRef.current = () => {
    if (!chapter || !isDirty || saveMutation.isPending) return;
    saveMutation.mutate(contentDraft);
  };

  const changeRightTab = (tab: PanelTab) => {
    setRightTab(tab);
    storePanelTab(novelId, tab, storage);
    setRightPanelOpen(true);
  };

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (!isSaveShortcut(event)) return;
      event.preventDefault();
      saveShortcutRef.current();
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);

  return (
    <div data-local-draft-status={draft.localDraftStatus} data-local-draft-loaded={String(draft.localDraftLoaded)} data-local-draft-dirty={String(isDirty)} className="flex min-h-0 flex-1 flex-col gap-4">
      {qualityDebtDetails ? (
        <div className="flex shrink-0 flex-col gap-3 rounded-2xl border border-amber-200 bg-amber-50 px-4 py-3 text-amber-950 sm:flex-row sm:items-center sm:justify-between">
          <div className="flex min-w-0 items-start gap-3">
            <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0 text-amber-600" />
            <div className="min-w-0 text-sm leading-6">
              <div className="font-medium">本章正文可继续使用，但还有质量待优化项</div>
              <div className="text-xs text-amber-900/80">
                来源：{formatQualityDebtSource(qualityDebtDetails.source)} · {qualityDebtDetails.reason || "AI 审校发现了局部问题。"}
                {` · 自动修复：${formatQualityDebtAttempts(qualityDebtDetails)}`}
              </div>
              {isDirty ? <div className="mt-1 text-xs font-medium">请先保存正文，再让 AI 重新审校确认。</div> : null}
            </div>
          </div>
          <Button
            type="button"
            size="sm"
            variant="outline"
            className="shrink-0 border-amber-300 bg-white/70 hover:bg-white"
            disabled={isDirty || saveMutation.isPending || reviewMutation.isPending}
            onClick={() => reviewMutation.mutate()}
          >
            {reviewMutation.isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
            重新审校确认
          </Button>
        </div>
      ) : null}
      {(restored || hasExternalChange || storageError || draft.localDraftConflict || draft.localDraftStatus === "error") ? (
        <div data-testid="chapter-draft-banner" className="flex shrink-0 flex-wrap items-center justify-between gap-3 rounded-2xl border border-amber-200 bg-amber-50 px-4 py-3 text-sm text-amber-950">
          <span>{draft.localDraftStatus === "error" ? "本地草稿保存失败。请先保存正式正文，并检查磁盘空间。" : storageError ? "浏览器暂时无法保存本地草稿，请尽快保存到作品。" : draft.localDraftConflict ? "本地草稿基于较早的正式版本，已保留双方内容。请对比后继续。" : hasExternalChange ? "本章已有新的保存版本，你的草稿仍保留。" : "已恢复未保存的正文草稿。"}</span>
          {hasExternalChange ? <Button size="sm" variant="outline" onClick={draft.loadSavedVersion}>加载新保存版本</Button> : null}
          {draft.localDraftStatus === "error" ? <Button size="sm" variant="outline" onClick={() => void draft.flushDraft()}>重试保存草稿</Button> : null}
        </div>
      ) : null}
      {isDirty && draft.localDraftStatus === "saved" ? (
        <p data-testid="chapter-local-draft-saved" className="shrink-0 text-xs text-muted-foreground">本地草稿已安全保存；正式正文尚未保存。</p>
      ) : null}
      {pendingRewriteCount > 0 ? (
        <div
          data-testid="chapter-selection-rewrite-unknown"
          className="flex shrink-0 items-start gap-3 rounded-2xl bg-amber-50 px-4 py-3 text-sm leading-6 text-amber-950"
        >
          <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0 text-amber-600" />
          <span>
            上一次选区改写没有拿到可确认的结果，可能已经发送到模型服务并产生费用。
            重新生成会再次调用模型；如果只是想换一种改法，可以先确认模型服务状态，再决定是否重新生成。
          </span>
        </div>
      ) : null}
      <div className="flex shrink-0 items-center justify-end gap-2 xl:hidden">
        <button
          type="button"
          onClick={() => setRightPanelOpen((open) => !open)}
          data-testid="toggle-right-panel"
          className="rounded-lg bg-muted/60 px-3 py-1.5 text-xs text-muted-foreground"
        >
          {rightPanelOpen ? "收起 AI 助手" : "展开 AI 助手"}
        </button>
      </div>
      <div className={`grid min-h-0 flex-1 gap-4 overflow-hidden ${gridClassName}`}>
        <ChapterEditorSidebar
          chapter={chapter}
          chapters={chapters}
          volumes={volumes}
          onSelectChapter={onSelectChapter}
          runAttention={runAttention}
          onOpenRunTab={() => changeRightTab("run")}
          isBusy={isDraftWriteBusy}
          hasPendingCandidate={isCandidatePending}
          workspace={workspace}
          workspaceStatus={workspaceStatus}
          wordCount={wordCount}
          saveStatusLabel={headerSaveLabel}
          isDirty={isDirty}
          isSaving={saveMutation.isPending}
          selectedDiagnosticId={selectedDiagnosticId}
          onBack={onBack ? () => leaveEditor(onBack) : undefined}
          onOpenChapterVersions={onOpenVersionHistory ? undefined : () => setIsVersionDrawerOpen(true)}
          onOpenVersionHistory={onOpenVersionHistory ? () => leaveEditor(onOpenVersionHistory) : undefined}
          onRefreshWorkspace={onRefreshWorkspace}
          isRefreshingWorkspace={isRefreshingWorkspace}
          onSave={() => saveMutation.mutate(contentDraft)}
          onFocusDiagnostic={handleFocusDiagnostic}
          onRunDiagnostic={handleRunDiagnostic}
          onResolveDiagnostic={(card) => {
            if (card.sourceIssueId) {
              resolveDiagnosticMutation.mutate(card.sourceIssueId);
            }
          }}
          isResolvingDiagnostic={resolveDiagnosticMutation.isPending}
        />

        <div data-testid="chapter-editor-surface" className="relative min-h-0 overflow-hidden">
          <ChapterTextEditor
            value={contentDraft}
            readOnly={session.status !== "idle"}
            onChange={(next) => {
              draft.setContentDraft(next);
              setSaveStatus("idle");
            }}
            onSelectionChange={(nextSelection, position) => {
              setSelection(nextSelection);
              setSelectionToolbarPosition(position);
              if (nextSelection) {
                setCursorOffset(null);
                setCursorToolbarPosition(null);
              }
              if (nextSelection) {
                setSelectedDiagnosticId(null);
              }
            }}
            onCursorChange={(offset, position) => {
              setCursorOffset(offset);
              setCursorToolbarPosition(position);
              if (offset !== null) {
                setSelection(null);
                setSelectionToolbarPosition(null);
                setSelectedDiagnosticId(null);
              }
            }}
            restorePosition={restorePosition}
            onPositionChange={rememberPosition}
            preview={previewPayload}
            focusRange={session.status === "idle"
              ? selection
                ? { from: selection.from, to: selection.to }
                : selectedDiagnosticCard?.anchorRange ?? null
              : null}
          />
          <SelectionAIFloatingToolbar
            visible={Boolean(selection && session.status === "idle")}
            position={selectionToolbarPosition}
            disabled={previewMutation.isPending}
            onRunOperation={handleRunOperation}
          />
          <CursorAIFloatingToolbar
            visible={Boolean(cursorOffset !== null && !selection && session.status === "idle")}
            position={cursorToolbarPosition}
            disabled={continuationMutation.isPending || previewMutation.isPending}
            onRunOperation={handleRunCursorOperation}
          />
        </div>

        <div className={`min-h-0 overflow-hidden ${rightPanelOpen ? "" : "hidden"} xl:block`}>
          <ChapterRightPanel
            novelId={novelId}
            activeTab={rightTab}
            onTabChange={changeRightTab}
            chapterId={chapter?.id ?? ""}
            workspace={workspace}
            selectedDiagnosticId={selectedDiagnosticId}
            onOpenChapter={(sourceChapterId) => leaveEditor(() => onSelectChapter?.(sourceChapterId))}
            world={world}
            characters={characters}
            onFocusDiagnostic={handleFocusDiagnostic}
            onRunDiagnostic={handleRunDiagnostic}
            onResolveDiagnostic={(card) => {
              if (card.sourceIssueId) {
                resolveDiagnosticMutation.mutate(card.sourceIssueId);
              }
            }}
            isResolvingDiagnostic={resolveDiagnosticMutation.isPending}
          >
          <ChapterEditorDirectorPanel
            workspace={workspace}
            workspaceStatus={workspaceStatus}
            selectedDiagnosticCard={selectedDiagnosticCard}
            session={session}
            activeCandidate={activeCandidate}
            diffChanges={review.changes}
            selectedDiffChangeIds={review.selectedChangeIds}
            reviewError={review.error}
            hasSelectedChanges={review.resultText !== session.targetRange?.text}
            revisionScope={revisionScope}
            revisionInstruction={revisionInstruction}
            canRunSelectionRevision={canRunSelectionRevision}
            currentTargetDescription={currentTargetDescription}
            isGenerating={previewMutation.isPending}
            isApplying={acceptMutation.isPending}
            onInstructionChange={setRevisionInstruction}
            onScopeChange={setRevisionScope}
            onRunRecommended={handleRunRecommended}
            onRunSelectedDiagnostic={handleRunSelectedDiagnostic}
            onRunFreeform={handleRunFreeform}
            onSelectCandidate={(candidateId) => setSession((current) => ({ ...current, activeCandidateId: candidateId }))}
            onSelectAllChanges={review.selectAll}
            onToggleDiffChange={review.toggleChange}
            onChangeViewMode={(mode) => setSession((current) => ({ ...current, viewMode: mode }))}
            onAccept={() => acceptMutation.mutate()}
            onReject={handleReject}
            onRegenerate={handleRegenerate}
          />
          </ChapterRightPanel>
        </div>
      </div>
      <ChapterVersionDrawer
        novelId={novelId}
        chapterId={chapter.id}
        chapterTitle={chapter.title}
        open={isVersionDrawerOpen}
        onOpenChange={setIsVersionDrawerOpen}
        currentContent={contentDraft}
        isDirty={isDirty}
        isDraftWriteBusy={isDraftWriteBusy}
        hasPendingCandidate={isCandidatePending}
        onLoadContent={handleLoadSnapshotContent}
      />
    </div>
  );
}
