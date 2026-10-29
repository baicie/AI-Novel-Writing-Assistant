# P14.5 Code Review

## P0/P1 findings

### P0 — Old Scheduler could cancel a Run adopted by a new Scheduler

- File: `crates/novel-application/src/batch.rs` bind conflict branch.
- Reproduction: A opens Run, loses lease; B claims Batch and binds the same deterministic Run; A's bind fails and previously cancelled the Run unconditionally.
- Root cause: binding failure was treated as proof of orphan status.
- Fix:
  - never cancel on lost fence;
  - re-read Batch; only if status is `cancelled` and no Item references the Run may the stale scheduler clean it up through the legal Run cancel command;
  - cancel command now closes the Batch boundary first, then resolves the deterministic open command via `workflow_command` and cancels only an unbound Run;
  - no direct RunControl writes.
- Tests: batch repository idempotency/fence; Native batch create, 3/10 chapters, cancel, pause/resume, two-process restart.
- Residual: exact A/B interleaving is not deterministically injected in a two-process Native test.

### P1 — Cancel cleaned Run before closing scheduling boundary

- Fix: set Batch cancelled first, then resolve/cancel current or open-intent Run.
- Residual: if the process dies after setting cancelled but before orphan cancellation, an unbound Run can remain; needs a persisted cleanup intent for full crash safety.

### P1 — B0 index worker test flake

- File: `index_worker_crash.rs`.
- Fix: bounded spawn retries around environment-level DB open failures; SIGKILL/fencing assertions unchanged.
- Commit: `beed6efc`.

### P1 — Batch UI remount/invalidation

- Fix: persistent BatchWorkspace, background active-run polling, post-action invalidation.
- Commit: `7225dbac`.

## Not implemented / residual

- Exact SIGKILL between `open_run` commit and Item binding.
- Two independent Scheduler processes.
- Reject/Repair/Repair-budget/RemoteStarted Unknown Batch tests.
- Task Center (NOT_STARTED).
- Native Save Dialog / volume export (PARTIAL/NOT_STARTED).
- Index Retry Native (PARTIAL).
- Multi-window restore (PARTIAL).
- Structured Story Memory (NOT_STARTED).
