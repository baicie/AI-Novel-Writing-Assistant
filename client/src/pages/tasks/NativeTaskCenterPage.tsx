import { useEffect, useMemo, useState } from "react";
import { Button } from "@/components/ui/button";
import { toast } from "@/components/ui/toast";
import {
  cancelBatch,
  getBatch,
  listNativeTasks,
  pauseBatch,
  resumeBatch,
  retryIndexJob,
  type NativeTaskSummary,
} from "@/platform/desktopTransport";

const statusLabel: Record<string, string> = {
  queued: "排队中", running: "进行中", waiting_approval: "等待确认", paused: "已暂停",
  failed: "失败", cancelled: "已取消", succeeded: "已完成", completed: "已完成",
  retry_wait: "等待重试", obsolete: "已过期", created: "已创建", open: "待处理",
};

const typeLabel: Record<string, string> = {
  director_run: "单章创作", batch: "连续创作", index_job: "知识索引",
  memory_extraction: "记忆抽取", quality_debt: "质量问题",
};

const filters = [
  { key: "", label: "全部" },
  { key: "running", label: "进行中" },
  { key: "action", label: "等待作者" },
  { key: "paused", label: "已暂停" },
  { key: "failed", label: "失败" },
  { key: "done", label: "已完成" },
] as const;

function matchesFilter(task: NativeTaskSummary, filter: string): boolean {
  if (!filter) return true;
  if (filter === "action") return task.requiresUserAction;
  if (filter === "running") return ["running", "queued", "waiting_approval", "retry_wait"].includes(task.status);
  if (filter === "done") return ["succeeded", "completed"].includes(task.status);
  return task.status === filter;
}

export default function NativeTaskCenterPage() {
  const [tasks, setTasks] = useState<NativeTaskSummary[]>([]);
  const [filter, setFilter] = useState<string>("");
  const [typeFilter, setTypeFilter] = useState("");
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = async () => {
    setLoading(true);
    try {
      setTasks(await listNativeTasks({ limit: 200 }));
      setError(null);
    } catch (cause: unknown) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void refresh();
    const timer = window.setInterval(() => { void refresh(); }, 4000);
    return () => window.clearInterval(timer);
  }, []);

  const visible = useMemo(
    () => tasks.filter((task) => matchesFilter(task, filter) && (!typeFilter || task.taskType === typeFilter)),
    [tasks, filter, typeFilter],
  );
  const actionCount = tasks.filter((task) => task.requiresUserAction).length;

  const open = (route: string) => {
    window.location.hash = `#${route}`;
  };

  const runAction = async (task: NativeTaskSummary, action: string) => {
    setBusy(task.taskId);
    setError(null);
    try {
      if (task.taskType === "batch" && task.batchId && ["pause", "resume", "cancel"].includes(action)) {
        const current = await getBatch(task.batchId);
        if (action === "pause") await pauseBatch(task.batchId, current.version);
        if (action === "resume") await resumeBatch(task.batchId, current.version);
        if (action === "cancel") await cancelBatch(task.batchId, current.version);
        toast.success("批量任务状态已更新。");
      } else if (task.taskType === "index_job" && action === "retry") {
        await retryIndexJob({ jobId: task.taskId });
        toast.success("索引任务已重新排队。");
      } else {
        open(task.detailRoute);
      }
      await refresh();
    } catch (cause: unknown) {
      const message = cause instanceof Error ? cause.message : String(cause);
      setError(`操作未完成：${message}。已重新读取最新任务状态。`);
      await refresh();
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="mx-auto flex h-full min-h-0 w-full max-w-6xl flex-col gap-4 overflow-y-auto p-4">
      <header className="flex flex-wrap items-end justify-between gap-2 border-b border-border/50 pb-3">
        <div>
          <h1 className="text-lg font-semibold">运行记录</h1>
          <p className="mt-1 text-xs text-muted-foreground">集中查看单章创作、连续创作、知识索引和记忆抽取的持久化状态。</p>
        </div>
        <Button variant="outline" onClick={() => void refresh()} disabled={loading}>{loading ? "刷新中..." : "刷新"}</Button>
      </header>

      <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
        {[
          ["进行中", tasks.filter((task) => ["running", "queued", "retry_wait"].includes(task.status)).length],
          ["等待作者", actionCount],
          ["失败", tasks.filter((task) => task.status === "failed").length],
          ["已完成", tasks.filter((task) => ["succeeded", "completed"].includes(task.status)).length],
        ].map(([label, count]) => (
          <div key={String(label)} className="rounded-xl bg-muted/25 px-3 py-2">
            <div className="text-xs text-muted-foreground">{label}</div>
            <div className="mt-1 text-lg font-semibold">{count}</div>
          </div>
        ))}
      </div>

      <div className="flex flex-wrap items-center gap-2">
        {filters.map((item) => (
          <Button key={item.key} size="sm" variant={filter === item.key ? "default" : "outline"} onClick={() => setFilter(item.key)}>{item.label}</Button>
        ))}
        <select className="rounded-md border border-input bg-background px-2 py-1.5 text-xs" value={typeFilter} onChange={(event) => setTypeFilter(event.target.value)}>
          <option value="">全部类型</option>
          {Object.entries(typeLabel).map(([key, label]) => <option key={key} value={key}>{label}</option>)}
        </select>
      </div>

      {error ? <p role="alert" className="rounded-lg bg-destructive/10 px-3 py-2 text-sm text-destructive">{error}</p> : null}
      {loading && !tasks.length ? <p className="text-sm text-muted-foreground">正在读取任务...</p> : null}
      {!loading && !visible.length ? <p className="text-sm text-muted-foreground">当前筛选下没有任务。</p> : null}

      <ul className="space-y-2">
        {visible.map((task) => (
          <li key={`${task.taskType}-${task.taskId}`} className="rounded-xl bg-muted/20 px-4 py-3">
            <div className="flex flex-wrap items-start justify-between gap-2">
              <div className="min-w-0">
                <div className="flex flex-wrap items-center gap-2 text-sm font-medium">
                  <span>{task.title}</span>
                  {task.requiresUserAction ? <span className="rounded-full bg-primary/10 px-2 py-0.5 text-[10px] text-primary">需要处理</span> : null}
                </div>
                <p className="mt-1 text-xs text-muted-foreground">
                  {typeLabel[task.taskType] ?? task.taskType} · {statusLabel[task.status] ?? task.status}
                  {task.currentStage ? ` · ${task.currentStage}` : ""} · {new Date(task.updatedAt).toLocaleString("zh-CN")}
                </p>
                {task.lastError ? <p className="mt-1 line-clamp-2 text-xs text-destructive">{task.lastError}</p> : null}
              </div>
              <div className="flex flex-wrap gap-2">
                {task.availableActions.includes("open_approval") ? <Button size="sm" onClick={() => open(task.detailRoute)}>打开确认</Button> : null}
                {task.availableActions.includes("open_batch") ? <Button size="sm" variant="outline" onClick={() => open(task.detailRoute)}>打开连续创作</Button> : null}
                {task.availableActions.includes("pause") ? <Button size="sm" variant="outline" data-testid="task-pause" disabled={busy === task.taskId} onClick={() => void runAction(task, "pause")}>暂停</Button> : null}
                {task.availableActions.includes("resume") ? <Button size="sm" variant="outline" data-testid="task-resume" disabled={busy === task.taskId} onClick={() => void runAction(task, "resume")}>继续</Button> : null}
                {task.availableActions.includes("cancel") ? <Button size="sm" variant="outline" data-testid="task-cancel" disabled={busy === task.taskId} onClick={() => void runAction(task, "cancel")}>取消</Button> : null}
                {task.availableActions.includes("retry") ? <Button size="sm" variant="outline" data-testid="task-retry" disabled={busy === task.taskId} onClick={() => void runAction(task, "retry")}>重试索引</Button> : null}
                {task.availableActions.includes("open_detail") && !task.availableActions.some((action) => action.startsWith("open_") && action !== "open_detail") ? <Button size="sm" variant="outline" onClick={() => open(task.detailRoute)}>查看详情</Button> : null}
              </div>
            </div>
          </li>
        ))}
      </ul>
    </div>
  );
}
