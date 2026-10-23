import { useEffect, useMemo, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useParams } from "react-router-dom";
import { Button } from "@/components/ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import {
  acceptQualityDebt,
  approveRun,
  cancelRun,
  clearCandidateEditDraft,
  confirmCandidateAfterEdit,
  getCandidateEditDraft,
  listCandidateEditDrafts,
  regenerateRun,
  saveCandidateEditDraft,
  rejectRun,
  getActiveRun,
  getModelSetup,
  getNovel,
  getRunDetail,
  listChapters,
  listRuns,
  openRun,
  replayRunEvents,
  resumeRun,
  subscribeRunWithCleanup,
  type CandidateEditDraftView,
  type GenerationEvent,
  type RunDetailView,
} from "@/platform/desktopTransport";

const STEPS = [
  ["story_macro", "故事宏观"],
  ["character_cast", "人物阵容"],
  ["volume_strategy", "卷策略"],
  ["chapter_list", "章节计划"],
  ["context", "上下文"],
  ["draft", "章节草稿"],
  ["consistency_review", "一致性审校"],
  ["bounded_repair", "局部修复"],
  ["approval", "人工确认"],
  ["commit", "正式保存"],
] as const;

function commandId(prefix: string): string {
  return `${prefix}:${Date.now()}:${crypto.randomUUID?.() ?? Math.random().toString(36).slice(2)}`;
}

function statusLabel(status: string): string {
  return ({ queued: "排队中", running: "生成中", waiting_approval: "等待确认", paused: "已暂停", succeeded: "已完成", failed: "失败", cancelled: "已取消" } as Record<string, string>)[status] ?? status;
}

function eventLabel(event: GenerationEvent): string {
  if (event.kind === "token_delta") return "正在生成内容";
  return ({ run_started: "创作开始", step_started: "阶段开始", checkpoint_saved: "候选正文已保存", needs_approval: "等待人工确认", step_succeeded: "阶段完成", run_paused: "创作已暂停", run_failed: "创作失败", run_cancelled: "已取消", run_completed: "创作完成" } as Record<string, string>)[event.kind] ?? event.kind;
}

function artifactLabel(type: string): string {
  return ({ story_macro: "故事宏观", character_cast: "人物阵容", volume_strategy: "卷策略", chapter_list: "章节计划", context: "上下文", consistency_review: "一致性审校", chapter_approval: "章节确认", chapter_commit: "正式提交" } as Record<string, string>)[type] ?? type;
}

function pretty(value: string): string {
  try { return JSON.stringify(JSON.parse(value), null, 2); } catch { return value; }
}

function candidateDiff(original: string, edited: string): Array<{ kind: "same" | "added" | "removed"; text: string }> {
  const before = original.split("\n");
  const after = edited.split("\n");
  const out: Array<{ kind: "same" | "added" | "removed"; text: string }> = [];
  const max = Math.max(before.length, after.length);
  for (let index = 0; index < max; index += 1) {
    if (before[index] === after[index]) {
      if (before[index] !== undefined) out.push({ kind: "same", text: before[index] });
    } else {
      if (before[index] !== undefined) out.push({ kind: "removed", text: before[index] });
      if (after[index] !== undefined) out.push({ kind: "added", text: after[index] });
    }
  }
  return out;
}

export default function TauriDirectorPage() {
  const { novelId = "" } = useParams();
  const queryClient = useQueryClient();
  const [chapterId, setChapterId] = useState("");
  const [selectedRunId, setSelectedRunId] = useState("");
  const [liveText, setLiveText] = useState("");
  const [events, setEvents] = useState<GenerationEvent[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [editingCandidate, setEditingCandidate] = useState(false);
  const [candidateEdit, setCandidateEdit] = useState("");
  const [candidateDraftStatus, setCandidateDraftStatus] = useState<"idle" | "saving" | "saved" | "error" | "restored">("idle");
  const [candidateDraftChoices, setCandidateDraftChoices] = useState<CandidateEditDraftView[]>([]);
  const candidateClientId = useRef<string>("");
  const candidateDraftRevision = useRef(0);
  if (!candidateClientId.current) {
    // sessionStorage is per window and survives refresh; a new app process gets a
    // new writer identity. Prior drafts are discovered through the run journal.
    try {
      candidateClientId.current = sessionStorage.getItem("ai-novel:candidate-edit-client:v1")
        ?? (() => { const id = crypto.randomUUID?.() ?? `candidate-${Date.now()}`; sessionStorage.setItem("ai-novel:candidate-edit-client:v1", id); return id; })();
    } catch { candidateClientId.current = `candidate-${Date.now()}`; }
  }
  const mounted = useRef(true);

  useEffect(() => () => { mounted.current = false; }, []);

  const novelQuery = useQuery({ queryKey: ["tauri-director", novelId, "novel"], queryFn: () => getNovel(novelId), enabled: !!novelId });
  const chaptersQuery = useQuery({ queryKey: ["tauri-director", novelId, "chapters"], queryFn: () => listChapters(novelId), enabled: !!novelId });
  const modelQuery = useQuery({ queryKey: ["tauri-director", "model"], queryFn: getModelSetup });
  const activeQuery = useQuery({ queryKey: ["tauri-director", novelId, "active"], queryFn: () => getActiveRun(novelId), enabled: !!novelId, refetchInterval: 3000 });
  const historyQuery = useQuery({ queryKey: ["tauri-director", novelId, "history"], queryFn: () => listRuns(novelId), enabled: !!novelId, refetchInterval: 5000 });
  const runId = activeQuery.data?.id ?? selectedRunId;
  const detailQuery = useQuery<RunDetailView>({ queryKey: ["tauri-director", novelId, "detail", runId], queryFn: () => getRunDetail(novelId, runId), enabled: !!runId, refetchInterval: 3000 });
  const detail = detailQuery.data;
  const chapters = chaptersQuery.data ?? [];
  const latestCandidate = detail?.latestCandidate ?? null;
  const chapterApprovalGate = detail?.gate?.artifactTypes.includes("chapter_approval") ?? false;
  useEffect(() => {
    if (!editingCandidate || !latestCandidate || !runId || !detail?.gate?.id) return;
    let cancelled = false;
    void (async () => {
      const own = await getCandidateEditDraft({ runId, gateId: detail.gate!.id, clientId: candidateClientId.current });
      if (cancelled) return;
      if (own) {
        candidateDraftRevision.current = own.localRevision;
        setCandidateEdit(own.content);
        setCandidateDraftStatus("restored");
        setCandidateDraftChoices([]);
        return;
      }
      const all = await listCandidateEditDrafts({ novelId, runId });
      if (cancelled) return;
      const recoverable = all.filter(draft => draft.clientId !== candidateClientId.current && (draft.content !== latestCandidate.content || draft.baseCandidateId !== latestCandidate.id));
      setCandidateDraftChoices(recoverable);
      candidateDraftRevision.current = 0;
      setCandidateEdit(latestCandidate.content);
      setCandidateDraftStatus(recoverable.length ? "idle" : "idle");
    })().catch(() => { if (!cancelled) setCandidateDraftStatus("error"); });
    return () => { cancelled = true; };
  }, [detail?.gate?.id, editingCandidate, latestCandidate?.id, runId]);
  useEffect(() => {
    if (!editingCandidate || !latestCandidate || !runId || !detail?.gate?.id || !novelId) return;
    if (candidateEdit === latestCandidate.content) return;
    const timer = setTimeout(() => {
      setCandidateDraftStatus("saving");
      void saveCandidateEditDraft({
        novelId, chapterId: detail.chapterId, runId, gateId: detail.gate!.id,
        clientId: candidateClientId.current, baseCandidateId: latestCandidate.id,
        baseContentSha256: latestCandidate.contentSha256, content: candidateEdit,
        controlVersion: detail.controlVersion, expectedRevision: candidateDraftRevision.current,
      }).then((saved) => {
        candidateDraftRevision.current = saved.localRevision;
        setCandidateDraftStatus("saved");
      }).catch(() => setCandidateDraftStatus("error"));
    }, 500);
    return () => clearTimeout(timer);
  }, [candidateEdit, detail?.chapterId, detail?.controlVersion, detail?.gate?.id, editingCandidate, latestCandidate, novelId, runId]);
  const restoreCandidateDraft = (draft: CandidateEditDraftView) => {
    if (!latestCandidate || draft.gateId !== detail?.gate?.id || draft.baseCandidateId !== latestCandidate.id || draft.baseContentSha256 !== latestCandidate.contentSha256) {
      setError("这份候选草稿对应的 Gate 或候选版本已变化，只能查看，不能直接确认。");
      return;
    }
    candidateDraftRevision.current = 0;
    setCandidateEdit(draft.content);
    setCandidateDraftStatus("restored");
    setCandidateDraftChoices([]);
    setError(null);
  };

  const candidateDiffLines = useMemo(
    () => latestCandidate ? candidateDiff(latestCandidate.content, candidateEdit) : [],
    [latestCandidate, candidateEdit],
  );
  const model = modelQuery.data;
  const currentChapter = chapters.find(chapter => chapter.id === chapterId) ?? chapters[0];

  useEffect(() => {
    if (!chapterId && chapters[0]) setChapterId(chapters[0].id);
  }, [chapterId, chapters]);

  useEffect(() => {
    if (activeQuery.data?.id) {
      setSelectedRunId(activeQuery.data.id);
      return;
    }
    if (!selectedRunId && historyQuery.data?.[0]) {
      setSelectedRunId(historyQuery.data[0].id);
    }
  }, [activeQuery.data?.id, historyQuery.data, selectedRunId]);

  useEffect(() => {
    if (!runId) return;
    let disposed = false;
    const seen = new Set<string>();
    const onEvent = (event: GenerationEvent) => {
      if (disposed || !mounted.current) return;
      if (event.kind === "token_delta") {
        setLiveText(value => `${value}${event.text}`);
        return;
      }
      const key = `${event.kind}:${event.seq}`;
      if (seen.has(key)) return;
      seen.add(key);
      setEvents(previous => [...previous, event].slice(-200));
      if (["needs_approval", "run_completed", "run_failed", "run_cancelled", "run_paused", "step_succeeded"].includes(event.kind)) {
        void queryClient.invalidateQueries({ queryKey: ["tauri-director", novelId] });
      }
    };
    setLiveText("");
    setEvents([]);
    const subscription = subscribeRunWithCleanup({ runId }, onEvent);
    void subscription.ready
      .then(async () => {
        const missed = await replayRunEvents({ runId, afterSeq: 0 });
        missed.forEach(onEvent);
      })
      .catch((cause: unknown) => {
        if (!disposed) setError(cause instanceof Error ? cause.message : "实时订阅中断，正在通过历史记录恢复。");
      });
    return () => {
      disposed = true;
      subscription.close();
    };
  }, [novelId, queryClient, runId]);

  const start = useMutation({
    mutationFn: async () => {
      if (!currentChapter || !model) throw new Error("请先选择章节，并在桌面设置中配置模型。");
      if (!model.hasApiKey) throw new Error("当前模型没有可用 API Key，请先完成模型设置。");
      return openRun({
        commandId: commandId("director-open"),
        novelId,
        targetChapterId: currentChapter.id,
        provider: "openai-compatible",
        model: model.model,
        promptVersions: { chapter_draft: "chapter-draft-v2" },
        tokenBudget: null,
        maxAutomaticRetries: 0,
        endpoint: model.endpoint,
        allowLocalhostHttp: model.allowLocalhostHttp,
      });
    },
    onSuccess: async () => {
      setError(null);
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["tauri-director", novelId] }),
        queryClient.invalidateQueries({ queryKey: ["tauri-director", novelId, "history"] }),
      ]);
    },
    onError: (cause: unknown) => setError(cause instanceof Error ? cause.message : "创建创作任务失败。"),
  });

  const approve = useMutation({
    mutationFn: () => approveRun({ runId, commandId: commandId("director-approve"), expectedVersion: detail?.controlVersion }),
    onSuccess: async () => queryClient.invalidateQueries({ queryKey: ["tauri-director", novelId] }),
    onError: (cause: unknown) => setError(cause instanceof Error ? cause.message : "确认失败，请刷新后重试。"),
  });
  const reject = useMutation({
    mutationFn: () => rejectRun({ runId, commandId: commandId("director-reject"), expectedVersion: detail?.controlVersion }),
    onSuccess: async () => queryClient.invalidateQueries({ queryKey: ["tauri-director", novelId] }),
    onError: (cause: unknown) => setError(cause instanceof Error ? cause.message : "拒绝失败，请刷新后重试。"),
  });
  const regenerate = useMutation({
    mutationFn: () => regenerateRun({ runId, commandId: commandId("director-regenerate"), expectedVersion: detail?.controlVersion }),
    onSuccess: async () => queryClient.invalidateQueries({ queryKey: ["tauri-director", novelId] }),
    onError: (cause: unknown) => setError(cause instanceof Error ? cause.message : "重新生成失败，请刷新后重试。"),
  });
  const confirmEdit = useMutation({
    mutationFn: () => {
      if (!latestCandidate) throw new Error("没有可确认的候选正文。");
      return confirmCandidateAfterEdit({
        runId,
        commandId: commandId("director-confirm-edit"),
        expectedVersion: detail?.controlVersion,
        baseCandidateId: latestCandidate.id,
        baseContentSha256: latestCandidate.contentSha256,
        content: candidateEdit,
      });
    },
    onSuccess: async () => {
      if (runId && detail?.gate?.id) {
        void clearCandidateEditDraft({
          runId, gateId: detail.gate.id, clientId: candidateClientId.current,
          content: candidateEdit, localRevision: candidateDraftRevision.current,
        }).catch(() => undefined);
      }
      setEditingCandidate(false);
      setCandidateDraftStatus("idle");
      await queryClient.invalidateQueries({ queryKey: ["tauri-director", novelId] });
    },
    onError: (cause: unknown) => setError(cause instanceof Error ? cause.message : "确认修改后候选失败，请刷新后重试。"),
  });
  const acceptDebt = useMutation({
    mutationFn: ({ id, version }: { id: string; version: number }) =>
      acceptQualityDebt({ runId, debtId: id, expectedVersion: version, reason: "作者确认保留该质量问题，暂不自动修复。" }),
    onSuccess: async () => queryClient.invalidateQueries({ queryKey: ["tauri-director", novelId] }),
    onError: (cause: unknown) => setError(cause instanceof Error ? cause.message : "接受质量问题失败，请刷新后重试。"),
  });
  const cancel = useMutation({
    mutationFn: () => cancelRun({ runId, commandId: commandId("director-cancel") }),
    onSuccess: async () => queryClient.invalidateQueries({ queryKey: ["tauri-director", novelId] }),
    onError: (cause: unknown) => setError(cause instanceof Error ? cause.message : "取消失败。"),
  });
  const resume = useMutation({
    mutationFn: () => resumeRun({ runId, commandId: commandId("director-resume"), acceptCandidate: true }),
    onSuccess: async () => queryClient.invalidateQueries({ queryKey: ["tauri-director", novelId] }),
    onError: (cause: unknown) => setError(cause instanceof Error ? cause.message : "恢复失败。"),
  });

  const gate = detail?.gate;
  const draftArtifacts = useMemo(() => (detail?.artifacts ?? []).filter(artifact => artifact.status === "draft"), [detail?.artifacts]);
  const completed = detail?.status === "succeeded";

  return (
    <div data-tauri-director-page className="mx-auto flex h-full min-h-0 w-full max-w-[1500px] flex-col gap-4 p-4">
      <header className="flex flex-wrap items-center justify-between gap-3 border-b border-border/60 pb-3">
        <div>
          <h1 className="text-lg font-semibold">{novelQuery.data?.title ?? "自动导演"}</h1>
          <p className="mt-1 text-xs text-muted-foreground">当前仅支持单章节创作流程。多章节批量、世界模式和自定义策略暂未支持。</p>
        </div>
        <div className="flex gap-2">
          <Button asChild variant="ghost"><Link to="/novels">返回书架</Link></Button>
          <Button asChild variant="ghost"><Link to="/lab/director">运行记录</Link></Button>
        </div>
      </header>

      {error ? <p role="alert" className="rounded-md bg-destructive/5 p-3 text-sm text-destructive">{error}</p> : null}

      {!runId ? (
        <section data-tauri-director-start className="max-w-xl space-y-4 rounded-md bg-muted/20 p-4">
          <h2 className="text-sm font-semibold">开始一次创作</h2>
          <label className="block space-y-2 text-xs">选择章节
            <Select value={currentChapter?.id} onValueChange={setChapterId}>
              <SelectTrigger><SelectValue placeholder="选择要创作的章节" /></SelectTrigger>
              <SelectContent>{chapters.map(chapter => <SelectItem key={chapter.id} value={chapter.id}>{chapter.sortOrder}. {chapter.title}</SelectItem>)}</SelectContent>
            </Select>
          </label>
          <div className="text-xs leading-5 text-muted-foreground">
            {model ? <>模型：{model.model} · {model.endpoint}{model.hasApiKey ? "" : " · 缺少 API Key"}</> : "尚未配置模型。请先在桌面模型设置中填写服务地址、模型和 API Key。"}
          </div>
          <Button data-testid="director-start-button" className="w-full" disabled={!currentChapter || !model?.hasApiKey || start.isPending} onClick={() => start.mutate()}>
            {start.isPending ? "正在创建…" : "提交并开始创作"}
          </Button>
        </section>
      ) : (
        <div className="grid min-h-0 flex-1 gap-4 lg:grid-cols-[minmax(0,1fr)_380px]">
          <section className="min-h-0 space-y-4 overflow-y-auto">
            <div className="rounded-md bg-muted/20 p-4">
              <div className="flex flex-wrap items-center justify-between gap-2">
                <h2 data-testid="director-status" className="text-sm font-semibold">运行状态：{statusLabel(detail?.status ?? activeQuery.data?.state ?? "queued")}</h2>
                <span data-testid="director-run-meta" className="text-xs text-muted-foreground">Run {detail?.id ?? runId} · 状态版本 {detail?.controlVersion ?? "—"}</span>
              </div>
              <p className="mt-2 text-xs text-muted-foreground">章节：{detail?.chapterTitle ?? activeQuery.data?.chapterTitle ?? "—"} · 当前阶段：{detail?.cursorStepId ? (STEPS.find(step => step[0] === detail.cursorStepId)?.[1] ?? detail.cursorStepId) : "等待调度"}</p>
              {detail?.pauseReason ? <p className="mt-2 text-sm text-destructive">暂停原因：{detail.pauseReason}</p> : null}
              {detail?.failureReason ? <p className="mt-2 text-sm text-destructive">错误：{detail.failureReason}</p> : null}
              <div className="mt-3 flex flex-wrap gap-2 text-xs">
                <span className="rounded-full bg-background px-2 py-1">{detail?.usage.calls ?? 0} 次模型调用</span>
                <span className="rounded-full bg-background px-2 py-1">{detail?.usage.promptTokens ?? 0} 输入 tokens</span>
                <span className="rounded-full bg-background px-2 py-1">{detail?.usage.completionTokens ?? 0} 输出 tokens</span>
              </div>
              <div className="mt-4 flex flex-wrap gap-2">
                {detail?.legalActions?.includes("confirm") && gate ? <Button data-testid="director-approve-button" disabled={approve.isPending} onClick={() => approve.mutate()}>{approve.isPending ? "确认中…" : `确认「${artifactLabel(gate.artifactTypes[0] ?? "")}」并继续`}</Button> : null}
                {detail?.legalActions?.includes("reject") ? <Button data-testid="director-reject-button" variant="outline" disabled={reject.isPending} onClick={() => reject.mutate()}>{reject.isPending ? "拒绝中…" : "拒绝这个候选"}</Button> : null}
                {detail?.legalActions?.includes("regenerate") ? <Button data-testid="director-regenerate-button" variant="outline" disabled={regenerate.isPending} onClick={() => regenerate.mutate()}>{regenerate.isPending ? "重新生成中…" : "重新生成候选"}</Button> : null}
                {chapterApprovalGate && latestCandidate ? <Button data-testid="director-edit-candidate-button" variant="outline" onClick={() => setEditingCandidate(true)}>{editingCandidate ? "正在编辑候选" : "编辑候选正文"}</Button> : null}
                {detail?.legalActions?.includes("resume") && detail.latestCandidate ? <Button data-testid="director-resume-button" variant="outline" disabled={resume.isPending} onClick={() => resume.mutate()}>{resume.isPending ? "恢复中…" : "接受候选并恢复"}</Button> : null}
                {detail?.legalActions?.includes("cancel") ? <Button data-testid="director-cancel-button" variant="ghost" disabled={cancel.isPending} onClick={() => cancel.mutate()}>{cancel.isPending ? "取消中…" : "取消创作"}</Button> : null}
                {completed ? <Button asChild><Link to={`/novels/${encodeURIComponent(novelId)}/edit`}>查看正式正文</Link></Button> : null}
                {completed ? <Button variant="ghost" onClick={() => { setSelectedRunId(""); setChapterId(""); }}>再创建一次</Button> : null}
              </div>
            </div>

            <div className="rounded-md bg-muted/20 p-4">
              <h2 className="text-sm font-semibold">阶段</h2>
              <ol className="mt-3 grid gap-2 text-xs sm:grid-cols-2">{STEPS.map(([id, label]) => {
                const done = detail?.artifacts.some(artifact => artifact.artifactType === id && ["confirmed", "draft"].includes(artifact.status));
                const current = detail?.cursorStepId === id;
                return <li key={id} className={`flex items-center gap-2 ${done ? "text-foreground" : "text-muted-foreground"}`}><span className={`h-1.5 w-1.5 rounded-full ${done ? "bg-primary" : "bg-muted-foreground/30"}`} />{label}{current ? <span className="text-primary">当前</span> : null}</li>;
              })}</ol>
            </div>

            {detail?.qualityDebt?.length ? <section data-testid="director-quality-debt" className="space-y-2 rounded-md bg-muted/20 p-4">
              <h2 className="text-sm font-semibold">质量问题</h2>
              <ul className="space-y-2 text-xs">
                {detail.qualityDebt.map(item => <li key={String(item.id)} className="flex flex-wrap items-center justify-between gap-2 rounded-md bg-background/60 px-3 py-2">
                  <span>{String(item.issueType)} · {String(item.severity)} · {String(item.description)}</span>
                  <span className="flex items-center gap-2 text-muted-foreground">{String(item.status)}
                    {String(item.status) === "open" ? <Button data-testid="director-accept-quality-debt" variant="outline" onClick={() => acceptDebt.mutate({ id: String(item.id), version: Number(item.decisionVersion ?? 0) })} disabled={acceptDebt.isPending}>接受并保留</Button> : null}
                  </span>
                  <details className="basis-full text-xs text-muted-foreground">
                    <summary className="cursor-pointer">查看来源、证据与处理记录</summary>
                    <p className="mt-2">审校轮次：v{String(item.reviewVersion)} · 来源：{String(item.sourceRef ?? "未标注")}</p>
                    <p className="mt-1">证据：{String(item.evidence)}</p>
                    <p className="mt-1">建议：{String(item.suggestion)}</p>
                    {item.resolutionEvidence ? <p className="mt-1">处理记录：{String(item.resolutionEvidence)}</p> : null}
                    {item.acceptedReason ? <p className="mt-1">接受原因：{String(item.acceptedReason)}</p> : null}
                  </details>
                </li>)}
              </ul>
            </section> : null}
            {gate && draftArtifacts.length > 0 ? <section className="space-y-3 rounded-md bg-muted/20 p-4">
              <h2 className="text-sm font-semibold">待确认内容</h2>
              <p className="text-xs text-muted-foreground">请核对本阶段结构化结果后确认。确认会推进真实 Rust Run；取消不会提交正文。</p>
              {draftArtifacts.map(artifact => <details key={artifact.id} className="rounded-md bg-background/60 p-3" open={artifact.artifactType === gate.artifactTypes[0]}>
                <summary className="cursor-pointer text-xs font-medium">{artifactLabel(artifact.artifactType)} · v{artifact.version} · {artifact.status}</summary>
                <pre className="mt-2 max-h-72 overflow-auto whitespace-pre-wrap break-words text-xs text-muted-foreground">{pretty(artifact.payloadJson)}</pre>
              </details>)}
            </section> : null}

            {editingCandidate && latestCandidate ? <section data-testid="director-candidate-editor" className="space-y-3 rounded-md bg-muted/20 p-4">
              <div className="flex flex-wrap items-center justify-between gap-2">
                <h2 className="text-sm font-semibold">编辑候选正文</h2>
                <span data-testid="director-candidate-draft-status" className="text-xs text-muted-foreground">{candidateDraftStatus === "saved" ? "候选编辑已本地保存" : candidateDraftStatus === "restored" ? "已恢复未确认的候选编辑" : candidateDraftStatus === "error" ? "候选编辑保存失败，请重试" : candidateDraftStatus === "saving" ? "正在保存候选编辑…" : "只修改候选，不会直接覆盖正式章节"}</span>
              </div>
              {candidateDraftChoices.length ? <div data-testid="director-candidate-draft-recovery" className="space-y-2 rounded-md bg-background/60 p-3">
                <p className="text-xs text-muted-foreground">发现尚未确认的候选编辑。选择一份恢复后会作为当前窗口的草稿继续保存，其他草稿不会被删除。</p>
                {candidateDraftChoices.map((draft) => {
                  const matchesGate = draft.gateId === detail?.gate?.id && draft.baseCandidateId === latestCandidate.id && draft.baseContentSha256 === latestCandidate.contentSha256;
                  return <div key={`${draft.gateId}:${draft.clientId}`} className="flex flex-wrap items-center justify-between gap-2 text-xs">
                    <span className="text-muted-foreground">窗口 {draft.clientId.slice(-6)} · 修订 {draft.localRevision} · {new Date(draft.updatedAt).toLocaleTimeString()}{matchesGate ? "" : " · 版本已变化，只读"}</span>
                    <Button data-testid="director-candidate-draft-choice" variant="outline" onClick={() => restoreCandidateDraft(draft)} disabled={!matchesGate}>{matchesGate ? "恢复此草稿" : "旧版本草稿已保留，不可确认"}</Button>
                  </div>;
                })}
              </div> : null}
              <textarea
                data-testid="director-candidate-edit-input"
                className="min-h-56 w-full rounded-md border border-input bg-background p-3 text-sm leading-6"
                value={candidateEdit}
                onChange={(event) => setCandidateEdit(event.target.value)}
              />
              <div className="max-h-56 overflow-auto rounded-md bg-background/60 p-3 text-xs">
                {candidateDiffLines.map((line, index) => <div key={`${line.kind}-${index}`} className={line.kind === "added" ? "text-primary" : line.kind === "removed" ? "text-destructive line-through" : "text-muted-foreground"}>{line.kind === "added" ? "+ " : line.kind === "removed" ? "- " : "  "}{line.text}</div>)}
              </div>
              <div className="flex flex-wrap gap-2">
                <Button data-testid="director-confirm-candidate-edit" disabled={confirmEdit.isPending || !candidateEdit.trim() || candidateEdit === latestCandidate.content} onClick={() => confirmEdit.mutate()}>{confirmEdit.isPending ? "确认中…" : "确认修改后候选"}</Button>
                <Button data-testid="director-cancel-candidate-edit" variant="outline" onClick={() => { setCandidateEdit(latestCandidate.content); setEditingCandidate(false); }}>恢复原候选</Button>
              </div>
            </section> : null}
            <section className="rounded-md bg-muted/20 p-4">
              <h2 className="text-sm font-semibold">候选正文</h2>
              <pre className="mt-2 max-h-80 overflow-auto whitespace-pre-wrap break-words text-sm leading-6">{liveText || detail?.latestCandidate?.content || "尚未产生候选正文。"}</pre>
            </section>
          </section>

          <aside className="space-y-4">
            <section className="rounded-md bg-muted/20 p-4">
              <h2 className="text-sm font-semibold">最近事件</h2>
              <ol className="mt-3 space-y-2 text-xs">{events.length ? events.slice(-12).reverse().map(event => <li key={`${event.kind}-${event.kind === "token_delta" ? Math.random() : event.seq}`} data-testid="director-event" className="flex justify-between gap-2"><span>{eventLabel(event)}</span><span className="text-muted-foreground">{event.kind === "token_delta" ? "流式" : `#${event.seq}`}</span></li>) : <li className="text-muted-foreground">等待首个事件。</li>}</ol>
            </section>
            <section className="rounded-md bg-muted/20 p-4">
              <h2 className="text-sm font-semibold">运行历史</h2>
              <ol className="mt-3 space-y-2 text-xs">{(historyQuery.data ?? []).slice(0, 8).map(run => <li key={run.id} className="flex items-center justify-between gap-2"><span className={run.id === runId ? "font-medium text-foreground" : "text-muted-foreground"}>{statusLabel(run.state)} · {run.chapterTitle || "未命名章节"}</span><span className="text-muted-foreground">{new Date(run.updatedAt).toLocaleDateString()}</span></li>)}</ol>
            </section>
            <section className="rounded-md bg-muted/20 p-4 text-xs text-muted-foreground">
              <h2 className="text-sm font-semibold text-foreground">暂未支持</h2>
              <p className="mt-2 leading-5">多章节批量、世界模式、自定义 driver、handoff 和修改后确认的编辑界面仍待接入；不会把未接入能力显示为可执行按钮。</p>
            </section>
          </aside>
        </div>
      )}
    </div>
  );
}
