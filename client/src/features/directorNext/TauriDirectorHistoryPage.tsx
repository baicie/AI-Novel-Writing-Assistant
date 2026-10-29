import { useQuery } from "@tanstack/react-query";
import { Link } from "react-router-dom";
import { Button } from "@/components/ui/button";
import { listAllRuns } from "@/platform/desktopTransport";

const labels: Record<string, string> = {
  queued: "排队中",
  running: "生成中",
  waiting_approval: "等待确认",
  paused: "已暂停",
  succeeded: "已完成",
  failed: "失败",
  cancelled: "已取消",
};

export default function TauriDirectorHistoryPage() {
  const runs = useQuery({ queryKey: ["tauri-director", "all-runs"], queryFn: listAllRuns, refetchInterval: 5000 });
  return (
    <div className="mx-auto w-full max-w-4xl space-y-4 p-4">
      <header className="flex flex-wrap items-center justify-between gap-3 border-b border-border/60 pb-3">
        <div>
          <h1 className="text-lg font-semibold">运行记录</h1>
          <p className="mt-1 text-xs text-muted-foreground">这里只查看历史状态和产物，不在这里执行确认、恢复或取消。</p>
        </div>
        <Button asChild variant="ghost"><Link to="/novels">返回书架</Link></Button>
      </header>
      {runs.isError ? <p role="alert" className="text-sm text-destructive">运行记录读取失败。</p> : null}
      <div className="divide-y divide-border/50">
        {(runs.data ?? []).map(run => (
          <Link key={run.id} to={`/lab/director/${encodeURIComponent(run.novelId)}`} className="flex flex-wrap items-center justify-between gap-3 py-3 text-sm hover:bg-muted/30">
            <span className="min-w-0 flex-1 truncate">{run.chapterTitle || "未命名章节"}</span>
            <span className="text-xs text-muted-foreground">{labels[run.state] ?? run.state}</span>
            <span className="text-xs text-muted-foreground">{new Date(run.updatedAt).toLocaleString()}</span>
          </Link>
        ))}
      </div>
      {!runs.isLoading && (runs.data ?? []).length === 0 ? <p className="py-8 text-sm text-muted-foreground">还没有运行记录。</p> : null}
    </div>
  );
}
