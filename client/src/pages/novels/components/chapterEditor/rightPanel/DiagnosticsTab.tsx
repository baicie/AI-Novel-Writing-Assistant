import type { ChapterEditorDiagnosticCard, ChapterEditorWorkspaceResponse } from "@ai-novel/shared/types/novel";
import { Button } from "@/components/ui/button";

interface DiagnosticsTabProps {
  workspace: ChapterEditorWorkspaceResponse | null;
  selectedDiagnosticId: string | null;
  onFocusDiagnostic: (card: ChapterEditorDiagnosticCard) => void;
  onRunDiagnostic: (card: ChapterEditorDiagnosticCard) => void;
  onResolveDiagnostic: (card: ChapterEditorDiagnosticCard) => void;
  isResolvingDiagnostic?: boolean;
}

/** 「审校问题」：复用工作区里已有的诊断卡，不改数据来源。 */
export default function DiagnosticsTab(props: DiagnosticsTabProps) {
  const {
    workspace,
    selectedDiagnosticId,
    onFocusDiagnostic,
    onRunDiagnostic,
    onResolveDiagnostic,
    isResolvingDiagnostic = false,
  } = props;
  const cards = workspace?.diagnosticCards ?? [];

  if (cards.length === 0) {
    return (
      <div className="px-1 py-4 text-sm text-muted-foreground" data-testid="diagnostics-empty">
        当前章节没有待处理的审校问题。
      </div>
    );
  }

  return (
    <ul className="flex flex-col gap-2 px-1 py-2" data-testid="diagnostics-tab">
      {cards.map((card) => (
        <li
          key={card.id}
          className={`rounded-xl px-3 py-2 ${card.id === selectedDiagnosticId ? "bg-muted" : "bg-muted/40"}`}
        >
          <div className="text-sm font-medium text-foreground">{card.title}</div>
          <div className="mt-1 text-xs leading-5 text-muted-foreground">{card.problemSummary}</div>
          {card.whyItMatters ? (
            <div className="mt-1 text-xs leading-5 text-muted-foreground/80">{card.whyItMatters}</div>
          ) : null}
          <div className="mt-2 flex flex-wrap gap-2">
            <Button size="sm" variant="outline" onClick={() => onFocusDiagnostic(card)}>
              定位
            </Button>
            <Button size="sm" onClick={() => onRunDiagnostic(card)}>
              让 AI 改这一段
            </Button>
            <Button size="sm" variant="outline" disabled={isResolvingDiagnostic} onClick={() => onResolveDiagnostic(card)}>
              标记已处理
            </Button>
          </div>
        </li>
      ))}
    </ul>
  );
}
