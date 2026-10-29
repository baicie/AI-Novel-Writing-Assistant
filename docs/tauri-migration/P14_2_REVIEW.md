# P14.2 Code Review Findings

## P0/P1

### P1 — Cross-process stale index write
- File: `crates/novel-storage-sqlite/src/knowledge.rs:53`
- Root cause: revision comparison happened before the write transaction; process-local lock did not protect a second SQLite kernel.
- Fix: additive `source_revision_rank` + conditional upsert + ready CAS (`migrations/0017`, `knowledge.rs:89`).
- Test: `second_process_cannot_overwrite_newer_revision`.
- Commit: `2dfc7fa8`.

### P1 — Unversioned revision could silently replace another
- File: `knowledge.rs:53`
- Root cause: both revisions ranked `0`, so CAS accepted any replacement.
- Fix: two unversioned differing revisions now Conflict; versioned cannot be replaced by unversioned.
- Test: `opaque_revisions_cannot_overwrite_without_ordering`.
- Commit: `6df88668`.

### P1 — Restore command ACL missing
- File: `src-tauri/capabilities/default.json:34`
- Symptom: `Command restore_chapter_revision not allowed by ACL`.
- Fix: capability allow entry; E2E PASS.
- Commit: `945ee55b` / `70982a05`.

## P2 / Residual

- S2 export uses browser Blob download, not a Tauri save dialog; no overwrite confirmation.
- S2 volume export and 500+ chapter stream write not implemented.
- S3/S4 batch orchestrator/workbench NOT_STARTED.
- S5 unified Task Center NOT_STARTED.
- S6 Native Index Retry WDIO missing; Rust Worker SIGKILL and cross-process rank tests exist.
- S7 structured memory NOT_STARTED.
- S8 full journey blocked on batch and task center.
- Restore UI exposes keep/reject draft; backend discard-own exists but UI has no discard action.
- Provenance `origin_detail` additive; old rows default empty and UI labels them author text.

## Transaction / CAS audit

- Restore: write-lock, official version CAS, draft decision, new revision, artifact, memory invalidation, knowledge sync in one transaction.
- Restore command_id partial unique index; replay returns current chapter without append.
- Export read-only, filtered by `novel_id` via Director list.
- Index write: rank CAS + process lock + final ready CAS.
