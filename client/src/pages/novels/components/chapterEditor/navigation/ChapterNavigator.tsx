import { useCallback, useEffect, useMemo, useState } from "react";
import type { Chapter } from "@ai-novel/shared/types/novel";
import { Button } from "@/components/ui/button";
import {
  buildChapterGroups,
  chapterNavigationIntent,
  filterChapters,
  formatWordCount,
  neighbourOf,
  shouldBlockChapterSwitch,
  type ChapterNavVolume,
  type ChapterRunAttention,
} from "./chapterNavigatorModel";

interface ChapterNavigatorProps {
  chapters: Chapter[];
  volumes?: ChapterNavVolume[];
  currentChapterId: string;
  isDirty?: boolean;
  /** 保存中或本地草稿异常时也按「需要确认」处理，避免切换打断写入。 */
  isBusy?: boolean;
  runAttention?: Record<string, ChapterRunAttention>;
  onOpenRunTab?: () => void;
  onSelectChapter: (chapterId: string) => void;
}

/**
 * 整本小说范围内的章节导航：卷/章分组、搜索、上一章/下一章、Run 关注标记。
 * 所有入口（行按钮、待处理徽标、上/下一章、快捷键）都走同一个导航意图与同一套未保存守卫。
 */
export default function ChapterNavigator(props: ChapterNavigatorProps) {
  const {
    chapters,
    volumes = [],
    currentChapterId,
    isDirty = false,
    isBusy = false,
    runAttention = {},
    onOpenRunTab,
    onSelectChapter,
  } = props;
  const [keyword, setKeyword] = useState("");

  const visible = useMemo(() => filterChapters(chapters, keyword), [chapters, keyword]);
  const groups = useMemo(() => buildChapterGroups(visible, volumes), [visible, volumes]);
  const { previous, next } = useMemo(() => neighbourOf(chapters, currentChapterId), [chapters, currentChapterId]);

  const navigateTo = useCallback(
    (targetChapterId: string, openRunTab: boolean) => {
      const intent = chapterNavigationIntent(targetChapterId, currentChapterId, openRunTab);
      if (intent.requiresSwitchGuard && shouldBlockChapterSwitch(isDirty || isBusy, (message) => window.confirm(message))) {
        // 用户取消：章节、页签、草稿、选择全部保持不变。
        return;
      }
      if (intent.requiresSwitchGuard) {
        onSelectChapter(intent.targetChapterId);
      }
      if (intent.openRunTab) {
        onOpenRunTab?.();
      }
    },
    [currentChapterId, isBusy, isDirty, onOpenRunTab, onSelectChapter],
  );

  // 键盘导航：Alt+←/→ 切到上一章/下一章，和按钮走同一套保护逻辑。
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (!event.altKey || event.metaKey || event.ctrlKey) return;
      if (event.key === "ArrowLeft" && previous) {
        event.preventDefault();
        navigateTo(previous.id, false);
      }
      if (event.key === "ArrowRight" && next) {
        event.preventDefault();
        navigateTo(next.id, false);
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [navigateTo, next, previous]);

  return (
    <div className="flex flex-col gap-2" data-testid="chapter-navigator">
      <div className="flex items-center justify-between gap-2">
        <Button
          size="sm"
          variant="outline"
          disabled={!previous}
          onClick={() => previous && navigateTo(previous.id, false)}
          data-testid="chapter-nav-previous"
        >
          上一章
        </Button>
        <span className="text-xs text-muted-foreground">共 {chapters.length} 章</span>
        <Button
          size="sm"
          variant="outline"
          disabled={!next}
          onClick={() => next && navigateTo(next.id, false)}
          data-testid="chapter-nav-next"
        >
          下一章
        </Button>
      </div>

      <input
        className="w-full rounded-xl bg-muted/50 px-3 py-2 text-sm outline-none placeholder:text-muted-foreground focus-visible:ring-1 focus-visible:ring-ring"
        placeholder="搜索章节标题或序号"
        value={keyword}
        onChange={(event) => setKeyword(event.target.value)}
        data-testid="chapter-nav-search"
      />

      <div className="max-h-[280px] space-y-3 overflow-y-auto pr-1">
        {groups.length === 0 ? (
          <div className="px-2 py-3 text-xs text-muted-foreground">没有匹配的章节</div>
        ) : (
          groups.map((group) => (
            <div key={group.key} className="space-y-1">
              <div className="px-1 text-xs font-medium text-muted-foreground">{group.label}</div>
              <ul className="space-y-1">
                {group.chapters.map((item) => {
                  const isCurrent = item.id === currentChapterId;
                  const attention = runAttention[item.id];
                  return (
                    <li key={item.id} data-chapter-id={item.id} className="flex items-stretch gap-1">
                      <button
                        type="button"
                        onClick={() => navigateTo(item.id, false)}
                        data-testid="chapter-nav-item"
                        data-current={String(isCurrent)}
                        aria-current={isCurrent ? "true" : undefined}
                        className={`flex min-w-0 flex-1 items-center justify-between gap-2 rounded-lg px-2 py-1.5 text-left text-sm transition-colors ${
                          isCurrent ? "bg-muted font-medium text-foreground" : "text-muted-foreground hover:bg-muted/60"
                        }`}
                      >
                        <span className="truncate">
                          第 {item.order} 章 · {item.title}
                        </span>
                        <span className="shrink-0 text-[11px] text-muted-foreground">
                          {item.hasContent ? `v${item.officialVersion}` : "空"}
                          {item.hasContent ? ` · ${formatWordCount(item.wordCount)}` : ""}
                        </span>
                      </button>
                      {attention === "action" ? (
                        <button
                          type="button"
                          onClick={() => navigateTo(item.id, true)}
                          data-testid="chapter-run-action"
                          data-chapter-id={item.id}
                          aria-label={`第 ${item.order} 章有待处理的运行，打开运行与恢复`}
                          className="shrink-0 rounded-full bg-amber-500/20 px-2 py-0.5 text-[11px] text-amber-700 hover:bg-amber-500/30 dark:text-amber-300"
                        >
                          待处理
                        </button>
                      ) : attention === "running" ? (
                        <span
                          className="shrink-0 self-center rounded-full bg-muted px-2 py-0.5 text-[11px] text-muted-foreground"
                          data-testid="chapter-run-running"
                        >
                          生成中
                        </span>
                      ) : null}
                    </li>
                  );
                })}
              </ul>
            </div>
          ))
        )}
      </div>
    </div>
  );
}
