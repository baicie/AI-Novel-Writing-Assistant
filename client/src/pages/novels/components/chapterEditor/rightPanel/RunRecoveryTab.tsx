import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "react-router-dom";
import { toast } from "@/components/ui/toast";
import { Button } from "@/components/ui/button";
import { queryKeys } from "@/api/queryKeys";
import {
  approveRun,
  cancelRun,
  confirmCandidateAfterEdit,
  getRunDetail,
  listRuns,
  regenerateRun,
  rejectRun,
  resumeRun,
  resumeRunWithQualityDebt,
} from "@/platform/desktopTransport";
import { availableRunActions, formatRunStatus, isRecoverableStatus, type RunActionOption } from "./panelModel";

interface RunRecoveryTabProps {
  novelId: string;
  chapterId: string;
}

function newCommandId(prefix: string): string {
  return `${prefix}-${crypto.randomUUID()}`;
}

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

  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: ["authoring-runs", novelId] });
    void queryClient.invalidateQueries({ queryKey: ["authoring-run-detail", novelId] });
    void queryClient.invalidateQueries({ queryKey: queryKeys.novels.detail(novelId) });
    void queryClient.invalidateQueries({ queryKey: queryKeys.novels.chapterEditorWorkspace(novelId, chapterId) });
  };

  const actionMutation = useMutation({
    mutationFn: async (action: RunActionOption) => {
      if (!latest) throw new Error("还没有可操作的运行。");
      const commandId = newCommandId(`chapter-workbench-${action.key}`);
      const expectedVersion = detailQuery.data?.controlVersion ?? undefined;

      switch (action.command) {
        case "approve_run":
          return approveRun({ runId: latest.id, commandId, expectedVersion });
        case "reject_run":
          return rejectRun({ runId: latest.id, commandId, expectedVersion });
        case "regenerate_run":
          return regenerateRun({ runId: latest.id, commandId, expectedVersion });
        case "confirm_candidate_after_edit_run": {
          const candidate = detailQuery.data?.latestCandidate;
          if (!candidate?.id || !candidate.contentSha256 || !candidate.content) {
            throw new Error("候选正文还没准备好，请刷新后再确认。");
          }
          return confirmCandidateAfterEdit({
            runId: latest.id,
            commandId,
            expectedVersion,
            baseCandidateId: candidate.id,
            baseContentSha256: candidate.contentSha256,
            content: candidate.content,
          });
        }
        case "resume_run":
          return resumeRun({ runId: latest.id, commandId, acceptCandidate: false });
        case "resume_run_with_quality_debt":
          return resumeRunWithQualityDebt({
            runId: latest.id,
            commandId,
            reason: "章节工作台里确认带质量提醒继续",
          });
        case "cancel_run":
          return cancelRun({ runId: latest.id, commandId });
        default:
          throw new Error("这个动作暂时不能在章节工作台执行。");
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
