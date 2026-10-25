import { useEffect, useMemo, useState } from "react";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { toast } from "@/components/ui/toast";
import {
  listChapterRevisions,
  restoreChapterRevision,
  type ChapterRevisionSnapshot,
  type ChapterSnapshot,
} from "@/platform/desktopTransport";

interface NativeChapterHistoryDialogProps {
  novelId: string;
  chapterId: string;
  chapterTitle: string;
  currentVersion: number;
  currentContent: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onRestored: (chapter: ChapterSnapshot) => void;
}

function formatTime(value: number): string {
  if (!value) return "时间未知";
  return new Intl.DateTimeFormat("zh-CN", {
    month: "numeric", day: "numeric", hour: "2-digit", minute: "2-digit",
  }).format(new Date(value));
}

function originLabel(origin: string): string {
  if (origin === "ai_candidate") return "AI 候选确认";
  if (origin === "promoted") return "候选提交";
  return "作者正文";
}

export default function NativeChapterHistoryDialog(props: NativeChapterHistoryDialogProps) {
  const { novelId, chapterId, chapterTitle, currentVersion, currentContent, open, onOpenChange, onRestored } = props;
  const [revisions, setRevisions] = useState<ChapterRevisionSnapshot[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [restoring, setRestoring] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setLoading(true);
    setError(null);
    listChapterRevisions({ novelId, chapterId })
      .then((rows) => {
        setRevisions(rows);
        setSelectedId((current) => current && rows.some((row) => row.id === current) ? current : rows[0]?.id ?? null);
      })
      .catch((cause: unknown) => setError(cause instanceof Error ? cause.message : "版本历史读取失败。"))
      .finally(() => setLoading(false));
  }, [open, novelId, chapterId]);

  const selected = useMemo(
    () => revisions.find((revision) => revision.id === selectedId) ?? null,
    [revisions, selectedId],
  );

  const restore = async () => {
    if (!selected) return;
    if (!confirming) {
      setConfirming(true);
      return;
    }
    setRestoring(true);
    setError(null);
    try {
      const chapter = await restoreChapterRevision({
        novelId, chapterId, revisionId: selected.id, expectedVersion: currentVersion,
      });
      toast.success(`已恢复为新的正式版本 v${chapter.officialVersion}。`);
      onRestored(chapter);
      setConfirming(false);
      onOpenChange(false);
    } catch (cause: unknown) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setRestoring(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[86vh] max-w-4xl overflow-hidden">
        <DialogHeader>
          <DialogTitle>本章历史</DialogTitle>
          <DialogDescription>
            查看「{chapterTitle}」保存过的正式正文。恢复旧版本会生成新的正式版本，历史记录会保留。
          </DialogDescription>
        </DialogHeader>
        {loading ? <p className="py-8 text-sm text-muted-foreground">正在读取版本历史...</p> : null}
        {error ? <p className="rounded-lg bg-destructive/10 px-3 py-2 text-sm text-destructive">{error}</p> : null}
        <div className="grid min-h-0 gap-4 md:grid-cols-[240px_minmax(0,1fr)]">
          <div className="max-h-[52vh] space-y-1 overflow-y-auto">
            {revisions.length === 0 ? (
              <p className="py-6 text-sm text-muted-foreground">还没有正式正文版本。</p>
            ) : revisions.map((revision) => (
              <button
                key={revision.id}
                type="button"
                data-testid={`chapter-history-version-${revision.version}`}
                onClick={() => { setSelectedId(revision.id); setConfirming(false); }}
                className={`w-full rounded-xl px-3 py-3 text-left transition ${revision.id === selectedId ? "bg-muted text-foreground" : "text-muted-foreground hover:bg-muted/60 hover:text-foreground"}`}
              >
                <div className="text-sm font-medium">v{revision.version} · {originLabel(revision.origin)}</div>
                <div className="mt-1 text-xs">{formatTime(revision.createdAt)} · {revision.content.length.toLocaleString()} 字</div>
              </button>
            ))}
          </div>
          <div className="min-h-0">
            {selected ? (
              <div className="flex h-full flex-col gap-3">
                <div className="flex flex-wrap items-center justify-between gap-2 text-xs text-muted-foreground">
                  <span>当前正式版本 v{currentVersion} · {currentContent.length.toLocaleString()} 字</span>
                  <span>查看 v{selected.version}</span>
                </div>
                <pre className="max-h-[46vh] min-h-[240px] overflow-y-auto whitespace-pre-wrap break-words rounded-xl bg-muted/20 p-4 text-sm leading-7">
                  {selected.content || "（这一版正文为空）"}
                </pre>
                <div className="flex justify-end">
                  <Button type="button" data-testid="chapter-history-restore" disabled={restoring} onClick={() => { void restore(); }}>
                    {restoring ? "恢复中..." : confirming ? `确认恢复 v${selected.version}` : `恢复 v${selected.version} 为正式正文`}
                  </Button>
                </div>
              </div>
            ) : null}
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
