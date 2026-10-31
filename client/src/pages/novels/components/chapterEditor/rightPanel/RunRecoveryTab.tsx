import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "@/components/ui/toast";
import { Button } from "@/components/ui/button";
import {
  approveRun,
  cancelRun,
  getRunDetail,
  listRuns,
  resumeRun,
  resumeRunWithQualityDebt,
} from "@/platform/desktopTransport";
import { availableRunActions, formatRunStatus, isRecoverableStatus } from "./panelModel";

interface RunRecoveryTabProps {
  novelId: string;
  chapterId: string;
}

function newCommandId(prefix: string): string {
  return `${prefix}-${crypto.randomUUID()}`;
}

/**
 * 「运行与恢复」：只读展示当前 Run 的真实状态，动作严格取自后端 legalActions，
 * 不在前端臆造入口，也不直接改数据库。
 */
export default function RunRecoveryTab(props: RunRecoveryTabProps) {
  const { novelId, chapterId } = props;
  const queryClient = useQueryClient();

  const runsQuery = useQuery({
    queryKey: ["authoring-runs", novelId],
    queryFn: () => listRuns(novelId),
    refetchInterval: 5000,
  });

  const chapterRuns = (runsQuery.data ?? []).filter((run) => run.chapterId === chapterId);
  const latest = (chapterRuns.length > 0 ? chapterRuns : runsQuery.data ?? [])
    .slice()
    .sort((left, right) => right.updatedAt - left.updatedAt)[0];

  const detailQuery = useQuery({
    queryKey: ["authoring-run-detail", novelId, latest?.id ?? "none"],
    queryFn: () => getRunDetail(novelId, latest!.id),
    enabled: Boolean(latest?.id),
    refetchInterval: 5000,
  });

  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: ["authoring-runs", novelId] });
    void queryClient.invalidateQueries({ queryKey: ["authoring-run-detail", novelId] });
  };

  const actionMutation = useMutation({
    mutationFn: async (action: string) => {
      if (!latest) return;
      const commandId = newCommandId(`chapter-workspace-${action}`);
      if (action === "approve") return approveRun({ runId: latest.id, commandId });
      if (action === "resume") return resumeRun({ runId: latest.id, commandId, acceptCandidate: false });
      if (action === "resume_debt")
        return resumeRunWithQualityDebt({
          runId: latest.id,
          commandId,
          reason: "章节工作台里确认带质量提醒继续",
        });
      if (action === "regenerate") return resumeRun({ runId: latest.id, commandId, acceptCandidate: false });
      if (action === "cancel") return cancelRun({ runId: latest.id, commandId });
      return undefined;
    },
    onSuccess: () => {
      refresh();
    },
    onError: (error: unknown) => {
      toast.error(error instanceof Error ? error.message : "操作没有完成，请稍后重试。");
    },
  });

  if (runsQuery.isLoading) {
    return <div className="px-1 py-4 text-sm text-muted-foreground">正在读取运行状态...</div>;
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
      <div className="px-1 py-4 text-sm text-muted-foreground">
        这本书还没有运行记录。先在「AI 修改」里发起一次创作，这里会显示进度与可执行的下一步。
      </div>
    );
  }

  const detail = detailQuery.data;
  const status = detail?.status ?? latest.state;
  const actions = availableRunActions(detail?.legalActions);
  const reason = detail?.pauseReason ?? detail?.failureReason ?? null;

  return (
    <div className="flex flex-col gap-3 px-1 py-2" data-testid="run-recovery-tab">
      <div className="space-y-1">
        <div className="text-sm font-medium text-foreground" data-testid="run-status">
          {detail?.chapterTitle || latest.chapterTitle}：{formatRunStatus(status)}
        </div>
        <div className="text-xs text-muted-foreground">
          {detail?.provider ?? latest.provider} / {detail?.model ?? latest.model}
          {detail?.usage ? ` · 已调用 ${detail.usage.calls} 次` : ""}
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
            variant={action.key === "cancel" ? "outline" : "default"}
            disabled={actionMutation.isPending}
            onClick={() => actionMutation.mutate(action.key)}
            data-testid={`run-action-${action.key}`}
          >
            {action.label}
          </Button>
        ))}
        {actions.length === 0 && !isRecoverableStatus(status) ? (
          <span className="text-xs text-muted-foreground">当前没有需要你处理的步骤。</span>
        ) : null}
      </div>
    </div>
  );
}
