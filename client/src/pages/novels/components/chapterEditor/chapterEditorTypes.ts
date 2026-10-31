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
