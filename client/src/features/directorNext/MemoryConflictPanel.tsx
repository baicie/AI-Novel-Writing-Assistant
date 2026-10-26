import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import {
  listMemoryConflicts,
  resolveMemoryConflict,
  type MemoryConflictView,
} from "@/platform/desktopTransport";

export default function MemoryConflictPanel({ novelId }: { novelId: string }) {
  const [conflicts, setConflicts] = useState<MemoryConflictView[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = async () => {
    try {
      setConflicts(await listMemoryConflicts({ novelId }));
      setError(null);
    } catch (cause: unknown) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  };

  useEffect(() => { void refresh(); }, [novelId]);

  const decide = async (conflict: MemoryConflictView, decision: string, reason: string) => {
    setBusy(conflict.id);
    try {
      await resolveMemoryConflict({
        conflictId: conflict.id,
        expectedVersion: conflict.version,
        commandId: `structured-conflict-${Date.now()}`,
        decision,
        chosenCandidateId: conflict.candidateIds[0] ?? null,
        reason,
        operator: "structured-memory-ui",
      });
      await refresh();
    } catch (cause: unknown) {
      setError(cause instanceof Error ? cause.message : "冲突没有处理成功，请刷新后重试。");
      await refresh();
    } finally {
      setBusy(null);
    }
  };

  const open = conflicts.filter((conflict) => conflict.status === "open");
  const resolved = conflicts.filter((conflict) => conflict.status === "resolved");
  return (
    <section className="space-y-2 rounded-xl bg-muted/15 p-3" data-testid="memory-conflict-panel">
      <div className="flex items-center justify-between gap-2">
        <div>
          <h3 className="text-sm font-semibold">记忆冲突</h3>
          <p className="mt-1 text-xs text-muted-foreground">同一人物在相同剧情位置出现不兼容事实时，需要作者决定采用哪一条。</p>
        </div>
        <Button variant="outline" onClick={() => void refresh()}>刷新</Button>
      </div>
      {error ? <p role="alert" className="text-xs text-destructive">{error}</p> : null}
      {open.length ? <ul className="space-y-2">
        {open.map((conflict) => (
          <li key={conflict.id} className="rounded-lg bg-background/60 px-3 py-2 text-xs">
            <p className="leading-5">{conflict.description}</p>
            <p className="mt-1 text-muted-foreground">第{conflict.chapterOrder}章 · 候选 {conflict.candidateIds.length} · 事实 {conflict.factIds.length}</p>
            <div className="mt-2 flex flex-wrap gap-2">
              <Button data-testid="structured-conflict-confirm-new" variant="outline" disabled={busy === conflict.id} onClick={() => void decide(conflict, "confirm_new", "采用新事实")}>采用新事实</Button>
              <Button data-testid="structured-conflict-keep-existing" variant="outline" disabled={busy === conflict.id} onClick={() => void decide(conflict, "keep_existing", "保留现有事实")}>保留现有</Button>
              <Button data-testid="structured-conflict-dispute" variant="outline" disabled={busy === conflict.id} onClick={() => void decide(conflict, "mark_disputed", "标记为争议")}>标记争议</Button>
              <Button data-testid="structured-conflict-reject" variant="outline" disabled={busy === conflict.id} onClick={() => void decide(conflict, "reject_extraction", "拒绝新事实")}>拒绝新事实</Button>
            </div>
          </li>
        ))}
      </ul> : <p className="text-xs text-muted-foreground">当前没有待裁决冲突。</p>}
      {resolved.length ? <p className="text-xs text-muted-foreground">已裁决 {resolved.length} 条，历史仍保留。</p> : null}
    </section>
  );
}
