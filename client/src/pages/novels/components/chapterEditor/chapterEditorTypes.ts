import type {
  ChapterEditorAiRevisionRequest,
  ChapterEditorAiRevisionResponse,
  Chapter,
  ChapterEditorDiagnosticCard,
  ChapterEditorOperation,
  ChapterEditorCursorContinuationRequest,
  ChapterEditorCursorOperation,
  ChapterEditorRevisionScope,
  ChapterEditorWorkspaceResponse,
} from "@ai-novel/shared/types/novel";
import type { RewriteBillingOutcome } from "@/api/novel/chapterSelectionRewriteBilling";

export interface ChapterEditorSelectionRange {
  from: number;
  to: number;
  text: string;
}

export interface SelectionToolbarPosition {
  top: number;
  left: number;
}

export interface ChapterEditorSessionState extends Partial<ChapterEditorAiRevisionResponse> {
  status: "idle" | "loading" | "ready" | "error";
  requestLabel?: string;
  customInstruction?: string;
  viewMode: "inline" | "block";
  errorMessage?: string;
  mode: "revision" | "continuation";
  continuationOperation?: ChapterEditorCursorOperation;
  /**
   * 上一次改写失败时能确认到的计费状态。
   * 结果未知时界面必须提示可能已产生费用，并在重新生成前要求作者确认。
   */
  billingOutcome?: RewriteBillingOutcome;
}

export interface ChapterEditorCursorRequestBuilderInput {
  operation: ChapterEditorCursorOperation;
  cursorOffset: number;
  content: string;
  context: ChapterEditorCursorContinuationRequest["context"];
  instruction?: string;
  provider?: import("@ai-novel/shared/types/llm").LLMProvider;
  model?: string;
  temperature?: number;
}

export interface ChapterEditorShellProps {
  novelId: string;
  chapter: Chapter | undefined;
  /** 整本小说的章节列表，用于章节导航；缺省时不显示导航。 */
  chapters?: Chapter[];
  volumes?: { id: string; title: string }[];
  onSelectChapter?: (chapterId: string) => void;
  runAttention?: Record<string, import("./navigation/chapterNavigatorModel").ChapterRunAttention>;
  world?: { name: string; summary: string | null } | null;
  characters?: import("./rightPanel/panelModel").CastItem[];
  workspace: ChapterEditorWorkspaceResponse | null;
  workspaceStatus: "loading" | "ready" | "error";
  onBack?: () => void;
  onOpenVersionHistory?: () => void;
  onRefreshWorkspace?: () => void;
  isRefreshingWorkspace?: boolean;
  onRunFullAudit?: () => void;
  onGenerateChapterPlan?: () => void;
  onReplanChapter?: () => void;
  isRunningFullAudit?: boolean;
  isGeneratingChapterPlan?: boolean;
  isReplanningChapter?: boolean;
}

export interface ChapterEditorRequestBuilderInput {
  source: "preset" | "freeform";
  scope: ChapterEditorRevisionScope;
  presetOperation?: ChapterEditorOperation;
  instruction?: string;
  selection?: ChapterEditorSelectionRange | null;
  content: string;
  contextRange?: ChapterEditorSelectionRange | null;
  provider?: import("@ai-novel/shared/types/llm").LLMProvider;
  model?: string;
  temperature?: number;
}

export interface ChapterEditorDiagnosticSelectionState {
  card: ChapterEditorDiagnosticCard;
  fromTask?: boolean;
}

export type BuildAiRevisionRequest = (
  input: ChapterEditorRequestBuilderInput,
) => ChapterEditorAiRevisionRequest;
