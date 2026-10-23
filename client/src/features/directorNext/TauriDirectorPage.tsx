import { useEffect, useMemo, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useParams } from "react-router-dom";
import { Button } from "@/components/ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import {
  approveRun,
  cancelRun,
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

export default function TauriDirectorPage() {
  const { novelId = "" } = useParams();
  const queryClient = useQueryClient();
  const [chapterId, setChapterId] = useState("");
  const [selectedRunId, setSelectedRunId] = useState("");
  const [liveText, setLiveText] = useState("");
  const [events, setEvents] = useState<GenerationEvent[]>([]);
  const [error, setError] = useState<string | null>(null);
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
  const model = modelQuery.data;
  const currentChapter = chapters.find(chapter => chapter.id === chapterId) ?? chapters[0];

  useEffect(() => {
    if (!chapterId && chapters[0]) setChapterId(chapters[0].id);
  }, [chapterId, chapters]);

  useEffect(() => {
    if (activeQuery.data?.id) setSelectedRunId(activeQuery.data.id);
  }, [activeQuery.data?.id]);

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
        promptVersions: { chapter_draft: "chapter-draft-v1" },
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
          <Button className="w-full" disabled={!currentChapter || !model?.hasApiKey || start.isPending} onClick={() => start.mutate()}>
            {start.isPending ? "正在创建…" : "提交并开始创作"}
          </Button>
        </section>
      ) : (
        <div className="grid min-h-0 flex-1 gap-4 lg:grid-cols-[minmax(0,1fr)_380px]">
          <section className="min-h-0 space-y-4 overflow-y-auto">
            <div className="rounded-md bg-muted/20 p-4">
              <div className="flex flex-wrap items-center justify-between gap-2">
                <h2 className="text-sm font-semibold">运行状态：{statusLabel(detail?.status ?? activeQuery.data?.state ?? "queued")}</h2>
                <span className="text-xs text-muted-foreground">状态版本 {detail?.controlVersion ?? "—"}</span>
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
                {detail?.status === "waiting_approval" && gate ? <Button disabled={approve.isPending} onClick={() => approve.mutate()}>{approve.isPending ? "确认中…" : `确认「${artifactLabel(gate.artifactTypes[0] ?? "")}」并继续`}</Button> : null}
                {detail?.status === "paused" && detail.latestCandidate && detail.pauseKind !== "safety" ? <Button variant="outline" disabled={resume.isPending} onClick={() => resume.mutate()}>{resume.isPending ? "恢复中…" : "接受候选并恢复"}</Button> : null}
                {detail && !["succeeded", "failed", "cancelled"].includes(detail.status) ? <Button variant="ghost" disabled={cancel.isPending} onClick={() => cancel.mutate()}>{cancel.isPending ? "取消中…" : "取消创作"}</Button> : null}
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

            {gate && draftArtifacts.length > 0 ? <section className="space-y-3 rounded-md bg-muted/20 p-4">
              <h2 className="text-sm font-semibold">待确认内容</h2>
              <p className="text-xs text-muted-foreground">请核对本阶段结构化结果后确认。确认会推进真实 Rust Run；取消不会提交正文。</p>
              {draftArtifacts.map(artifact => <details key={artifact.id} className="rounded-md bg-background/60 p-3" open={artifact.artifactType === gate.artifactTypes[0]}>
                <summary className="cursor-pointer text-xs font-medium">{artifactLabel(artifact.artifactType)} · v{artifact.version} · {artifact.status}</summary>
                <pre className="mt-2 max-h-72 overflow-auto whitespace-pre-wrap break-words text-xs text-muted-foreground">{pretty(artifact.payloadJson)}</pre>
              </details>)}
            </section> : null}

            <section className="rounded-md bg-muted/20 p-4">
              <h2 className="text-sm font-semibold">候选正文</h2>
              <pre className="mt-2 max-h-80 overflow-auto whitespace-pre-wrap break-words text-sm leading-6">{liveText || detail?.latestCandidate?.content || "尚未产生候选正文。"}</pre>
            </section>
          </section>

          <aside className="space-y-4">
            <section className="rounded-md bg-muted/20 p-4">
              <h2 className="text-sm font-semibold">最近事件</h2>
              <ol className="mt-3 space-y-2 text-xs">{events.length ? events.slice(-12).reverse().map(event => <li key={`${event.kind}-${event.kind === "token_delta" ? Math.random() : event.seq}`} className="flex justify-between gap-2"><span>{eventLabel(event)}</span><span className="text-muted-foreground">{event.kind === "token_delta" ? "流式" : `#${event.seq}`}</span></li>) : <li className="text-muted-foreground">等待首个事件。</li>}</ol>
            </section>
            <section className="rounded-md bg-muted/20 p-4">
              <h2 className="text-sm font-semibold">运行历史</h2>
              <ol className="mt-3 space-y-2 text-xs">{(historyQuery.data ?? []).slice(0, 8).map(run => <li key={run.id} className="flex items-center justify-between gap-2"><span className={run.id === runId ? "font-medium text-foreground" : "text-muted-foreground"}>{statusLabel(run.state)} · {run.chapterTitle || "未命名章节"}</span><span className="text-muted-foreground">{new Date(run.updatedAt).toLocaleDateString()}</span></li>)}</ol>
            </section>
            <section className="rounded-md bg-muted/20 p-4 text-xs text-muted-foreground">
              <h2 className="text-sm font-semibold text-foreground">暂未支持</h2>
              <p className="mt-2 leading-5">拒绝/重新生成、多章节批量、世界模式、自定义 driver、handoff 和确认后编辑暂未接入 Rust 后端；这些操作不会显示为可执行按钮。</p>
            </section>
          </aside>
        </div>
      )}
    </div>
  );
}
