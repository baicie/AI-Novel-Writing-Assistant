import type { ReactNode } from "react";
import type { ChapterEditorDiagnosticCard, ChapterEditorWorkspaceResponse } from "@ai-novel/shared/types/novel";
import CastWorldTab from "./CastWorldTab";
import ContextPreviewTab from "./ContextPreviewTab";
import type { CastItem, PanelTab } from "./panelModel";
import DiagnosticsTab from "./DiagnosticsTab";
import RunRecoveryTab from "./RunRecoveryTab";

const TAB_LABELS: Array<{ key: PanelTab; label: string }> = [
  { key: "revise", label: "AI 修改" },
  { key: "review", label: "审校问题" },
  { key: "cast", label: "人物与世界观" },
  { key: "context", label: "上下文" },
  { key: "run", label: "运行与恢复" },
];

interface ChapterRightPanelProps {
  novelId: string;
  activeTab: PanelTab;
  onTabChange: (tab: PanelTab) => void;
  chapterId: string;
  workspace: ChapterEditorWorkspaceResponse | null;
  selectedDiagnosticId: string | null;
  world?: { name: string; summary: string | null } | null;
  characters?: CastItem[];
  onOpenChapter?: (chapterId: string) => void;
  onFocusDiagnostic: (card: ChapterEditorDiagnosticCard) => void;
  onRunDiagnostic: (card: ChapterEditorDiagnosticCard) => void;
  onResolveDiagnostic: (card: ChapterEditorDiagnosticCard) => void;
  isResolvingDiagnostic?: boolean;
  /** 「AI 修改」页签的内容由 Shell 传入，保持既有候选/选区逻辑零改动。 */
  children: ReactNode;
}

/**
 * 右侧统一工作区：把「AI 修改」「审校问题」「人物与世界观」「运行与恢复」收在一个面板里。
 * 每个页签只读各自的数据源；AI 修改仍然完全复用原有组件与状态。
 */
export default function ChapterRightPanel(props: ChapterRightPanelProps) {
  const {
    novelId,
    activeTab: tab,
    onTabChange,
    chapterId,
    workspace,
    selectedDiagnosticId,
    onOpenChapter,
    onFocusDiagnostic,
    onRunDiagnostic,
    onResolveDiagnostic,
    isResolvingDiagnostic = false,
    children,
  } = props;
  const issueCount = workspace?.diagnosticCards.length ?? 0;

  return (
    <div className="flex h-full min-h-0 flex-col gap-2" data-testid="chapter-right-panel">
      <div className="flex shrink-0 gap-1 rounded-xl bg-muted/40 p-1">
        {TAB_LABELS.map((item) => (
          <button
            key={item.key}
            type="button"
            onClick={() => onTabChange(item.key)}
            data-testid={`panel-tab-${item.key}`}
            data-active={String(tab === item.key)}
            className={`flex-1 rounded-lg px-2 py-1.5 text-xs transition-colors ${
              tab === item.key ? "bg-background font-medium text-foreground" : "text-muted-foreground hover:text-foreground"
            }`}
          >
            {item.label}
            {item.key === "review" && issueCount > 0 ? ` ${issueCount}` : ""}
          </button>
        ))}
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto">
        {tab === "revise" ? children : null}
        {tab === "review" ? (
          <DiagnosticsTab
            workspace={workspace}
            selectedDiagnosticId={selectedDiagnosticId}
            onFocusDiagnostic={onFocusDiagnostic}
            onRunDiagnostic={onRunDiagnostic}
            onResolveDiagnostic={onResolveDiagnostic}
            isResolvingDiagnostic={isResolvingDiagnostic}
          />
        ) : null}
        {tab === "cast" ? (
          <CastWorldTab novelId={novelId} chapterId={chapterId} onOpenChapter={onOpenChapter} />
        ) : null}
        {tab === "context" ? <ContextPreviewTab novelId={novelId} chapterId={chapterId} /> : null}
        {tab === "run" ? <RunRecoveryTab novelId={novelId} chapterId={chapterId} /> : null}
      </div>
    </div>
  );
}
