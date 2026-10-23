import { useCallback, useEffect, useRef, useState } from "react";
import { normalizeChapterText } from "@ai-novel/shared/types/chapterEditor/document";
import {
  clearChapterDraft,
  getChapterDraft,
  hasDesktopShell,
  listChapterDrafts,
  saveChapterDraft,
} from "@/platform/desktopTransport";
import { acknowledgeChapterSave, receiveChapter, restoreChapterDraft, type ChapterDraft } from "./draftState";

const CLIENT_KEY = "ai-novel:chapter-editor-client:v1";

function getClientId(): string {
  try {
    const existing = sessionStorage.getItem(CLIENT_KEY);
    if (existing) return existing;
    const next = globalThis.crypto?.randomUUID?.() ?? `editor-${Date.now()}-${Math.random().toString(36).slice(2)}`;
    sessionStorage.setItem(CLIENT_KEY, next);
    return next;
  } catch {
    return `editor-${Date.now()}-${Math.random().toString(36).slice(2)}`;
  }
}

export type LocalDraftStatus = "unavailable" | "idle" | "saving" | "saved" | "error";

/** Mounted once per chapter. Tab-scoped sessionStorage remains the offline fallback. */
export function useChapterDraft(input: {
  novelId: string;
  chapter: { id: string; content?: string | null; updatedAt: string; officialVersion?: number } | undefined;
  busy: boolean;
}) {
  const storageKey = `ai-novel:chapter-draft:v1:${input.novelId}:${input.chapter?.id}`;
  const incoming = { content: normalizeChapterText(input.chapter?.content), updatedAt: input.chapter?.updatedAt ?? "" };
  const desktop = hasDesktopShell();
  const chapterId = input.chapter?.id;
  const chapterOfficialVersion = input.chapter?.officialVersion ?? 0;
  const chapterUpdatedAt = input.chapter?.updatedAt;
  const clientId = useRef(getClientId());
  const durableRevision = useRef(0);
  const saveSequence = useRef(0);
  const saveQueue = useRef<Promise<boolean>>(Promise.resolve(false));
  const mounted = useRef(true);
  const [durableLoaded, setDurableLoaded] = useState(!desktop || !input.chapter);
  const [localDraftStatus, setLocalDraftStatus] = useState<LocalDraftStatus>(desktop ? "idle" : "unavailable");
  const [localDraftConflict, setLocalDraftConflict] = useState(false);
  const [restored, setRestored] = useState(false);
  const [storageError, setStorageError] = useState(false);
  const [draft, setDraft] = useState<ChapterDraft>(() => {
    const saved = { content: incoming.content, savedContent: incoming.content, savedUpdatedAt: incoming.updatedAt };
    try { return restoreChapterDraft(sessionStorage.getItem(storageKey), saved); } catch { return saved; }
  });
  const current = useRef(draft);
  const isDirty = draft.content !== draft.savedContent;
  const hasExternalChange = incoming.updatedAt > draft.savedUpdatedAt;

  useEffect(() => () => { mounted.current = false; }, []);

  const publish = useCallback((next: ChapterDraft) => {
    current.current = next;
    setDraft(next);
    try {
      if (next.content === next.savedContent) sessionStorage.removeItem(storageKey);
      else sessionStorage.setItem(storageKey, JSON.stringify({ version: 1, ...next }));
      setStorageError(false);
    } catch { setStorageError(true); }
  }, [storageKey]);

  const identityRef = useRef(storageKey);
  useEffect(() => {
    if (identityRef.current === storageKey) return;
    identityRef.current = storageKey;
    durableRevision.current = 0;
    const saved = { content: incoming.content, savedContent: incoming.content, savedUpdatedAt: incoming.updatedAt };
    try { publish(restoreChapterDraft(sessionStorage.getItem(storageKey), saved)); }
    catch { publish(saved); }
  }, [incoming.content, incoming.updatedAt, publish, storageKey]);

  useEffect(() => { setRestored(current.current.content !== current.current.savedContent); }, []);

  // Load the durable SQLite draft for this tab. A stale base version is preserved and
  // surfaced as a conflict; it is never deleted here.
  useEffect(() => {
    if (!desktop || !input.chapter) {
      setDurableLoaded(true);
      return;
    }
    let cancelled = false;
    setDurableLoaded(false);
    setLocalDraftStatus("idle");
    void getChapterDraft({
      novelId: input.novelId,
      chapterId: input.chapter.id,
      clientId: clientId.current,
    }).then(async (currentDraft) => {
      if (cancelled) return;
      let stored = currentDraft;
      let recoveredFromAnotherClient = false;
      if (!stored) {
        const drafts = await listChapterDrafts({ novelId: input.novelId, chapterId: input.chapter!.id });
        stored = drafts.find(draft => draft.content !== incoming.content) ?? drafts[0] ?? null;
        recoveredFromAnotherClient = Boolean(stored);
      }
      if (!stored) return;
      durableRevision.current = recoveredFromAnotherClient ? 0 : stored.localRevision;
      if (stored.content !== incoming.content) {
        publish({ content: stored.content, savedContent: incoming.content, savedUpdatedAt: stored.baseUpdatedAt || incoming.updatedAt });
        setRestored(true);
      }
      setLocalDraftConflict(
        recoveredFromAnotherClient
        || stored.baseOfficialVersion !== (input.chapter?.officialVersion ?? 0)
        || stored.baseUpdatedAt !== incoming.updatedAt,
      );
      setLocalDraftStatus("saved");
    }).catch(() => {
      if (!cancelled) setLocalDraftStatus("error");
    }).finally(() => {
      if (!cancelled) setDurableLoaded(true);
    });
    return () => { cancelled = true; };
  }, [desktop, incoming.content, incoming.updatedAt, input.chapter?.id, input.novelId, publish]);

  useEffect(() => {
    const next = receiveChapter(current.current, incoming, input.busy);
    if (next !== current.current) publish(next);
  }, [incoming.content, incoming.updatedAt, input.busy, publish]);

  const persistDraft = useCallback((content: string) => {
    if (!desktop || !chapterId || !chapterUpdatedAt) return Promise.resolve(false);
    const sequence = ++saveSequence.current;
    setLocalDraftStatus("saving");
    const operation = saveQueue.current.then(async () => {
      try {
        const saved = await saveChapterDraft({
          novelId: input.novelId,
          chapterId,
          clientId: clientId.current,
          content,
          baseOfficialVersion: chapterOfficialVersion,
          baseUpdatedAt: chapterUpdatedAt,
          expectedRevision: durableRevision.current,
        });
        durableRevision.current = saved.localRevision;
        if (mounted.current && sequence === saveSequence.current && current.current.content === content) {
          setLocalDraftStatus("saved");
        }
        return true;
      } catch {
        if (mounted.current && sequence === saveSequence.current && current.current.content === content) {
          setLocalDraftStatus("error");
        }
        return false;
      }
    });
    saveQueue.current = operation.catch(() => false);
    return operation;
  }, [chapterId, chapterOfficialVersion, chapterUpdatedAt, desktop, input.novelId]);

  // Local-draft autosave only; it never creates an official revision.
  useEffect(() => {
    if (!desktop || !durableLoaded || !chapterId) return;
    if (draft.content === draft.savedContent) return;
    const timer = setTimeout(() => { void persistDraft(current.current.content); }, 600);
    return () => clearTimeout(timer);
  }, [chapterId, desktop, durableLoaded, draft.content, draft.savedContent, persistDraft]);

  // Best-effort flush when the WebView is hidden or unloaded. Tauri async writes can
  // still be interrupted by SIGKILL; the 600ms debounce window is the documented gap.
  useEffect(() => {
    if (!desktop) return;
    const flush = () => {
      if (current.current.content !== current.current.savedContent) {
        void persistDraft(current.current.content);
      }
    };
    const onVisibility = () => { if (document.visibilityState === "hidden") flush(); };
    window.addEventListener("pagehide", flush);
    window.addEventListener("beforeunload", flush);
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      flush();
      window.removeEventListener("pagehide", flush);
      window.removeEventListener("beforeunload", flush);
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, [desktop, persistDraft]);

  useEffect(() => {
    if (!isDirty && !input.busy) return;
    const protect = (event: BeforeUnloadEvent) => { event.preventDefault(); event.returnValue = ""; };
    window.addEventListener("beforeunload", protect);
    return () => window.removeEventListener("beforeunload", protect);
  }, [isDirty, input.busy]);

  return {
    contentDraft: draft.content,
    savedContent: draft.savedContent,
    savedUpdatedAt: draft.savedUpdatedAt,
    isDirty, hasExternalChange, restored, storageError,
    localDraftStatus,
    localDraftLoaded: durableLoaded,
    localDraftConflict,
    localDraftRevision: durableRevision.current,
    flushDraft: () => persistDraft(current.current.content),
    setContentDraft: (content: string) => publish({ ...current.current, content }),
    acknowledgeSave: (submitted: string, saved: { content?: string | null; updatedAt: string }) => {
      publish(acknowledgeChapterSave(current.current, submitted, { content: normalizeChapterText(saved.content), updatedAt: saved.updatedAt }));
      setRestored(false);
      if (desktop && input.chapter) {
        void clearChapterDraft({
          novelId: input.novelId,
          chapterId: input.chapter.id,
          clientId: clientId.current,
          submittedContent: submitted,
          localRevision: durableRevision.current,
        }).catch(() => undefined);
      }
    },
    loadSavedVersion: () => {
      publish({ content: incoming.content, savedContent: incoming.content, savedUpdatedAt: incoming.updatedAt });
      setRestored(false);
    },
  };
}
