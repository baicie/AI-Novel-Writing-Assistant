import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { toast } from "@/components/ui/toast";
import { exportNovelSelection } from "@/platform/desktopTransport";

interface NovelExportDialogProps {
  novelId: string;
  chapterCount: number;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export default function NovelExportDialog(props: NovelExportDialogProps) {
  const { novelId, chapterCount, open, onOpenChange } = props;
  const [selection, setSelection] = useState<"entire_book" | "range">("entire_book");
  const [format, setFormat] = useState<"markdown" | "txt">("markdown");
  const [startOrder, setStartOrder] = useState(1);
  const [endOrder, setEndOrder] = useState(Math.max(1, chapterCount));
  const [exporting, setExporting] = useState(false);

  const exportNovel = async () => {
    setExporting(true);
    try {
      const text = await exportNovelSelection({
        novelId, selection, format,
        startOrder: selection === "range" ? startOrder : undefined,
        endOrder: selection === "range" ? endOrder : undefined,
      });
      const blob = new Blob([text], { type: format === "markdown" ? "text/markdown;charset=utf-8" : "text/plain;charset=utf-8" });
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement("a");
      anchor.href = url;
      anchor.download = `novel-export-${novelId}.${format === "markdown" ? "md" : "txt"}`;
      anchor.click();
      URL.revokeObjectURL(url);
      toast.success("导出文件已生成。");
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
            <select className="w-full rounded-lg border bg-background px-3 py-2" value={selection} onChange={(event) => setSelection(event.target.value as "entire_book" | "range")} data-testid="export-selection">
              <option value="entire_book">全书（{chapterCount} 章）</option>
              <option value="range">指定章节范围</option>
            </select>
          </label>
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
