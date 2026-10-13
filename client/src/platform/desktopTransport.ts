const IPC_KEY = "__TAURI_TO_IPC_KEY__";

type TauriInternals = {
  invoke: (command: string, args?: Record<string, unknown>) => Promise<unknown>;
  transformCallback: (callback: (response: unknown) => void, once?: boolean) => number;
};

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
  updatedAt: number;
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

export interface CommandOutcome {
  runId: string;
  status: string;
  replayed: boolean;
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
  | { kind: "run_completed"; runId: string; seq: number; occurredAt: number };

function internals(): TauriInternals {
  const host = window as Window & { 
    __TAURI_INTERNALS__?: TauriInternals;
    __TAURI__?: { core?: { invoke: (command: string, args?: Record<string, unknown>) => Promise<unknown> } };
  };
  
  // Try to use Tauri's native API first
  if (host.__TAURI__?.core?.invoke) {
    return {
      invoke: (command: string, args?: Record<string, unknown>) => {
        return host.__TAURI__!.core!.invoke(command, args);
      },
      transformCallback: (callback: (response: unknown) => void, once?: boolean) => {
        // Tauri handles callbacks differently, but we return an ID for compatibility
        return 0;
      },
    };
  }
  
  // Fall back to Electron IPC setup
  if (host.__TAURI_INTERNALS__) {
    return host.__TAURI_INTERNALS__;
  }
  
  throw new Error("当前窗口没有连接到本地创作内核。");
}

class IpcChannel<T> {
  readonly id: number;

  constructor(onMessage: (payload: T) => void) {
    this.id = Math.random();
    
    // Store callback for potential future use
    // In native Tauri, channels are handled automatically
  }

  [IPC_KEY](): string {
    // Tauri will automatically convert this to the right format
    return `__CHANNEL__:${this.id}`;
  }
}

function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  return internals().invoke(command, args) as Promise<T>;
}

function eventKey(event: GenerationEvent): string | null {
  if (event.kind === "token_delta") {
    return null;
  }
  return `${event.kind}:${event.seq}`;
}

export function listNovels(): Promise<NovelSummary[]> {
  return invoke("list_novels");
}

export function createNovel(input: { title: string; description: string }): Promise<NovelSummary> {
  return invoke("create_novel", input);
}

export function createChapter(input: { novelId: string; title: string; sortOrder: number }): Promise<ChapterSnapshot> {
  return invoke("create_chapter", input);
}

export function getChapter(input: { novelId: string; chapterId: string }): Promise<ChapterSnapshot> {
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

export function approveRun(input: { runId: string; commandId: string }): Promise<CommandOutcome> {
  return invoke("approve_run", input);
}

export function cancelRun(input: { runId: string; commandId: string }): Promise<CommandOutcome> {
  return invoke("cancel_run", input);
}

export function resumeRun(input: { runId: string; commandId: string; acceptCandidate: boolean }): Promise<CommandOutcome> {
  return invoke("resume_run", input);
}

export function replayRunEvents(input: { runId: string; afterSeq: number }): Promise<GenerationEvent[]> {
  return invoke("replay_run", input);
}

export function subscribeRun(input: { runId: string }, onEvent: (event: GenerationEvent) => void): Promise<void> {
  return invoke("subscribe_run", { runId: input.runId, channel: new IpcChannel(onEvent) });
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
