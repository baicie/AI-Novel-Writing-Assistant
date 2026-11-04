import { useQuery } from "@tanstack/react-query";
import { queryKeys } from "@/api/queryKeys";
import { Button } from "@/components/ui/button";
import { previewChapterContext } from "@/platform/desktopTransport";

interface ContextPreviewTabProps {
  novelId: string;
  chapterId: string;
}

/**
 * 「上下文」：下一次创作会引用哪些来源。
 *
 * 只读 trace，不生成 Prompt 正文、不发送模型请求；必需来源缺失时明确阻断。
 */
export default function ContextPreviewTab(props: ContextPreviewTabProps) {
  const { novelId, chapterId } = props;
  const previewQuery = useQuery({
    queryKey: queryKeys.novels.chapterContextPreview(novelId, chapterId),
    queryFn: () => previewChapterContext({ novelId, chapterId }),
    enabled: Boolean(novelId && chapterId),
  });

  if (previewQuery.isLoading) {
    return <div className="px-1 py-4 text-sm text-muted-foreground" data-testid="context-preview-loading">正在核对本次创作将引用的上下文...</div>;
  }
  if (previewQuery.isError) {
    return (
      <div className="space-y-2 px-1 py-4 text-sm text-muted-foreground" data-testid="context-preview-error">
        <div>上下文读取失败；这不是「没有相关资料」，请检查本地数据后重试。</div>
        <Button size="sm" variant="outline" onClick={() => void previewQuery.refetch()}>重试</Button>
      </div>
    );
  }
  const preview = previewQuery.data;
  if (!preview) {
    return <div className="px-1 py-4 text-sm text-muted-foreground">暂无可预览的上下文。</div>;
  }
  const { trace, blockedReasons } = preview;
  const selected = trace.sources.filter((source) => source.selected);
  const excluded = trace.sources.filter((source) => !source.selected);

  return (
    <div className="flex flex-col gap-3 px-1 py-2" data-testid="context-preview">
      <div className="text-xs text-muted-foreground">
        上下文版本 {trace.assemblyVersion} · 预算 {trace.inputBudget} · 预计使用 {trace.estimatedTokens} tokens ·
        输出预留 {trace.outputReserve}
      </div>
      <div className="text-[11px] text-muted-foreground">
        目标章节 {trace.targetChapterId} · 指纹 {trace.contentHash.slice(0, 12)}
      </div>

      {blockedReasons.length > 0 ? (
        <div className="rounded-xl bg-rose-50 px-3 py-2 text-xs leading-5 text-rose-950" data-testid="context-blocked">
          本次生成被阻断：
          <ul className="mt-1 list-disc pl-4">
            {blockedReasons.map((reason) => <li key={reason}>{reason}</li>)}
          </ul>
        </div>
      ) : (
        <div className="rounded-xl bg-emerald-50 px-3 py-2 text-xs text-emerald-900" data-testid="context-ready">
          必要来源齐备，可以生成。
        </div>
      )}

      <section className="space-y-2">
        <h3 className="text-sm font-medium text-foreground">将引用的来源（{selected.length}）</h3>
        {selected.length === 0 ? (
          <div className="text-xs text-muted-foreground">没有来源被选中。</div>
        ) : (
          <ul className="space-y-2">
            {selected.map((source) => (
              <li key={`${source.sourceType}:${source.sourceId}`} className="rounded-xl bg-muted/30 px-3 py-2" data-testid="context-source">
                <div className="flex flex-wrap items-center gap-2 text-xs">
                  <span className="rounded-full bg-background px-2 py-0.5 text-[11px] text-muted-foreground">{source.sourceType}</span>
                  <span className="text-foreground">{source.sourceId || "（无编号）"}</span>
                  <span className="text-[11px] text-muted-foreground">版本 {source.sourceVersion ?? "未知"}</span>
                  <span className="text-[11px] text-muted-foreground">约 {source.estimatedTokens} tokens</span>
                </div>
                <div className="mt-1 text-[11px] text-muted-foreground">
                  状态 {source.sourceStatus}
                  {source.contentHash ? ` · 指纹 ${source.contentHash.slice(0, 12)}` : ""}
                </div>
              </li>
            ))}
          </ul>
        )}
      </section>

      {excluded.length > 0 ? (
        <section className="space-y-2">
          <h3 className="text-sm font-medium text-foreground">未使用或缺失的来源（{excluded.length}）</h3>
          <ul className="space-y-2">
            {excluded.map((source) => (
              <li key={`${source.sourceType}:${source.sourceId}`} className="rounded-xl bg-muted/20 px-3 py-2" data-testid="context-source-excluded">
                <div className="flex flex-wrap items-center gap-2 text-xs">
                  <span className="rounded-full bg-background px-2 py-0.5 text-[11px] text-muted-foreground">{source.sourceType}</span>
                  <span className="text-foreground">{source.sourceId || "（无编号）"}</span>
                  <span className="text-[11px] text-amber-800">{source.exclusionReason ?? source.sourceStatus}</span>
                </div>
                <div className="mt-1 text-[11px] text-muted-foreground">
                  版本 {source.sourceVersion ?? "未知"} · 约 {source.estimatedTokens} tokens
                </div>
              </li>
            ))}
          </ul>
        </section>
      ) : null}

      <div className="text-[11px] leading-5 text-muted-foreground">
        预览只读取本地已确认来源，不会调用模型，也不会把整本正文写进日志。
      </div>
    </div>
  );
}
