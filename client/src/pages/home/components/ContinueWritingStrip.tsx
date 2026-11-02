import { ArrowRight, PenLine } from "lucide-react";
import { Link } from "react-router-dom";
import { useMemo } from "react";
import {
  readRecentEditTarget,
  startupEditRoute,
} from "@/pages/novels/components/chapterEditor/document/editPosition";

/**
 * 首页的「继续上次写作」入口。
 *
 * 桌面版重新打开后停在首页，由这里把作者带回上次写作的章节与光标，
 * 而不是启动时强制跳转——那样会打断「先看一眼整体进度」的常见动作。
 * 只在本地有最近编辑记录时出现；记录损坏或缺失时整块不渲染。
 */
export default function ContinueWritingStrip(props: {
  novels: Array<{ id: string; title: string }>;
  /** 首页列表已经加载完成；未完成时先不判断，避免把「还在加载」当成「没有记录」。 */
  loaded?: boolean;
}) {
  const target = useMemo(() => {
    if (typeof window === "undefined") return null;
    if (props.loaded === false) return null;
    const recent = readRecentEditTarget(window.localStorage);
    const route = startupEditRoute({ startRoute: null, recent });
    if (!recent || !route) return null;
    const novel = props.novels.find((item) => item.id === recent.novelId);
    // 列表尚未包含这本书（可能刚创建或列表还没刷新）时，仍然给出入口：
    // 章节页自己会处理「小说或章节已不存在」的兜底。
    return { route, title: novel?.title ?? "上次写作的章节" };
  }, [props.novels, props.loaded]);

  // 没有可恢复的位置时不占位。
  if (!target) return null;

  return (
    <Link
      to={target.route}
      data-testid="home-continue-writing"
      className="group flex items-center justify-between gap-4 rounded-2xl bg-muted/40 px-5 py-4 transition-colors hover:bg-muted/60"
    >
      <div className="flex min-w-0 items-center gap-3">
        <PenLine className="h-4 w-4 shrink-0 text-muted-foreground" aria-hidden="true" />
        <div className="min-w-0">
          <div className="truncate text-sm font-medium text-foreground">继续上次写作</div>
          <div className="mt-0.5 truncate text-xs text-muted-foreground">
            {target.title} · 回到上次停下的位置
          </div>
        </div>
      </div>
      <ArrowRight className="h-4 w-4 shrink-0 text-muted-foreground transition-transform group-hover:translate-x-0.5" aria-hidden="true" />
    </Link>
  );
}
