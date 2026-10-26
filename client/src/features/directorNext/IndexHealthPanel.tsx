import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { listIndexJobs, retryIndexJob, type IndexJobView } from "@/platform/desktopTransport";

export default function IndexHealthPanel({ novelId }: { novelId: string }) {
  const [jobs, setJobs] = useState<IndexJobView[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = async () => {
    try {
      setJobs(await listIndexJobs({ novelId }));
      setError(null);
    } catch (cause: unknown) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  };

  useEffect(() => {
    void refresh();
    const timer = window.setInterval(() => { void refresh(); }, 4000);
    return () => window.clearInterval(timer);
  }, [novelId]);

  const retry = async (job: IndexJobView) => {
    setBusy(job.id);
    setError(null);
    try {
      await retryIndexJob({ jobId: job.id });
      await refresh();
    } catch (cause: unknown) {
      setError(cause instanceof Error ? cause.message : "索引任务没有恢复成功，请稍后重试。");
      await refresh();
    } finally {
      setBusy(null);
    }
  };

  return (
    <section className="space-y-2 rounded-xl bg-muted/15 p-3" data-testid="index-health-panel">
      <div className="flex items-center justify-between gap-2">
        <div>
          <h3 className="text-sm font-semibold">知识索引</h3>
          <p className="mt-1 text-xs text-muted-foreground">查看索引任务状态；失败后可从原始知识页面重建，已保存的状态与错误会保留。</p>
        </div>
        <Button variant="outline" onClick={() => void refresh()}>刷新</Button>
      </div>
      {error ? <p role="alert" className="text-xs text-destructive">{error}</p> : null}
      {jobs.length ? <ul className="space-y-1">
        {jobs.slice(0, 20).map((job) => (
          <li key={job.id} className="flex flex-wrap items-center justify-between gap-2 rounded-lg bg-background/60 px-3 py-2 text-xs">
            <span className="min-w-0">
              <span className="font-medium">{job.status}</span> · {job.sourceType}:{job.sourceId.slice(0, 16)} · 尝试 {job.attemptCount}/{job.maxAttempts}
              {job.lastError ? <span className="ml-1 text-destructive">{job.lastError}</span> : null}
            </span>
            {["failed", "retry_wait", "obsolete", "pending"].includes(job.status) ? (
              <Button data-testid="native-retry-index-job" variant="outline" disabled={busy === job.id} onClick={() => void retry(job)}>
                {busy === job.id ? "恢复中..." : "重试索引"}
              </Button>
            ) : null}
          </li>
        ))}
      </ul> : <p className="text-xs text-muted-foreground">当前没有索引任务。</p>}
    </section>
  );
}
