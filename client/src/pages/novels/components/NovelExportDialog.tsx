import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { toast } from "@/components/ui/toast";
import { exportNovelToFile, listVolumes, type VolumeSnapshot } from "@/platform/desktopTransport";

interface NovelExportDialogProps {
  novelId: string;
  chapterCount: number;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export default function NovelExportDialog(props: NovelExportDialogProps) {
  const { novelId, chapterCount, open, onOpenChange } = props;
  const [selection, setSelection] = useState<"entire_book" | "range" | "volumes">("entire_book");
  const [volumes, setVolumes] = useState<VolumeSnapshot[]>([]);
  const [volumeIds, setVolumeIds] = useState<string[]>([]);
  const [format, setFormat] = useState<"markdown" | "txt">("markdown");
  const [startOrder, setStartOrder] = useState(1);
  const [endOrder, setEndOrder] = useState(Math.max(1, chapterCount));
  const [exporting, setExporting] = useState(false);

  useEffect(() => {
    if (!open) return;
    listVolumes(novelId).then((rows) => {
      setVolumes(rows);
      setVolumeIds((current) => current.length ? current : rows.map((row) => row.id));
    }).catch(() => setVolumes([]));
  }, [open, novelId]);

  const exportNovel = async () => {
    setExporting(true);
    try {
      const saved = await exportNovelToFile({
        novelId, selection, format,
        startOrder: selection === "range" ? startOrder : undefined,
        endOrder: selection === "range" ? endOrder : undefined,
        volumeIds: selection === "volumes" ? volumeIds : undefined,
      });
      if (!saved) {
        toast.info("已取消保存。");
        return;
      }
      toast.success(`已保存到 ${saved}`);
      onOpenChange(false);
    } catch (cause: unknown) {
      toast.error(cause instanceof Error ? cause.message : "导出失败，请稍后重试。");
    } finally {
      setExporting(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>导出小说</DialogTitle>
          <DialogDescription>只导出已经确认的正式正文，不会包含未确认候选或本地草稿。</DialogDescription>
        </DialogHeader>
        <div className="space-y-4 text-sm">
          <label className="block space-y-1">
            <span className="text-muted-foreground">导出范围</span>
            <select className="w-full rounded-lg border bg-background px-3 py-2" value={selection} onChange={(event) => setSelection(event.target.value as "entire_book" | "range" | "volumes")} data-testid="export-selection">
              <option value="entire_book">全书（{chapterCount} 章）</option>
              <option value="range">指定章节范围</option>
              <option value="volumes">按卷导出</option>
            </select>
          </label>
          {selection === "volumes" ? (
            <div className="space-y-1 rounded-lg bg-muted/20 p-2">
              {volumes.length === 0 ? <p className="text-xs text-muted-foreground">这本书还没有分卷。</p> : volumes.map((volume) => (
                <label key={volume.id} className="flex items-center gap-2 text-xs">
                  <input
                    type="checkbox"
                    checked={volumeIds.includes(volume.id)}
                    onChange={(event) => setVolumeIds((current) => event.target.checked
                      ? [...current, volume.id]
                      : current.filter((id) => id !== volume.id))}
                  />
                  <span>{volume.title || `第${volume.sortOrder}卷`}</span>
                </label>
              ))}
            </div>
          ) : null}
          {selection === "range" ? (
            <div className="grid grid-cols-2 gap-3">
              <label className="space-y-1">
                <span className="text-muted-foreground">起始章节序号</span>
                <input className="w-full rounded-lg border bg-background px-3 py-2" type="number" min={1} value={startOrder} onChange={(event) => setStartOrder(Number(event.target.value))} />
              </label>
              <label className="space-y-1">
                <span className="text-muted-foreground">结束章节序号</span>
                <input className="w-full rounded-lg border bg-background px-3 py-2" type="number" min={1} value={endOrder} onChange={(event) => setEndOrder(Number(event.target.value))} />
              </label>
            </div>
          ) : null}
          <label className="block space-y-1">
            <span className="text-muted-foreground">文件格式</span>
            <select className="w-full rounded-lg border bg-background px-3 py-2" value={format} onChange={(event) => setFormat(event.target.value as "markdown" | "txt")} data-testid="export-format">
              <option value="markdown">Markdown</option>
              <option value="txt">UTF-8 TXT</option>
            </select>
          </label>
        </div>
        <div className="flex justify-end gap-2">
          <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>取消</Button>
          <Button type="button" disabled={exporting} onClick={() => { void exportNovel(); }} data-testid="export-confirm">
            {exporting ? "导出中..." : "生成导出文件"}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
