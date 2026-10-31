import { useCallback, useEffect, useMemo, useState } from "react";
import type { Chapter } from "@ai-novel/shared/types/novel";
import { Button } from "@/components/ui/button";
import {
  buildChapterGroups,
  filterChapters,
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
  runAttention?: Record<string, ChapterRunAttention>;
  onOpenRunTab?: () => void;
  onSelectChapter: (chapterId: string) => void;
}

/**
 * 整本小说范围内的章节导航：卷/章分组、搜索、上一章/下一章。
 * 只消费已经加载好的章节数据，不自己编造进度。
 */
export default function ChapterNavigator(props: ChapterNavigatorProps) {
  const {
    chapters,
    volumes = [],
    currentChapterId,
    isDirty = false,
    runAttention = {},
    onOpenRunTab,
    onSelectChapter,
  } = props;
  const [keyword, setKeyword] = useState("");

  const visible = useMemo(() => filterChapters(chapters, keyword), [chapters, keyword]);
  const groups = useMemo(() => buildChapterGroups(visible, volumes), [visible, volumes]);
  const { previous, next } = useMemo(() => neighbourOf(chapters, currentChapterId), [chapters, currentChapterId]);

  const switchTo = useCallback(
    (chapterId: string) => {
      if (chapterId === currentChapterId) return;
      if (shouldBlockChapterSwitch(isDirty, (message) => window.confirm(message))) {
        return;
      }
      onSelectChapter(chapterId);
    },
    [currentChapterId, isDirty, onSelectChapter],
  );

  // 键盘导航：Alt+←/→ 切到上一章/下一章，和按钮走同一套保护逻辑。
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (!event.altKey || event.metaKey || event.ctrlKey) return;
      if (event.key === "ArrowLeft" && previous) {
        event.preventDefault();
        switchTo(previous.id);
      }
      if (event.key === "ArrowRight" && next) {
        event.preventDefault();
        switchTo(next.id);
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [next, previous, switchTo]);

  return (
    <div className="flex flex-col gap-2" data-testid="chapter-navigator">
      <div className="flex items-center justify-between gap-2">
        <Button
          size="sm"
          variant="outline"
          disabled={!previous}
          onClick={() => previous && switchTo(previous.id)}
          data-testid="chapter-nav-previous"
        >
          上一章
        </Button>
        <span className="text-xs text-muted-foreground">
          共 {chapters.length} 章
        </span>
        <Button
          size="sm"
          variant="outline"
          disabled={!next}
          onClick={() => next && switchTo(next.id)}
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
              {group.chapters.map((chapter) => {
                const item = chapter as Chapter & { hasContent?: boolean; officialVersion?: number };
                const isCurrent = chapter.id === currentChapterId;
                return (
                  <button
                    key={chapter.id}
                    type="button"
                    onClick={() => switchTo(chapter.id)}
                    data-testid="chapter-nav-item"
                    data-current={String(isCurrent)}
                    className={`flex w-full items-center justify-between gap-2 rounded-lg px-2 py-1.5 text-left text-sm transition-colors ${
                      isCurrent ? "bg-muted font-medium text-foreground" : "text-muted-foreground hover:bg-muted/60"
                    }`}
                  >
                    <span className="truncate">
                      第 {chapter.order} 章 · {chapter.title?.trim() || "未命名章节"}
                    </span>
                    <span className="flex shrink-0 items-center gap-1 text-[11px] text-muted-foreground">
                      {runAttention[chapter.id] === "action" ? (
                        <span
                          role="button"
                          tabIndex={0}
                          onClick={(event) => {
                            event.stopPropagation();
                            switchTo(chapter.id);
                            onOpenRunTab?.();
                          }}
                          onKeyDown={(event) => {
                            if (event.key !== "Enter" && event.key !== " ") return;
                            event.preventDefault();
                            event.stopPropagation();
                            switchTo(chapter.id);
                            onOpenRunTab?.();
                          }}
                          className="rounded-full bg-amber-500/20 px-1.5 py-0.5 text-amber-700 dark:text-amber-300"
                          data-testid="chapter-run-action"
                        >
                          待处理
                        </span>
                      ) : runAttention[chapter.id] === "running" ? (
                        <span className="rounded-full bg-muted px-1.5 py-0.5" data-testid="chapter-run-running">
                          生成中
                        </span>
                      ) : null}
                      {item.hasContent ? `v${item.officialVersion ?? 0}` : "空"}
                    </span>
                  </button>
                );
              })}
            </div>
          ))
        )}
      </div>
    </div>
  );
}
