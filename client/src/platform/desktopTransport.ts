import { Channel, invoke, isTauri } from "@tauri-apps/api/core";

export type AppErrorCode =
  | "VALIDATION"
  | "CONFLICT"
  | "NOT_FOUND"
  | "CANCELLED"
  | "MODEL_RATE_LIMIT"
  | "STORAGE_CORRUPT"
  | "INTEGRITY_ERROR";

export interface AppError {
  code: AppErrorCode;
  message: string;
  retryable: boolean;
  details?: unknown;
}

export interface NovelSummary {
  id: string;
  title: string;
  description: string;
  status: string;
  creationExperience: string;
  narrativeForm: string;
  writingMode: string;
  projectMode: string;
  updatedAt: number;
}

export interface NovelDetail {
  id: string;
  title: string;
  description: string;
  status: string;
  creationExperience: string;
  narrativeForm: string;
  targetAudience: string;
  bookSellingPoint: string;
  competingFeel: string;
  first30ChapterPromise: string;
  commercialTags: string[];
  genreId: string | null;
  primaryStoryModeId: string | null;
  secondaryStoryModeId: string | null;
  worldId: string | null;
  writingMode: string;
  projectMode: string;
  narrativePov: string | null;
  pacePreference: string | null;
  styleTone: string;
  emotionIntensity: string | null;
  aiFreedom: string | null;
  defaultChapterLength: number | null;
  estimatedChapterCount: number | null;
  projectStatus: string;
  storylineStatus: string;
  outlineStatus: string;
  resourceReadyScore: number;
  postGenerationStyleReviewEnabled: boolean;
  createdAt: number;
  updatedAt: number;
}

export interface CharacterSnapshot {
  id: string;
  novelId: string;
  name: string;
  role: string;
  personality?: string | null;
  arc?: string | null;
  createdAt: number;
  updatedAt: number;
}

export interface KnowledgeDocumentSummary {
  id: string;
  novelId: string;
  ownerType: string;
  title: string;
  version: number;
  indexStatus: string;
  createdAt: number;
}

export interface ChapterSnapshot {
  id: string;
  novelId: string;
  title: string;
  sortOrder: number;
  protection: string;
  officialVersion: number;
  content: string;
  updatedAt: number;
}

export interface ChapterRevisionSnapshot {
  id: string;
  chapterId: string;
  version: number;
  origin: string;
  content: string;
  contentSha256: string;
  createdAt: number;
  originDetail?: string;
  restoredFromRevisionId?: string | null;
  restoredBy?: string | null;
}

export interface VolumeSnapshot {
  id: string;
  novelId: string;
  title: string;
  sortOrder: number;
  summary: string;
}

export interface RunSummary {
  id: string;
  novelId: string;
  chapterId: string;
  chapterTitle: string;
  state: string;
  createdAt: number;
  updatedAt: number;
  provider: string;
  model: string;
}

export interface CommandOutcome {
  runId: string;
  status: string;
  replayed: boolean;
}

export interface RunArtifactView {
  id: string;
  runId?: string | null;
  artifactType: string;
  scope: string;
  version: number;
  status: string;
  protectedUserContent: boolean;
  payloadJson: string;
  createdAt: number;
}

export interface RunGateView {
  id: string;
  artifactTypes: string[];
}

export interface RunCandidateView {
  id: string;
  content: string;
  contentSha256: string;
}

export interface RunUsageView {
  calls: number;
  promptTokens: number;
  completionTokens: number;
}

export interface StoryFactView {
  id: string;
  factType: string;
  body: string;
  chapterOrder: number;
  sourceRevision?: string | null;
  status: string;
}

export function listStoryFacts(input: { novelId: string; beforeChapterOrder: number }): Promise<StoryFactView[]> {
  return invoke("list_story_facts", input);
}

export function upsertCharacterState(input: {
  id: string; characterId: string; chapterOrder: number; body: string;
  sourceChapterId: string; sourceRevision: string; status: string;
}): Promise<void> {
  return invoke("upsert_character_state", { request: input });
}

export function upsertPlotThread(input: {
  id: string; novelId: string; title: string; body: string; openedChapterOrder: number;
  resolvedChapterOrder?: number | null; status: string; sourceChapterId: string; sourceRevision: string;
}): Promise<void> {
  return invoke("upsert_plot_thread", { request: input });
}

export interface MemoryCandidateView {
  id: string; jobId: string; novelId: string; chapterId: string; chapterVersion: number;
  sourceContentHash: string; factType: string; truthfulness: string;
  entityName?: string | null; entityId?: string | null; value: string; evidenceQuote: string;
  evidenceStart?: number | null; confidence?: number | null; status: string; ambiguous: boolean;
  version: number; decisionReason?: string | null; createdAt: number; updatedAt: number;
}

export interface IndexJobView {
  id: string; novelId: string; sourceType: string; sourceId: string; sourceRevision: string;
  indexVersion: number; status: string; attemptCount: number; maxAttempts: number;
  nextRunAt: number; lastErrorCategory?: string | null; lastError?: string | null;
  createdAt: number; updatedAt: number;
}

export interface MemoryConflictView {
  id: string; novelId: string; conflictType: string; entityKey: string; chapterId: string;
  chapterOrder: number; candidateIds: string[]; factIds: string[]; status: string;
  version: number; description: string; createdAt: number; updatedAt: number;
}

export interface MemoryExtractionJobView {
  id: string; commandId: string; novelId: string; chapterId: string;
  chapterVersion: number; status: string; error?: string | null; createdAt: number;
}

export function extractMemory(input: {
  commandId: string; novelId: string; chapterId: string; provider: string;
  model: string; endpoint: string; allowLocalhostHttp: boolean; tokenBudget?: number | null;
}): Promise<MemoryExtractionJobView> {
  return invoke("extract_memory", { request: input });
}

export function listMemoryCandidates(input: { novelId: string; chapterId: string }): Promise<MemoryCandidateView[]> {
  return invoke("list_memory_candidates", input);
}

export function confirmMemoryCandidate(input: { candidateId: string; expectedVersion: number }): Promise<MemoryCandidateView> {
  return invoke("confirm_memory_candidate", input);
}

export function rejectMemoryCandidate(input: { candidateId: string; expectedVersion: number; reason: string }): Promise<MemoryCandidateView> {
  return invoke("reject_memory_candidate", input);
}

export function deferMemoryCandidate(input: { candidateId: string; expectedVersion: number }): Promise<MemoryCandidateView> {
  return invoke("defer_memory_candidate", input);
}

export function syncMemoryConflicts(input: { novelId: string }): Promise<MemoryConflictView[]> {
  return invoke("sync_memory_conflicts", input);
}

export function listMemoryConflicts(input: { novelId: string }): Promise<MemoryConflictView[]> {
  return invoke("list_memory_conflicts", input);
}

export function resolveMemoryConflict(input: {
  conflictId: string; expectedVersion: number; commandId: string; decision: string;
  chosenCandidateId?: string | null; reason: string; operator: string;
}): Promise<MemoryConflictView> {
  return invoke("resolve_memory_conflict", { request: input });
}

export function updateMemoryCandidateValue(input: { candidateId: string; expectedVersion: number; value: string }): Promise<MemoryCandidateView> {
  return invoke("update_memory_candidate_value", input);
}

export function upsertTimelineEvent(input: {
  id: string; novelId: string; chapterOrder: number; body: string;
  sourceChapterId: string; sourceRevision: string; status: string;
}): Promise<void> {
  return invoke("upsert_timeline_event", { request: input });
}

export interface RunDetailView {
  id: string;
  novelId: string;
  chapterId: string;
  chapterTitle: string;
  status: string;
  controlVersion: number;
  cursorStepId?: string | null;
  pauseKind?: string | null;
  pauseReason?: string | null;
  failureReason?: string | null;
  gate?: RunGateView | null;
  latestCandidate?: RunCandidateView | null;
  qualityDebt?: Array<Record<string, unknown>>;
  legalActions?: string[];
  artifacts: RunArtifactView[];
  usage: RunUsageView;
  provider: string;
  model: string;
  createdAt: number;
  updatedAt: number;
}

export interface StartGeneration {
  commandId: string;
  novelId: string;
  targetChapterId: string;
  provider: string;
  model: string;
  promptVersions: Record<string, string>;
  tokenBudget?: number | null;
  maxAutomaticRetries: number;
  endpoint: string;
  allowLocalhostHttp: boolean;
}

export type GenerationEvent =
  | { kind: "run_started"; runId: string; seq: number; occurredAt: number }
  | { kind: "step_started"; runId: string; seq: number; stepId: string; occurredAt: number }
  | { kind: "token_delta"; runId: string; stepId: string; text: string }
  | { kind: "checkpoint_saved"; runId: string; seq: number; stepId: string; candidateId: string; occurredAt: number }
  | { kind: "needs_approval"; runId: string; seq: number; stepId: string; occurredAt: number }
  | { kind: "step_succeeded"; runId: string; seq: number; stepId: string; occurredAt: number }
  | { kind: "run_paused"; runId: string; seq: number; stepId?: string; occurredAt: number }
  | { kind: "run_failed"; runId: string; seq: number; occurredAt: number }
  | { kind: "run_cancelled"; runId: string; seq: number; occurredAt: number }
  | { kind: "run_completed"; runId: string; seq: number; occurredAt: number };

export interface TextStreamEvent {
  kind: "delta" | "done" | "error";
  requestId: string;
  text?: string;
  usage?: { promptTokens: number | null; completionTokens: number | null };
  code?: string;
  message?: string;
  retryable: boolean;
}

export interface GenerateTextInput {
  requestId: string;
  prompt: string;
  maxOutputTokens?: number;
  model?: string;
}

export interface Genre {
  id: string;
  name: string;
  slug: string;
  description: string;
  sortOrder: number;
}

export interface StoryMode {
  id: string;
  name: string;
  slug: string;
  description: string;
  sortOrder: number;
}

export interface World {
  id: string;
  name: string;
  description: string;
  overviewSummary: string;
  sourceTemplateId: string | null;
  ownerNovelId: string | null;
  status: string;
  createdAt: number;
  updatedAt: number;
}

export interface KnowledgeHit {
  chunkId: string;
  ownerType: string;
  body: string;
}

export interface IndexKnowledgeInput {
  id: string;
  novelId: string;
  ownerType: "world" | "character" | "chapter" | "style" | "document";
  title: string;
  body: string;
  embedding?: number[];
  embeddingModel?: string;
}

export interface ChapterDraftView {
  novelId: string;
  chapterId: string;
  clientId: string;
  content: string;
  baseOfficialVersion: number;
  baseUpdatedAt: string;
  localRevision: number;
  updatedAt: number;
}

export interface ModelSetupView {
  endpoint: string;
  model: string;
  allowLocalhostHttp: boolean;
  hasApiKey: boolean;
}

export interface SaveModelSetupInput {
  endpoint: string;
  model: string;
  allowLocalhostHttp: boolean;
  apiKey: string;
}

export interface DesktopShellBootstrap {
  state: "launching" | "starting-server" | "ready" | "error";
  stage: string;
  title: string;
  detail: string;
  logDir: string;
  logFile: string;
  updatedAtMs: number;
  canRetry: boolean;
}

const liveChannels = new Set<Channel<GenerationEvent>>();
const liveTextChannels = new Set<Channel<TextStreamEvent>>();

function eventKey(event: GenerationEvent): string | null {
  return event.kind === "token_delta" ? null : `${event.kind}:${event.seq}`;
}

export function hasDesktopShell(): boolean {
  return typeof window !== "undefined" && isTauri();
}

export function listNovels(): Promise<NovelSummary[]> {
  return invoke("list_novels");
}

export function getNovel(novelId: string): Promise<NovelDetail> {
  return invoke("get_novel", { novelId });
}

export function createNovel(input: { title: string; description: string }): Promise<NovelSummary> {
  return invoke("create_novel", input);
}

export interface CreateNovelSetupInput {
  title: string;
  description?: string;
  targetAudience?: string;
  bookSellingPoint?: string;
  competingFeel?: string;
  first30ChapterPromise?: string;
  commercialTags?: string[];
  genreId?: string;
  primaryStoryModeId?: string;
  secondaryStoryModeId?: string;
  worldId?: string;
  writingMode?: string;
  projectMode?: string;
  creationExperience?: string;
  narrativeForm?: string;
  styleTone?: string;
  defaultChapterLength?: number;
  estimatedChapterCount?: number;
  postGenerationStyleReviewEnabled?: boolean;
}

export function createNovelWithSetup(input: CreateNovelSetupInput): Promise<NovelDetail> {
  return invoke("create_novel_with_setup", { request: input });
}

export interface UpdateNovelInput {
  novelId: string;
  title?: string;
  description?: string;
  targetAudience?: string | null;
  bookSellingPoint?: string | null;
  competingFeel?: string | null;
  first30ChapterPromise?: string | null;
  commercialTags?: string[] | null;
  genreId?: string | null;
  primaryStoryModeId?: string | null;
  secondaryStoryModeId?: string | null;
  worldId?: string | null;
  narrativeForm?: string;
  writingMode?: string;
  projectMode?: string;
  narrativePov?: string | null;
  pacePreference?: string | null;
  styleTone?: string | null;
  emotionIntensity?: string | null;
  aiFreedom?: string | null;
  defaultChapterLength?: number | null;
  estimatedChapterCount?: number | null;
  projectStatus?: string;
  storylineStatus?: string;
  outlineStatus?: string;
  resourceReadyScore?: number;
  status?: string;
  postGenerationStyleReviewEnabled?: boolean;
}

export function updateNovel(input: UpdateNovelInput): Promise<NovelDetail> {
  const { novelId, ...request } = input;
  return invoke("update_novel", { novelId, request });
}

export function deleteNovel(novelId: string): Promise<void> {
  return invoke("delete_novel", { novelId });
}

export function createCharacter(input: { id: string; novelId: string; name: string; role: string }): Promise<CharacterSnapshot> {
  return invoke("create_character", { request: input });
}

export function listCharacters(novelId: string): Promise<CharacterSnapshot[]> {
  return invoke("list_characters", { novelId });
}

export function listChapters(novelId: string): Promise<ChapterSnapshot[]> {
  return invoke("list_chapters", { novelId });
}

export function createChapter(input: {
  novelId: string;
  title: string;
  sortOrder: number;
  volumeId?: string;
}): Promise<ChapterSnapshot> {
  return invoke("create_chapter", input);
}

export function getChapter(input: {
  novelId: string;
  chapterId: string;
}): Promise<ChapterSnapshot> {
  return invoke("get_chapter", input);
}

export function saveChapter(input: {
  novelId: string;
  chapterId: string;
  expectedVersion: number;
  title: string;
  content: string;
}): Promise<ChapterSnapshot> {
  return invoke("save_chapter", input);
}

export function deleteChapter(input: {
  novelId: string;
  chapterId: string;
  expectedVersion: number;
}): Promise<void> {
  return invoke("delete_chapter", input);
}

export interface ChapterEditorWorkspace {
  chapterMeta: {
    chapterId: string;
    order: number;
    title: string;
    wordCount: number;
    openIssueCount: number;
    styleSummary?: string | null;
    updatedAt: string;
  };
  macroContext: {
    chapterRoleInVolume: string;
    volumeTitle: string;
    volumePositionLabel: string;
    volumePhaseLabel: string;
    paceDirective: string;
    chapterMission: string;
    previousChapterBridge: string;
    nextChapterBridge: string;
    activePlotThreads: string[];
    characterStateSummary: string;
    worldConstraintSummary: string;
    mustKeepConstraints: string[];
  };
  diagnosticCards: unknown[];
  recommendedTask: unknown | null;
  refreshReason: string;
}

export function getChapterEditorWorkspace(input: {
  novelId: string;
  chapterId: string;
}): Promise<ChapterEditorWorkspace> {
  return invoke("get_chapter_editor_workspace", input);
}

export function updateChapterMeta(input: {
  novelId: string;
  chapterId: string;
  expectedVersion: number;
  title: string;
  sortOrder: number;
  volumeId?: string;
}): Promise<ChapterSnapshot> {
  return invoke("update_chapter_meta", input);
}

export function exportNovel(input: {
  novelId: string;
  format: "markdown" | "json" | "txt";
}): Promise<string> {
  return invoke("export_novel", input);
}

export function exportChapters(input: {
  novelId: string;
  chapterIds: string[];
  format: "markdown" | "txt";
}): Promise<string> {
  return invoke("export_chapters", input);
}

export function exportNovelSelection(input: {
  novelId: string;
  selection: "chapter" | "range" | "entire_book" | "chapter_ids";
  format: "markdown" | "txt";
  chapterId?: string;
  chapterIds?: string[];
  startOrder?: number;
  endOrder?: number;
}): Promise<string> {
  return invoke("export_novel_selection", input);
}

export function listChapterRevisions(input: {
  novelId: string;
  chapterId: string;
}): Promise<ChapterRevisionSnapshot[]> {
  return invoke("list_chapter_revisions", input);
}

export function restoreChapterRevision(input: {
  novelId: string;
  chapterId: string;
  revisionId: string;
  expectedVersion: number;
  commandId: string;
  draftDecision: "reject" | "keep" | "discard_own";
  draftClientId?: string;
  expectedDraftRevision?: number;
}): Promise<ChapterSnapshot> {
  return invoke("restore_chapter_revision", input);
}

export function listVolumes(novelId: string): Promise<VolumeSnapshot[]> {
  return invoke("list_volumes", { novelId });
}

export function createVolume(input: {
  novelId: string;
  title: string;
  sortOrder: number;
  summary: string;
}): Promise<VolumeSnapshot> {
  return invoke("create_volume", input);
}

export function updateVolume(input: {
  novelId: string;
  volumeId: string;
  title: string;
  sortOrder: number;
  summary: string;
}): Promise<VolumeSnapshot> {
  return invoke("update_volume", input);
}

export function deleteVolume(input: { novelId: string; volumeId: string }): Promise<void> {
  return invoke("delete_volume", input);
}

export function listRuns(novelId: string): Promise<RunSummary[]> {
  return invoke("list_runs", { novelId });
}

export function listAllRuns(): Promise<RunSummary[]> {
  return invoke("list_all_runs");
}
export function getActiveRun(novelId: string): Promise<RunSummary | null> {
  return invoke("get_active_run", { novelId });
}

export function getRunDetail(novelId: string, runId: string): Promise<RunDetailView> {
  return invoke("get_run_detail", { novelId, runId });
}

export function approveRun(input: { runId: string; commandId: string; expectedVersion?: number }): Promise<CommandOutcome> {
  return invoke("approve_run", input);
}

export function rejectRun(input: { runId: string; commandId: string; expectedVersion?: number }): Promise<CommandOutcome> {
  return invoke("reject_run", input);
}

export function regenerateRun(input: { runId: string; commandId: string; expectedVersion?: number }): Promise<CommandOutcome> {
  return invoke("regenerate_run", input);
}

export function confirmAfterEditRun(input: { runId: string; commandId: string; expectedVersion?: number; payloadJson: string }): Promise<CommandOutcome> {
  return invoke("confirm_after_edit_run", input);
}

export interface CandidateEditDraftView {
  novelId: string; chapterId: string; runId: string; gateId: string; clientId: string;
  baseCandidateId: string; baseContentSha256: string; content: string;
  localRevision: number; controlVersion: number; updatedAt: number;
}

export function getCandidateEditDraft(input: { runId: string; gateId: string; clientId: string }): Promise<CandidateEditDraftView | null> {
  return invoke("get_candidate_edit_draft", input);
}

export function saveCandidateEditDraft(input: {
  novelId: string; chapterId: string; runId: string; gateId: string; clientId: string;
  baseCandidateId: string; baseContentSha256: string; content: string;
  controlVersion: number; expectedRevision: number;
}): Promise<CandidateEditDraftView> {
  return invoke("save_candidate_edit_draft", { request: input });
}

export interface ReviewOccurrenceView {
  id: string; runId: string; modelIssueId?: string | null; stableIssueKey: string;
  issueType: string; severity: string; description: string; evidence: string;
  suggestion: string; sourceRef?: string | null; reviewVersion: number; createdAt: number;
}

export function listReviewOccurrences(input: { runId: string; stableIssueKey: string }): Promise<ReviewOccurrenceView[]> {
  return invoke("list_review_occurrences", input);
}

export function acceptQualityDebt(input: { runId: string; debtId: string; expectedVersion: number; reason: string }): Promise<Record<string, unknown>> {
  return invoke("accept_quality_debt", input);
}

export function listCandidateEditDrafts(input: { novelId: string; runId: string }): Promise<CandidateEditDraftView[]> {
  return invoke("list_candidate_edit_drafts", input);
}

export function clearCandidateEditDraft(input: {
  runId: string; gateId: string; clientId: string; content: string; localRevision: number;
}): Promise<boolean> {
  return invoke("clear_candidate_edit_draft", input);
}

export function confirmCandidateAfterEdit(input: {
  runId: string;
  commandId: string;
  expectedVersion?: number;
  baseCandidateId: string;
  baseContentSha256: string;
  content: string;
}): Promise<CommandOutcome> {
  return invoke("confirm_candidate_after_edit_run", input);
}

export function cancelRun(input: { runId: string; commandId: string }): Promise<CommandOutcome> {
  return invoke("cancel_run", input);
}

export function resumeRun(input: {
  runId: string;
  commandId: string;
  acceptCandidate: boolean;
}): Promise<CommandOutcome> {
  return invoke("resume_run", input);
}

export function replayRunEvents(input: {
  runId: string;
  afterSeq: number;
}): Promise<GenerationEvent[]> {
  return invoke("replay_run", input);
}

export interface RunSubscription {
  ready: Promise<void>;
  close: () => void;
}

export function subscribeRunWithCleanup(
  input: { runId: string },
  onEvent: (event: GenerationEvent) => void,
): RunSubscription {
  const channel = new Channel<GenerationEvent>();
  let closed = false;
  channel.onmessage = (event) => {
    if (closed) return;
    onEvent(event);
    if (event.kind === "run_completed" || event.kind === "run_failed" || event.kind === "run_cancelled") {
      liveChannels.delete(channel);
    }
  };
  liveChannels.add(channel);
  return {
    ready: invoke("subscribe_run", { runId: input.runId, channel }),
    close: () => {
      closed = true;
      liveChannels.delete(channel);
    },
  };
}

export function subscribeRun(
  input: { runId: string },
  onEvent: (event: GenerationEvent) => void,
): Promise<void> {
  return subscribeRunWithCleanup(input, onEvent).ready;
}

export async function startGeneration(
  input: StartGeneration,
  onEvent: (event: GenerationEvent) => void,
): Promise<{ runId: string }> {
  const seen = new Set<string>();
  const deliver = (event: GenerationEvent) => {
    const key = eventKey(event);
    if (key) {
      if (seen.has(key)) {
        return;
      }
      seen.add(key);
    }
    onEvent(event);
  };
  const outcome = await invoke<CommandOutcome>("open_run", { request: input });
  await subscribeRun({ runId: outcome.runId }, deliver);
  const missed = await replayRunEvents({ runId: outcome.runId, afterSeq: 0 });
  missed.forEach(deliver);
  return { runId: outcome.runId };
}

/**
 * Open a run and wait until the first candidate checkpoint is visible to the editor.
 * This closes the race between `open_run` and `subscribe_run` without polling.
 */
export async function startGenerationAwaitCheckpoint(
  input: StartGeneration,
  onEvent: (event: GenerationEvent) => void,
  timeoutMs = 180_000,
): Promise<{ runId: string }> {
  const outcome = await invoke<CommandOutcome>("open_run", { request: input });
  await new Promise<void>((resolve, reject) => {
    let settled = false;
    const timer = setTimeout(() => {
      finish(new Error("本地生成等待超时，请检查模型服务后重试。"));
    }, timeoutMs);
    const finish = (error?: Error) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      if (error) {
        void cancelGeneration(outcome.runId).catch(() => undefined);
        reject(error);
        return;
      }
      resolve();
    };
    const deliver = (event: GenerationEvent) => {
      onEvent(event);
      if (event.kind === "checkpoint_saved") {
        finish();
      } else if (
        event.kind === "run_failed"
        || event.kind === "run_cancelled"
        || event.kind === "run_paused"
        || event.kind === "run_completed"
      ) {
        finish(new Error("本地生成没有产生可用候选。"));
      }
    };
    void subscribeRun({ runId: outcome.runId }, deliver)
      .then(async () => {
        const missed = await replayRunEvents({ runId: outcome.runId, afterSeq: 0 });
        missed.forEach(deliver);
      })
      .catch((error: unknown) => {
        finish(error instanceof Error ? error : new Error("本地生成订阅失败。"));
      });
  });
  return { runId: outcome.runId };
}

export function generateText(
  input: GenerateTextInput,
  onEvent: (event: TextStreamEvent) => void,
): Promise<void> {
  const channel = new Channel<TextStreamEvent>();
  channel.onmessage = onEvent;
  liveTextChannels.add(channel);
  return invoke<void>("generate_text", { request: input, channel }).finally(() => {
    liveTextChannels.delete(channel);
  });
}

export function smokeChannelMessage(onEvent: (event: GenerationEvent) => void): Promise<void> {
  const channel = new Channel<GenerationEvent>();
  channel.onmessage = onEvent;
  return invoke("smoke_channel_message", { channel });
}

export function cancelTextGeneration(requestId: string): Promise<void> {
  return invoke("cancel_text_generation", { requestId });
}

export function listKnowledgeDocuments(): Promise<KnowledgeDocumentSummary[]> {
  return invoke("list_knowledge_documents");
}

export function indexKnowledge(input: IndexKnowledgeInput): Promise<void> {
  return invoke("index_knowledge", { request: input });
}

export function listIndexJobs(input: { novelId: string }): Promise<IndexJobView[]> {
  return invoke("list_index_jobs", input);
}

export function retryIndexJob(input: { jobId: string }): Promise<IndexJobView> {
  return invoke("retry_index_job", input);
}

export function recoverIndexJobs(): Promise<IndexJobView[]> {
  return invoke("recover_index_jobs");
}

export function retryKnowledgeDocumentWithJob(input: {
  novelId: string; documentId: string; sourceRevision: string; indexVersion: number;
}): Promise<IndexJobView> {
  return invoke("retry_knowledge_document_with_job", input);
}

export function retryKnowledgeDocument(documentId: string): Promise<boolean> {
  return invoke("retry_knowledge_document", { documentId });
}

export interface KnowledgeDocumentBody {
  id: string; novelId: string; ownerType: string; title: string; body: string;
  sourceRevision?: string | null; indexStatus: string; indexVersion: number;
}

export function getKnowledgeDocument(documentId: string): Promise<KnowledgeDocumentBody | null> {
  return invoke("get_knowledge_document", { documentId });
}

export function deleteKnowledgeDocument(documentId: string): Promise<boolean> {
  return invoke("delete_knowledge_document", { documentId });
}

export function searchKnowledge(input: {
  novelId: string;
  query: string;
  limit?: number;
}): Promise<KnowledgeHit[]> {
  return invoke("search_knowledge", input);
}

export function searchKnowledgeVector(input: {
  novelId: string;
  model: string;
  vector: number[];
  limit?: number;
}): Promise<KnowledgeHit[]> {
  return invoke("search_knowledge_vector", input);
}

export function listGenres(): Promise<Genre[]> {
  return invoke("list_genres");
}

export function listStoryModes(): Promise<StoryMode[]> {
  return invoke("list_story_modes");
}

export function listWorlds(): Promise<World[]> {
  return invoke("list_worlds");
}

export interface TaskOverview {
  queuedCount: number;
  runningCount: number;
  failedCount: number;
  cancelledCount: number;
  waitingApprovalCount: number;
  recoveryCandidateCount: number;
}

export interface RecoverableTask {
  id: string;
  kind: string;
  title: string;
  ownerLabel: string;
  status: string;
  currentStage: string | null;
  currentItemLabel: string | null;
  resumeAction: string;
  sourceRoute: string;
  recoveryHint: string;
}

export function getTaskOverview(): Promise<TaskOverview> {
  return invoke("get_task_overview");
}

export function listRecoveryCandidates(): Promise<{ items: RecoverableTask[] }> {
  return invoke("list_recovery_candidates");
}

export function getQuickSetupStatus(): Promise<Record<string, unknown>> {
  return invoke("get_quick_setup_status");
}

export function getFirstNovelOnboarding(): Promise<Record<string, unknown>> {
  return invoke("get_first_novel_onboarding");
}

export function getChapterDraft(input: {
  novelId: string;
  chapterId: string;
  clientId: string;
}): Promise<ChapterDraftView | null> {
  return invoke("get_chapter_draft", input);
}

export function listChapterDrafts(input: {
  novelId: string;
  chapterId: string;
}): Promise<ChapterDraftView[]> {
  return invoke("list_chapter_drafts", input);
}

export function saveChapterDraft(input: {
  novelId: string;
  chapterId: string;
  clientId: string;
  content: string;
  baseOfficialVersion: number;
  baseUpdatedAt: string;
  expectedRevision: number;
}): Promise<ChapterDraftView> {
  return invoke("save_chapter_draft", { request: input });
}

export function clearChapterDraft(input: {
  novelId: string;
  chapterId: string;
  clientId: string;
  submittedContent: string;
  localRevision: number;
}): Promise<boolean> {
  return invoke("clear_chapter_draft", input);
}

export function getModelSetup(): Promise<ModelSetupView | null> {
  return invoke("get_model_setup");
}

export function saveModelSetup(input: SaveModelSetupInput): Promise<ModelSetupView> {
  return invoke("save_model_setup", { request: input });
}

export function hasApiKey(): Promise<boolean> {
  return invoke("has_api_key");
}

export function cancelGeneration(runId: string): Promise<void> {
  return invoke("cancel_generation", { runId });
}

export function openRun(input: StartGeneration): Promise<CommandOutcome> {
  return invoke("open_run", { request: input });
}

export function getDesktopShellBootstrap(): Promise<DesktopShellBootstrap> {
  return invoke("get_desktop_bootstrap_snapshot");
}

export function reportClientRuntime(input: {
  config: unknown;
  locationHash: string;
  runtimeMode: string;
  message?: string;
}): Promise<void> {
  return invoke("report_client_runtime", input);
}

export function openDesktopShellLogsDirectory(): Promise<void> {
  return invoke("open_desktop_logs_directory");
}

export function restartDesktopShell(): Promise<void> {
  return invoke("restart_desktop_app");
}

export interface BatchRunView {
  id: string;
  novelId: string;
  commandId: string;
  requestHash: string;
  status: string;
  cursor: number;
  version: number;
  claimEpoch: number;
  failureReason: string | null;
  createdAt: number;
  updatedAt: number;
  items: Array<{
    id: string;
    chapterId: string;
    ordinal: number;
    runId: string | null;
    status: string;
    version: number;
    lastError: string | null;
    startedAt: number | null;
    completedAt: number | null;
  }>;
}

export function createBatch(input: {
  commandId: string; novelId: string; chapterIds: string[];
  provider: string; model: string; promptVersions: Record<string, string>;
  tokenBudget?: number | null; contextTokenBudget?: number | null;
  maxAutomaticRetries: number; repairBudget?: number | null;
  endpoint: string; allowLocalhostHttp: boolean;
}): Promise<BatchRunView> { return invoke("create_batch", { request: input }); }
export function listBatches(novelId: string): Promise<BatchRunView[]> { return invoke("list_batches", { novelId }); }
export function getBatch(batchId: string): Promise<BatchRunView> { return invoke("get_batch", { batchId }); }
export function pauseBatch(batchId: string, expectedVersion: number): Promise<BatchRunView> { return invoke("pause_batch", { batchId, expectedVersion }); }
export function resumeBatch(batchId: string, expectedVersion: number): Promise<BatchRunView> { return invoke("resume_batch", { batchId, expectedVersion }); }
export function cancelBatch(batchId: string, expectedVersion: number): Promise<BatchRunView> { return invoke("cancel_batch", { batchId, expectedVersion }); }
export function tickBatches(): Promise<BatchRunView[]> { return invoke("tick_batches"); }
