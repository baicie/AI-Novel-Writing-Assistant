import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "react-router-dom";
import { toast } from "@/components/ui/toast";
import { Button } from "@/components/ui/button";
import { queryKeys } from "@/api/queryKeys";
import {
  approveRun,
  cancelRun,
  confirmCandidateAfterEdit,
  getRunContextTrace,
  getRunDetail,
  listRuns,
  regenerateRun,
  rejectRun,
  resumeRun,
  resumeRunWithQualityDebt,
} from "@/platform/desktopTransport";
import {
  availableRunActions,
  buildRunCommandRequest,
  formatRunStatus,
  isRecoverableStatus,
  type RunActionOption,
} from "./panelModel";

interface RunRecoveryTabProps {
  novelId: string;
  chapterId: string;
}

function newCommandId(prefix: string): string {
  return `${prefix}-${crypto.randomUUID()}`;
}

/** 写进运行记录的恢复原因，便于之后回看这一步是带着质量提醒继续的。 */
const QUALITY_DEBT_REASON = "章节工作台里确认带质量提醒继续";

/**
 * 「运行与恢复」：只展示**当前章节**的 Run，动作严格映射到真实 Director 命令。
 * 本章没有 Run 时给出明确空态，并引导到全书运行记录，绝不静默操作别的章节。
 */
export default function RunRecoveryTab(props: RunRecoveryTabProps) {
  const { novelId, chapterId } = props;
  const queryClient = useQueryClient();

  const runsQuery = useQuery({
    queryKey: ["authoring-runs", novelId],
    queryFn: () => listRuns(novelId),
    refetchInterval: 5000,
    enabled: Boolean(novelId && chapterId),
  });

  const chapterRuns = (runsQuery.data ?? []).filter((run) => run.chapterId === chapterId);
  const latest = chapterRuns.slice().sort((left, right) => right.updatedAt - left.updatedAt)[0];

  const detailQuery = useQuery({
    // 章节 ID 进 key：快速切章时不同章节的详情互相隔离，旧响应不会写进新章节。
    queryKey: ["authoring-run-detail", novelId, chapterId, latest?.id ?? "none"],
    queryFn: () => getRunDetail(novelId, latest!.id),
    enabled: Boolean(latest?.id),
    refetchInterval: 5000,
  });

  // 运行冻结的上下文 trace：只读该 Run 自己保存的版本，不跟随数据库最新状态。
  const runTraceQuery = useQuery({
    queryKey: queryKeys.novels.runContextTrace(novelId, latest?.id ?? "none"),
    queryFn: () => getRunContextTrace({ novelId, runId: latest!.id }),
    enabled: Boolean(latest?.id),
    staleTime: Infinity,
  });

  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: ["authoring-runs", novelId] });
    void queryClient.invalidateQueries({ queryKey: ["authoring-run-detail", novelId] });
    void queryClient.invalidateQueries({ queryKey: queryKeys.novels.detail(novelId) });
    void queryClient.invalidateQueries({ queryKey: queryKeys.novels.chapterEditorWorkspace(novelId, chapterId) });
  };

  const actionMutation = useMutation({
    mutationFn: async (action: RunActionOption) => {
      if (!latest) throw new Error("还没有可操作的运行。");
      // 参数一律由 panelModel 派生，这里只负责把它交给对应的命令，不重复手写参数。
      const request = buildRunCommandRequest(action, {
        runId: latest.id,
        commandId: newCommandId(`chapter-workbench-${action.key}`),
        controlVersion: detailQuery.data?.controlVersion,
        candidate: detailQuery.data?.latestCandidate ?? null,
        qualityDebtReason: QUALITY_DEBT_REASON,
      });

      switch (request.command) {
        case "approve_run":
          return approveRun(request.payload);
        case "reject_run":
          return rejectRun(request.payload);
        case "regenerate_run":
          return regenerateRun(request.payload);
        case "confirm_candidate_after_edit_run":
          return confirmCandidateAfterEdit(request.payload);
        case "resume_run":
          return resumeRun(request.payload);
        case "resume_run_with_quality_debt":
          return resumeRunWithQualityDebt(request.payload);
        case "cancel_run":
          return cancelRun(request.payload);
      }
    },
    onSuccess: () => {
      refresh();
    },
    onError: (error: unknown) => {
      toast.error(error instanceof Error ? error.message : "操作没有完成，请稍后重试。");
    },
  });

  if (runsQuery.isLoading) {
    return <div className="px-1 py-4 text-sm text-muted-foreground">正在读取本章运行状态...</div>;
  }
  if (runsQuery.isError) {
    return (
      <div className="space-y-2 px-1 py-4 text-sm text-muted-foreground">
        <div>运行状态读取失败。</div>
        <Button size="sm" variant="outline" onClick={() => void runsQuery.refetch()}>
          重试
        </Button>
      </div>
    );
  }
  if (!latest) {
    return (
      <div className="space-y-2 px-1 py-4 text-sm text-muted-foreground" data-testid="run-empty">
        <p>本章还没有运行记录。在「AI 修改」里发起一次改写或生成，这里会显示进度与可执行的下一步。</p>
        <Link
          to="/tasks"
          data-testid="run-history-link"
          className="inline-block text-xs underline-offset-4 hover:text-foreground hover:underline"
        >
          查看全书运行记录
        </Link>
      </div>
    );
  }

  const detail = detailQuery.data;
  // 详情必须属于本章：异步旧响应绝不驱动当前界面。
  const detailMatches = Boolean(detail && detail.chapterId === chapterId && detail.id === latest.id);
  const status = detailMatches ? detail!.status : latest.state;
  const actions = detailMatches
    ? availableRunActions({
        legalActions: detail!.legalActions,
        status: detail!.status,
        gateId: detail!.gate?.id ?? null,
        candidate: detail!.latestCandidate ?? null,
        controlVersion: detail!.controlVersion,
      })
    : [];
  const reason = detailMatches ? (detail!.pauseReason ?? detail!.failureReason ?? null) : null;

  return (
    <div className="flex flex-col gap-3 px-1 py-2" data-testid="run-recovery-tab">
      <div className="space-y-1">
        <div className="text-sm font-medium text-foreground" data-testid="run-status">
          {formatRunStatus(status)}
        </div>
        <div className="text-xs text-muted-foreground">
          {detailMatches ? detail!.provider : latest.provider} / {detailMatches ? detail!.model : latest.model}
          {detailMatches ? ` · 已调用 ${detail!.usage.calls} 次` : ""}
        </div>
      </div>

      {reason ? (
        <div className="rounded-xl bg-muted/50 px-3 py-2 text-xs leading-5 text-muted-foreground" data-testid="run-reason">
          {reason}
        </div>
      ) : null}

      {isRecoverableStatus(status) && actions.length === 0 ? (
        <div className="rounded-xl bg-muted/40 px-3 py-2 text-xs text-muted-foreground">
          这次运行没有可自动执行的下一步，请在「AI 修改」里重新发起，或到运行记录里查看详情。
        </div>
      ) : null}

      <div className="flex flex-wrap gap-2">
        {actions.map((action) => (
          <Button
            key={action.key}
            size="sm"
            variant={action.destructive ? "outline" : "default"}
            disabled={actionMutation.isPending}
            onClick={() => actionMutation.mutate(action)}
            data-testid={`run-action-${action.key}`}
          >
            {action.label}
          </Button>
        ))}
        {actions.length === 0 && !isRecoverableStatus(status) ? (
          <span className="text-xs text-muted-foreground">当前没有需要你处理的步骤。</span>
        ) : null}
      </div>

      <div className="space-y-2 border-t border-border/70 pt-3" data-testid="run-context-trace">
        <div className="text-sm font-medium text-foreground">这次运行引用的上下文</div>
        {runTraceQuery.isLoading ? (
          <div className="text-xs text-muted-foreground">正在读取冻结的上下文来源...</div>
        ) : runTraceQuery.isError ? (
          <div className="text-xs text-amber-800">上下文来源读取失败；历史运行不能被当成「没有来源」。</div>
        ) : !runTraceQuery.data ? (
          <div className="text-xs text-muted-foreground">这次运行还没有保存上下文来源。</div>
        ) : (
          <>
            <div className="text-[11px] text-muted-foreground">
              版本 {runTraceQuery.data.assemblyVersion} · 预算 {runTraceQuery.data.inputBudget} · 使用 {runTraceQuery.data.estimatedTokens} tokens
            </div>
            <ul className="space-y-1">
              {runTraceQuery.data.sources.filter((source) => source.selected).map((source) => (
                <li key={`${source.sourceType}:${source.sourceId}`} className="rounded-lg bg-muted/30 px-2 py-1" data-testid="run-context-source">
                  <span className="text-[11px] text-foreground">{source.sourceType}</span>
                  <span className="ml-2 text-[11px] text-muted-foreground">{source.sourceId || "（无编号）"} · 版本 {source.sourceVersion ?? "未知"} · 约 {source.estimatedTokens} tokens</span>
                </li>
              ))}
            </ul>
            {runTraceQuery.data.sources.some((source) => !source.selected) ? (
              <div className="text-[11px] text-amber-800">
                有 {runTraceQuery.data.sources.filter((source) => !source.selected).length} 个来源未使用（缺失、过期或超出预算）。
              </div>
            ) : null}
          </>
        )}
      </div>

      <Link
        to="/tasks"
        data-testid="run-history-link"
        className="text-xs text-muted-foreground underline-offset-4 hover:text-foreground hover:underline"
      >
        查看完整运行记录
      </Link>
    </div>
  );
}
