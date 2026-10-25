import { useEffect, useMemo, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import {
  cancelBatch,
  createBatch,
  getBatch,
  listBatches,
  pauseBatch,
  resumeBatch,
  type BatchRunView,
} from "@/platform/desktopTransport";

interface BatchWorkspaceProps {
  novelId: string;
  chapters: Array<{ id: string; title: string; sortOrder: number }>;
  model: { model: string; endpoint: string; allowLocalhostHttp: boolean; hasApiKey: boolean } | undefined;
  onOpenChapter: (chapterId: string) => void;
}

const statusLabel: Record<string, string> = {
  created: "已创建", running: "进行中", waiting_approval: "等待确认",
  paused: "已暂停", cancelled: "已取消", failed: "失败", completed: "已完成",
};

export default function BatchWorkspace({ novelId, chapters, model, onOpenChapter }: BatchWorkspaceProps) {
  const [selected, setSelected] = useState<string[]>([]);
  const [batches, setBatches] = useState<BatchRunView[]>([]);
  const [batchId, setBatchId] = useState("");
  const [busy, setBusy] = useState(false);
  const createCommandId = useRef<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setSelected((current) => {
      const valid = current.filter((id) => chapters.some((chapter) => chapter.id === id));
      return valid.length ? valid : chapters.slice(0, 3).map((chapter) => chapter.id);
    });
  }, [chapters]);

  useEffect(() => {
    let disposed = false;
    const load = async () => {
      try {
        const rows = await listBatches(novelId);
        if (disposed) return;
        setBatches(rows);
        setBatchId((current) => current && rows.some((row) => row.id === current) ? current : rows[0]?.id ?? "");
      } catch (cause: unknown) {
        if (!disposed) setError(cause instanceof Error ? cause.message : "批量任务读取失败。");
      }
    };
    void load();
    const timer = window.setInterval(load, 1500);
    return () => { disposed = true; window.clearInterval(timer); };
  }, [novelId]);

  const batch = useMemo(() => batches.find((row) => row.id === batchId) ?? null, [batches, batchId]);
  const chapterTitle = (chapterId: string) => chapters.find((chapter) => chapter.id === chapterId)?.title ?? chapterId;
  const canStart = selected.length >= 2 && Boolean(model?.hasApiKey) && !busy;

  const run = async (action: () => Promise<unknown>) => {
    setBusy(true);
    setError(null);
    try {
      const result = await action();
      if (result && typeof result === "object" && "id" in result) {
        setBatchId(String((result as { id: string }).id));
      }
      setBatches(await listBatches(novelId));
    } catch (cause: unknown) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  };

  const create = () => run(async () => {
    if (!model) throw new Error("请先配置模型。");
    createCommandId.current ??= `batch-create-${crypto.randomUUID()}`;
    const ordered = [...selected].sort((left, right) =>
      (chapters.find((chapter) => chapter.id === left)?.sortOrder ?? 0)
      - (chapters.find((chapter) => chapter.id === right)?.sortOrder ?? 0));
    const created = await createBatch({
      commandId: createCommandId.current,
      novelId,
      chapterIds: ordered,
      provider: "openai-compatible",
      model: model.model,
      promptVersions: { chapter_draft: "chapter-draft-v2" },
      tokenBudget: null,
      maxAutomaticRetries: 0,
      repairBudget: 0,
      endpoint: model.endpoint,
      allowLocalhostHttp: model.allowLocalhostHttp,
    });
    createCommandId.current = null;
    return created;
  });

  return (
    <section data-testid="batch-workspace" className="space-y-3 border-t border-border/50 pt-4 text-sm">
      <div className="flex flex-wrap items-start justify-between gap-2">
        <div>
          <h2 className="text-sm font-semibold">连续章节创作</h2>
          <p className="mt-1 text-xs text-muted-foreground">逐章生成并等待人工确认；前一章形成正式正文后才开始下一章。</p>
        </div>
        <Button data-testid="batch-refresh" variant="outline" onClick={() => void run(async () => (await listBatches(novelId)))} disabled={busy}>刷新</Button>
      </div>

      {error ? <p data-testid="batch-error" className="rounded-md bg-destructive/10 px-3 py-2 text-xs text-destructive">{error}</p> : null}

      <div data-testid="batch-chapter-picker" className="max-h-44 space-y-1 overflow-y-auto rounded-md bg-background/60 p-2">
        {chapters.map((chapter) => (
          <label key={chapter.id} className="flex items-center gap-2 text-xs">
            <input
              type="checkbox"
              checked={selected.includes(chapter.id)}
              onChange={(event) => setSelected((current) => event.target.checked
                ? [...current, chapter.id]
                : current.filter((id) => id !== chapter.id))}
            />
            <span>{chapter.sortOrder}. {chapter.title}</span>
          </label>
        ))}
      </div>

      <div className="flex flex-wrap gap-2">
        <Button data-testid="batch-create" disabled={!canStart} onClick={() => void create()}>
          {busy ? "处理中…" : `开始连续创作（${selected.length} 章）`}
        </Button>
        {batch ? (
          <>
            <Button data-testid="batch-pause" variant="outline" disabled={busy || !["created", "running", "waiting_approval"].includes(batch.status)} onClick={() => void run(async () => pauseBatch(batch.id, (await getBatch(batch.id)).version))}>暂停</Button>
            <Button data-testid="batch-resume" variant="outline" disabled={busy || batch.status !== "paused"} onClick={() => void run(async () => resumeBatch(batch.id, (await getBatch(batch.id)).version))}>继续</Button>
            <Button data-testid="batch-cancel" variant="outline" disabled={busy || ["cancelled", "completed", "failed"].includes(batch.status)} onClick={() => void run(async () => cancelBatch(batch.id, (await getBatch(batch.id)).version))}>取消</Button>
          </>
        ) : null}
      </div>

      {batch ? (
        <div data-testid="batch-current" className="space-y-2 rounded-md bg-muted/20 p-3 text-xs">
          <p className="font-medium">批量状态：{statusLabel[batch.status] ?? batch.status} · 已完成 {batch.items.filter((item) => item.status === "succeeded").length}/{batch.items.length}</p>
          {batch.failureReason ? <p className="text-destructive">最近错误：{batch.failureReason}</p> : null}
          <ul className="space-y-1">
            {batch.items.map((item) => (
              <li key={item.id} className="flex flex-wrap items-center justify-between gap-2 rounded-md bg-background/60 px-2 py-1.5">
                <span>{item.ordinal + 1}. {chapterTitle(item.chapterId)} · {statusLabel[item.status] ?? item.status}</span>
                <span className="flex items-center gap-2 text-muted-foreground">
                  {item.runId ? <code className="text-[10px]">{item.runId.slice(0, 8)}</code> : null}
                  {item.lastError ? <span className="text-destructive">{item.lastError}</span> : null}
                  <Button variant="ghost" onClick={() => onOpenChapter(item.chapterId)}>打开章节</Button>
                </span>
              </li>
            ))}
          </ul>
        </div>
      ) : <p className="text-xs text-muted-foreground">还没有批量任务。选择至少两个章节开始。</p>}

      {batches.length > 1 ? (
        <select className="w-full rounded-md border border-input bg-background p-2 text-xs" value={batchId} onChange={(event) => setBatchId(event.target.value)}>
          {batches.map((row) => <option key={row.id} value={row.id}>{statusLabel[row.status] ?? row.status} · {new Date(row.createdAt).toLocaleString("zh-CN")}</option>)}
        </select>
      ) : null}
    </section>
  );
}
