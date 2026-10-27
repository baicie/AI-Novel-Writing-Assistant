# P15.5 Code Review

## P1 — Batch Unknown had no real request-count harness

- Files: `crates/novel-storage-sqlite/tests/p155_batch_unknown.rs`, `src/bin/unknown_worker.rs`.
- Fix: real local TCP server, child worker, persisted RemoteStarted barrier, SIGKILL, lease expiry, recovery.
- Result: one provider request; Run paused; no candidate; no chapter 3.
- Commit `5955e86b`.
- Residual: provider-received-before-local-persist window not tested.

## P2 — Native reject had no continuation proof

- File: `tests/desktop-e2e/specs/batch-reject.spec.mjs`.
- Result: Native reject stops progression; native regenerate+approve advances chapter 2.
- Commits `862c61db`, `4025b55d`.

## P2 — Real second window exists but interaction incomplete

- Files: `src-tauri/src/commands/windows.rs`, `window-command.spec.mjs`.
- Result: `editor-b` created; both `main` and `editor-b` visible to Tauri driver.
- Residual: second-window editor did not render content; no draft isolation/restart proof.
- Commit `a035d43d`.

## P2 — Writer identity

- File: `useChapterDraft.ts`.
- Change: stable writer id from `writer` query param (second window) before sessionStorage fallback.
- Not yet proven by two-window draft test.
