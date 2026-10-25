import { useEffect, useMemo, useState } from "react";
import { Download, FlaskConical } from "lucide-react";
import { Link, useLocation, useNavigate, useParams } from "react-router-dom";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { getChapterEditorWorkspace, getNovelDetail } from "@/api/novel";
import { queryKeys } from "@/api/queryKeys";
import { Button } from "@/components/ui/button";
import { toast } from "@/components/ui/toast";
import { exportChapters } from "@/platform/desktopTransport";
import { stripLegacyTaskUrlParams } from "@/lib/legacyTaskUrlParams";
import ChapterEditorShell from "./components/chapterEditor/ChapterEditorShell";
import NativeChapterHistoryDialog from "./components/NativeChapterHistoryDialog";

function PageStateCard(props: { message: string }) {
  return (
    <div className="rounded-3xl border border-border/70 bg-background p-10 text-center text-sm text-muted-foreground shadow-sm">
      {props.message}
    </div>
  );
}

export default function NovelChapterEdit() {
  const { id = "", chapterId = "" } = useParams();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [historyOpen, setHistoryOpen] = useState(false);
  const location = useLocation();

  useEffect(() => {
    const searchParams = new URLSearchParams(location.search);
    const cleaned = stripLegacyTaskUrlParams(searchParams);
    if (cleaned.toString() !== searchParams.toString()) {
      navigate({
        pathname: location.pathname,
        search: `?${cleaned.toString()}`,
        hash: location.hash,
      }, { replace: true, state: location.state });
    }
  }, [location.hash, location.pathname, location.search, location.state, navigate]);

  const novelDetailQuery = useQuery({
    queryKey: queryKeys.novels.detail(id),
    queryFn: () => getNovelDetail(id),
    enabled: Boolean(id),
  });
  const chapterEditorWorkspaceQuery = useQuery({
    queryKey: queryKeys.novels.chapterEditorWorkspace(id, chapterId || "none"),
    queryFn: () => getChapterEditorWorkspace(id, chapterId),
    enabled: Boolean(id && chapterId),
    staleTime: 5 * 60 * 1000,
    refetchOnWindowFocus: false,
    refetchOnReconnect: false,
  });

  const detail = novelDetailQuery.data?.data;
  const chapter = useMemo(
    () => detail?.chapters.find((item) => item.id === chapterId),
    [chapterId, detail?.chapters],
  );

  if (novelDetailQuery.isLoading && !detail) {
    return (
      <div className="flex h-full min-h-0 flex-col gap-4">
        <PageStateCard message="正在加载章节编辑器..." />
      </div>
    );
  }

  if (novelDetailQuery.isError) {
    return (
      <div className="flex h-full min-h-0 flex-col gap-4">
        <PageStateCard message="章节数据加载失败，请刷新后重试。" />
      </div>
    );
  }

  const handleExportChapter = async () => {
    try {
      const text = await exportChapters({ novelId: id, chapterIds: [chapterId], format: "markdown" });
      const blob = new Blob([text], { type: "text/markdown;charset=utf-8" });
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement("a");
      anchor.href = url;
      anchor.download = `${(chapter?.title ?? "chapter").replace(/[\\/:*?"<>|]/g, "_") || "chapter"}.md`;
      anchor.click();
      URL.revokeObjectURL(url);
      toast.success("本章正文已导出。");
    } catch (cause: unknown) {
      toast.error(cause instanceof Error ? cause.message : "导出失败，请稍后重试。");
    }
  };

  if (!chapter) {
    return (
      <div className="flex h-full min-h-0 flex-col gap-4">
        <PageStateCard message="没有找到对应章节，可能已被删除或当前链接不完整。" />
      </div>
    );
  }

  return (
    <div className="flex h-full min-h-0 flex-col gap-4">
      <div className="flex shrink-0 justify-end gap-2">
        <Button type="button" variant="outline" data-testid="chapter-export-button" onClick={() => { void handleExportChapter(); }}>
          <Download className="mr-2 h-4 w-4" />
          导出本章
        </Button>
        <Button asChild variant="outline">
          <Link to={`/prompt-workbench?experience=writing&novelId=${encodeURIComponent(id)}&chapterId=${encodeURIComponent(chapterId)}`}>
            <FlaskConical className="mr-2 h-4 w-4" />
            正文效果实验室
          </Link>
        </Button>
      </div>
      <ChapterEditorShell
        key={chapter.id}
        novelId={id}
        chapter={chapter}
        workspace={chapterEditorWorkspaceQuery.data?.data ?? null}
        workspaceStatus={chapterEditorWorkspaceQuery.isLoading
          ? "loading"
          : chapterEditorWorkspaceQuery.isError
            ? "error"
            : "ready"}
        onBack={() => navigate(`/novels/${id}/edit`)}
        onOpenVersionHistory={() => setHistoryOpen(true)}
        onRefreshWorkspace={() => { void chapterEditorWorkspaceQuery.refetch(); }}
        isRefreshingWorkspace={chapterEditorWorkspaceQuery.isFetching}
      />
      <NativeChapterHistoryDialog
        novelId={id}
        chapterId={chapterId}
        chapterTitle={chapter.title}
        currentVersion={chapter.officialVersion ?? 0}
        currentContent={chapter.content ?? ""}
        open={historyOpen}
        onOpenChange={setHistoryOpen}
        onRestored={() => {
          void queryClient.invalidateQueries({ queryKey: queryKeys.novels.detail(id) });
          void chapterEditorWorkspaceQuery.refetch();
        }}
      />
    </div>
  );
}
