import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useMemo, useState } from "react";
import { queryKeys } from "@/api/queryKeys";
import { Button } from "@/components/ui/button";
import { toast } from "@/components/ui/toast";
import {
  createPlotCausality,
  createPlotThread,
  getPlotWorkspace,
  transitionPlotThread,
  type PlotThreadView,
  type TimelineEntryView,
} from "@/platform/desktopTransport";

interface PlotWorkspaceTabProps {
  novelId: string;
  chapterId: string;
  onOpenChapter?: (chapterId: string) => void;
}

const STATUS_LABELS: Record<string, string> = {
  open: "埋设",
  developing: "推进中",
  resolved: "已回收",
  deferred: "已暂停",
  abandoned: "已废弃",
  stale: "来源失效",
  superseded: "已失效",
  disputed: "待确认",
  confirmed: "已确认",
};

const TIMELINE_LABELS: Record<string, string> = {
  timeline_event: "时间线事件",
  character_state: "人物状态",
  plot_setup: "伏笔埋设",
  plot_develop: "伏笔推进",
  plot_payoff: "伏笔回收",
  plot_defer: "伏笔暂停",
  plot_abandon: "伏笔废弃",
  plot_recover: "伏笔恢复",
};

const ACTIONS_BY_STATUS: Record<string, Array<{ value: string; label: string }>> = {
  open: [
    { value: "develop", label: "推进" },
    { value: "payoff", label: "回收" },
    { value: "defer", label: "暂停" },
    { value: "abandon", label: "废弃" },
  ],
  developing: [
    { value: "develop", label: "继续推进" },
    { value: "payoff", label: "回收" },
    { value: "defer", label: "暂停" },
    { value: "abandon", label: "废弃" },
  ],
  deferred: [
    { value: "develop", label: "恢复推进" },
    { value: "payoff", label: "回收" },
    { value: "abandon", label: "废弃" },
  ],
  stale: [
    { value: "recover", label: "用新证据恢复" },
    { value: "payoff", label: "用新证据回收" },
    { value: "abandon", label: "废弃" },
  ],
};

function scenarioId(prefix: string): string {
  return `${prefix}-${crypto.randomUUID()}`;
}

function statusBadgeClass(status: string): string {
  if (status === "resolved") return "bg-emerald-100 text-emerald-900";
  if (status === "stale" || status === "superseded" || status === "disputed") return "bg-amber-100 text-amber-900";
  if (status === "abandoned") return "bg-muted text-muted-foreground";
  return "bg-sky-100 text-sky-900";
}

/**
 * 「剧情线索」：伏笔生命周期、按章时间线与因果关系的作者工作区。
 *
 * 所有写操作都走内核的版本 CAS 与证据校验；界面只收集作者输入，不自动推进或回收。
 */
export default function PlotWorkspaceTab(props: PlotWorkspaceTabProps) {
  const { novelId, chapterId, onOpenChapter } = props;
  const queryClient = useQueryClient();
  const [newTitle, setNewTitle] = useState("");
  const [newBody, setNewBody] = useState("");
  const [newEvidence, setNewEvidence] = useState("");
  const [actionByPlot, setActionByPlot] = useState<Record<string, string>>({});
  const [evidenceByPlot, setEvidenceByPlot] = useState<Record<string, string>>({});
  const [kindFilter, setKindFilter] = useState("all");
  const [plotFilter, setPlotFilter] = useState("all");
  const [volumeFilter, setVolumeFilter] = useState("all");
  const [edgeFrom, setEdgeFrom] = useState("");
  const [edgeTo, setEdgeTo] = useState("");
  const [edgeRelation, setEdgeRelation] = useState("导致");
  const [edgeEvidence, setEdgeEvidence] = useState("");

  const workspaceQuery = useQuery({
    queryKey: queryKeys.novels.plotWorkspace(novelId, chapterId),
    queryFn: () => getPlotWorkspace({ novelId, chapterId }),
    enabled: Boolean(novelId && chapterId),
  });

  const invalidate = () => {
    void queryClient.invalidateQueries({ queryKey: queryKeys.novels.plotWorkspace(novelId, chapterId) });
    void queryClient.invalidateQueries({ queryKey: queryKeys.novels.chapterContinuity(novelId, chapterId) });
    void queryClient.invalidateQueries({ queryKey: queryKeys.novels.chapterContextPreview(novelId, chapterId) });
  };

  const createMutation = useMutation({
    mutationFn: () =>
      createPlotThread({
        plotId: scenarioId("plot"),
        novelId,
        title: newTitle.trim(),
        body: newBody.trim(),
        chapterId,
        sourceRevision: workspaceQuery.data!.chapterVersion,
        evidence: newEvidence.trim(),
        commandId: scenarioId("plot-create"),
      }),
    onSuccess: () => {
      toast.success("已记录这条伏笔。");
      setNewTitle("");
      setNewBody("");
      setNewEvidence("");
      invalidate();
    },
    onError: (error: unknown) => toast.error(error instanceof Error ? error.message : "伏笔没有保存。"),
  });

  const transitionMutation = useMutation({
    mutationFn: (thread: PlotThreadView) => {
      const action = actionByPlot[thread.id] ?? ACTIONS_BY_STATUS[thread.status]?.[0]?.value ?? "";
      const evidence = (evidenceByPlot[thread.id] ?? "").trim();
      if (!action) throw new Error("请选择伏笔动作。");
      if (!evidence) throw new Error("请从当前章节正文中填写证据。");
      return transitionPlotThread({
        plotId: thread.id,
        novelId,
        expectedVersion: thread.version,
        action,
        chapterId,
        sourceRevision: workspaceQuery.data!.chapterVersion,
        evidence,
        commandId: scenarioId(`plot-${action}`),
      });
    },
    onSuccess: () => {
      toast.success("伏笔状态已更新。");
      invalidate();
    },
    onError: (error: unknown) => toast.error(error instanceof Error ? error.message : "伏笔状态没有更新。"),
  });

  const edgeMutation = useMutation({
    mutationFn: () => {
      if (!edgeFrom || !edgeTo) throw new Error("请选择因果关系的起点和终点。");
      if (edgeFrom === edgeTo) throw new Error("事件不能依赖自身。");
      const [fromKind, fromId] = edgeFrom.split(":");
      const [toKind, toId] = edgeTo.split(":");
      if (!edgeRelation.trim() || !edgeEvidence.trim()) throw new Error("请填写关系说明和证据。");
      return createPlotCausality({
        edgeId: scenarioId("edge"),
        novelId,
        fromKind: fromKind!,
        fromId: fromId!,
        toKind: toKind!,
        toId: toId!,
        relation: edgeRelation.trim(),
        chapterId,
        sourceRevision: workspaceQuery.data!.chapterVersion,
        evidence: edgeEvidence.trim(),
      });
    },
    onSuccess: () => {
      toast.success("因果关系已记录。");
      setEdgeRelation("导致");
      setEdgeEvidence("");
      invalidate();
    },
    onError: (error: unknown) => toast.error(error instanceof Error ? error.message : "因果关系没有保存。"),
  });

  const workspace = workspaceQuery.data;
  const timeline = workspace?.timeline ?? [];
  const volumes = useMemo(() => {
    const map = new Map<string, string>();
    for (const entry of timeline) {
      if (entry.volumeId) map.set(entry.volumeId, entry.volumeTitle ?? entry.volumeId);
    }
    return [...map.entries()];
  }, [timeline]);
  const filteredTimeline = timeline.filter((entry) => {
    if (kindFilter !== "all" && entry.kind !== kindFilter) return false;
    if (plotFilter !== "all" && entry.plotId !== plotFilter) return false;
    if (volumeFilter !== "all" && entry.volumeId !== volumeFilter) return false;
    return true;
  });
  const edgeOptions = useMemo(() => {
    const options: Array<{ value: string; label: string }> = [];
    for (const entry of timeline) {
      if (entry.kind === "timeline_event") {
        options.push({ value: `timeline_event:${entry.id}`, label: `第${entry.chapterOrder}章 事件：${entry.body.slice(0, 24)}` });
      }
    }
    for (const thread of workspace?.threads ?? []) {
      options.push({ value: `plot_thread:${thread.id}`, label: `伏笔：${thread.title}` });
    }
    return options;
  }, [timeline, workspace?.threads]);

  if (workspaceQuery.isLoading) {
    return <div className="px-1 py-4 text-sm text-muted-foreground" data-testid="plot-loading">正在整理伏笔与时间线...</div>;
  }
  if (workspaceQuery.isError) {
    return (
      <div className="space-y-2 px-1 py-4 text-sm text-muted-foreground" data-testid="plot-error">
        <div>剧情线索读取失败；这不是「没有伏笔」，请检查本地数据后重试。</div>
        <Button size="sm" variant="outline" onClick={() => void workspaceQuery.refetch()}>重试</Button>
      </div>
    );
  }
  if (!workspace) return null;

  return (
    <div className="flex flex-col gap-4 px-1 py-2" data-testid="plot-workspace">
      <div className="text-xs text-muted-foreground">
        截至第 {workspace.chapterOrder} 章（版本 {workspace.chapterVersion}）· 伏笔 {workspace.threads.length} 条 ·
        时间线 {timeline.length} 条{workspace.timelineTruncated ? "（已截断）" : ""}
      </div>

      <section className="space-y-2" data-testid="plot-create-form">
        <h3 className="text-sm font-medium text-foreground">埋设一条伏笔</h3>
        <input
          className="w-full rounded-lg border border-border bg-background px-2 py-1 text-xs"
          data-testid="plot-new-title"
          placeholder="伏笔标题"
          value={newTitle}
          onChange={(event) => setNewTitle(event.target.value)}
        />
        <textarea
          className="h-16 w-full rounded-lg border border-border bg-background px-2 py-1 text-xs"
          data-testid="plot-new-body"
          placeholder="这条伏笔是什么，准备如何回收"
          value={newBody}
          onChange={(event) => setNewBody(event.target.value)}
        />
        <textarea
          className="h-12 w-full rounded-lg border border-border bg-background px-2 py-1 text-xs"
          data-testid="plot-new-evidence"
          placeholder="当前章节中支持这条伏笔的原文证据"
          value={newEvidence}
          onChange={(event) => setNewEvidence(event.target.value)}
        />
        <Button
          size="sm"
          data-testid="plot-create-submit"
          disabled={createMutation.isPending || !newTitle.trim() || !newBody.trim() || !newEvidence.trim()}
          onClick={() => createMutation.mutate()}
        >
          记录伏笔
        </Button>
      </section>

      <section className="space-y-2">
        <h3 className="text-sm font-medium text-foreground">伏笔生命周期</h3>
        {workspace.threads.length === 0 ? (
          <div className="text-xs text-muted-foreground">还没有伏笔。可以在当前章节埋设第一条。</div>
        ) : (
          workspace.threads.map((thread) => {
            const behindLatest =
              thread.latestEventChapterOrder != null
              && workspace.chapterOrder < thread.latestEventChapterOrder;
            const actions = behindLatest ? [] : (ACTIONS_BY_STATUS[thread.status] ?? []);
            return (
              <div key={thread.id} className="rounded-xl bg-muted/30 px-3 py-2" data-testid="plot-thread">
                <div className="flex flex-wrap items-center gap-2">
                  <span className={`rounded-full px-2 py-0.5 text-[11px] ${statusBadgeClass(thread.status)}`} data-testid="plot-status">
                    {STATUS_LABELS[thread.status] ?? thread.status}
                  </span>
                  <span className="text-sm text-foreground">{thread.title}</span>
                </div>
                <div className="mt-1 text-xs leading-5 text-muted-foreground">{thread.body}</div>
                <div className="mt-1 text-[11px] text-muted-foreground">
                  埋设第 {thread.openedChapterOrder} 章
                  {thread.lastDevelopmentChapterOrder != null ? ` · 推进第 ${thread.lastDevelopmentChapterOrder} 章` : ""}
                  {thread.resolvedChapterOrder != null ? ` · 回收第 ${thread.resolvedChapterOrder} 章` : ""}
                  {thread.sourceChapterOrder != null ? ` · 来源第 ${thread.sourceChapterOrder} 章` : " · 来源不可用"}
                  {thread.status === "stale" && thread.lifecycleStatus !== "stale"
                    ? ` · 生命周期${STATUS_LABELS[thread.lifecycleStatus] ?? thread.lifecycleStatus}`
                    : ""}
                </div>
                {thread.evidence ? (
                  <div className="mt-1 text-[11px] leading-5 text-muted-foreground">证据：{thread.evidence}</div>
                ) : null}
                {behindLatest ? (
                  <div className="mt-1 text-[11px] text-muted-foreground">
                    这条伏笔的最新动作在第 {thread.latestEventChapterOrder} 章；到那一章继续操作。
                  </div>
                ) : null}
                {!thread.sourceValid || thread.status === "stale" ? (
                  <div className="mt-1 text-[11px] text-amber-800">
                    来源证据不可用；继续操作前请填写当前正文的新证据。
                  </div>
                ) : null}
                {actions.length > 0 ? (
                  <div className="mt-2 space-y-2">
                    <div className="flex flex-wrap gap-2">
                      {actions.map((action) => {
                        const selected = (actionByPlot[thread.id] ?? actions[0]!.value) === action.value;
                        return (
                          <button
                            key={action.value}
                            type="button"
                            data-testid={`plot-action-${action.value}`}
                            data-active={String(selected)}
                            className={`rounded-lg px-2 py-1 text-xs ${
                              selected
                                ? "bg-foreground text-background"
                                : "border border-border bg-background text-muted-foreground"
                            }`}
                            onClick={() =>
                              setActionByPlot((current) => ({ ...current, [thread.id]: action.value }))
                            }
                          >
                            {action.label}
                          </button>
                        );
                      })}
                    </div>
                    <div className="flex flex-wrap items-center gap-2">
                      <input
                        className="min-w-0 flex-1 rounded-lg border border-border bg-background px-2 py-1 text-xs"
                        data-testid="plot-evidence"
                        placeholder="当前章节的原文证据"
                        value={evidenceByPlot[thread.id] ?? ""}
                        onChange={(event) =>
                          setEvidenceByPlot((current) => ({ ...current, [thread.id]: event.target.value }))
                        }
                      />
                      <Button
                        size="sm"
                        variant="outline"
                        data-testid="plot-transition-submit"
                        disabled={transitionMutation.isPending || !(evidenceByPlot[thread.id] ?? "").trim()}
                        onClick={() => transitionMutation.mutate(thread)}
                      >
                        执行
                      </Button>
                    </div>
                  </div>
                ) : null}
              </div>
            );
          })
        )}
      </section>

      <section className="space-y-2">
        <h3 className="text-sm font-medium text-foreground">按章时间线</h3>
        <div className="flex flex-wrap gap-2">
          <select
            className="rounded-lg border border-border bg-background px-2 py-1 text-xs"
            data-testid="plot-timeline-filter-kind"
            value={kindFilter}
            onChange={(event) => setKindFilter(event.target.value)}
          >
            <option value="all">全部类型</option>
            {Object.entries(TIMELINE_LABELS).map(([kind, label]) => (
              <option key={kind} value={kind}>{label}</option>
            ))}
          </select>
          <select
            className="rounded-lg border border-border bg-background px-2 py-1 text-xs"
            value={plotFilter}
            onChange={(event) => setPlotFilter(event.target.value)}
          >
            <option value="all">全部伏笔</option>
            {workspace.threads.map((thread) => (
              <option key={thread.id} value={thread.id}>{thread.title}</option>
            ))}
          </select>
          <select
            className="rounded-lg border border-border bg-background px-2 py-1 text-xs"
            value={volumeFilter}
            onChange={(event) => setVolumeFilter(event.target.value)}
          >
            <option value="all">全部卷</option>
            {volumes.map(([id, title]) => (
              <option key={id} value={id}>{title}</option>
            ))}
          </select>
        </div>
        {workspace.timelineTruncated ? (
          <div className="text-[11px] text-amber-800" data-testid="plot-timeline-partial">
            时间线只加载最近 {workspace.timelineLimit} 条；筛选只作用于这些条目，更早记录未加载。
          </div>
        ) : null}
        {filteredTimeline.length === 0 ? (
          <div className="text-xs text-muted-foreground">
            {workspace.timelineTruncated
              ? `最近 ${workspace.timelineLimit} 条中没有匹配；更早记录未加载。`
              : "当前筛选下没有时间线条目。"}
          </div>
        ) : (
          <ul className="space-y-1">
            {filteredTimeline.map((entry: TimelineEntryView) => (
              <li key={`${entry.kind}:${entry.id}`} className="rounded-lg bg-muted/20 px-2 py-1" data-testid="plot-timeline-entry">
                <div className="flex flex-wrap items-center gap-2 text-[11px] text-muted-foreground">
                  <span>第 {entry.chapterOrder} 章</span>
                  <span>{TIMELINE_LABELS[entry.kind] ?? entry.kind}</span>
                  <span>{STATUS_LABELS[entry.status] ?? entry.status}</span>
                  {!entry.sourceValid ? <span className="text-amber-800">来源失效</span> : null}
                  {entry.chapterId && onOpenChapter ? (
                    <button
                      type="button"
                      className="underline underline-offset-2 hover:text-foreground"
                      data-testid="plot-chapter-link"
                      onClick={() => onOpenChapter(entry.chapterId)}
                    >
                      跳到来源
                    </button>
                  ) : null}
                </div>
                <div className="text-xs leading-5 text-foreground">{entry.title} · {entry.body}</div>
                {entry.evidence ? (
                  <div className="text-[11px] leading-5 text-muted-foreground">证据：{entry.evidence}</div>
                ) : null}
              </li>
            ))}
          </ul>
        )}
        {workspace.timelineTruncated ? (
          <div className="text-[11px] text-amber-800">
            时间线只加载最近 {workspace.timelineLimit} 条；更早记录未加载。
          </div>
        ) : null}
      </section>

      <section className="space-y-2">
        <h3 className="text-sm font-medium text-foreground">剧情因果</h3>
        {workspace.edges.length === 0 ? (
          <div className="text-xs text-muted-foreground">还没有因果关系。</div>
        ) : (
          <ul className="space-y-1">
            {workspace.edges.map((edge) => (
              <li key={edge.id} className="rounded-lg bg-muted/20 px-2 py-1" data-testid="plot-causality-edge">
                <div className="text-xs text-foreground">
                  {edge.fromLabel ?? edge.fromId} → {edge.toLabel ?? edge.toId} · {edge.relation}
                </div>
                <div className="text-[11px] text-muted-foreground">
                  证据：{edge.evidence}
                  {edge.sourceChapterOrder != null ? ` · 第 ${edge.sourceChapterOrder} 章` : ""}
                  {!edge.sourceValid ? " · 来源已失效" : ""}
                  {!edge.fromValid || !edge.toValid ? " · 引用对象失效" : ""}
                </div>
              </li>
            ))}
          </ul>
        )}
        <div className="space-y-2 rounded-xl bg-muted/20 px-3 py-2">
          <div className="flex flex-wrap gap-2">
            <select
              className="min-w-0 flex-1 rounded-lg border border-border bg-background px-2 py-1 text-xs"
              data-testid="plot-edge-from"
              value={edgeFrom}
              onChange={(event) => setEdgeFrom(event.target.value)}
            >
              <option value="">起点事件/伏笔</option>
              {edgeOptions.map((option) => (
                <option key={option.value} value={option.value}>{option.label}</option>
              ))}
            </select>
            <select
              className="min-w-0 flex-1 rounded-lg border border-border bg-background px-2 py-1 text-xs"
              data-testid="plot-edge-to"
              value={edgeTo}
              onChange={(event) => setEdgeTo(event.target.value)}
            >
              <option value="">终点事件/伏笔</option>
              {edgeOptions.map((option) => (
                <option key={option.value} value={option.value}>{option.label}</option>
              ))}
            </select>
          </div>
          <input
            className="w-full rounded-lg border border-border bg-background px-2 py-1 text-xs"
            data-testid="plot-edge-relation"
            placeholder="关系，例如：导致 / 依赖 / 回收"
            value={edgeRelation}
            onChange={(event) => setEdgeRelation(event.target.value)}
          />
          <textarea
            className="h-12 w-full rounded-lg border border-border bg-background px-2 py-1 text-xs"
            data-testid="plot-edge-evidence"
            placeholder="当前章节中支持这条因果关系的原文证据"
            value={edgeEvidence}
            onChange={(event) => setEdgeEvidence(event.target.value)}
          />
          <Button
            size="sm"
            variant="outline"
            data-testid="plot-edge-submit"
            disabled={edgeMutation.isPending || !edgeFrom || !edgeTo || !edgeEvidence.trim()}
            onClick={() => edgeMutation.mutate()}
          >
            记录因果关系
          </Button>
        </div>
      </section>
    </div>
  );
}
