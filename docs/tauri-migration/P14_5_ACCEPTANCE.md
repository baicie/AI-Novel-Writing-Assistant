# P14.5 Acceptance

Final HEAD: updated by this commit.

## Verified this round

- Batch Native 3 chapters: Native create + Native approve buttons + 3 distinct runs/revisions + export.
- Batch Native 10 chapters: 10 distinct run IDs and 10 official revisions.
- Batch process SIGKILL/restart: same data dir, new process, completed 3 chapters.
- Pause/resume/cancel: Native buttons; cancel does not open next run.
- Old Scheduler cleanup safety: no unconditional orphan cancellation; only cancelled+unbound Run cleanup via legal command.
- Cross-process index revision rank, chapter history restore, editor/candidate crash recovery.
- Full tests and production audit.

## Exact crash window

`p145_batch_crash_binding` verifies: open_run commit → no Item binding → close/reopen SQLite → replay deterministic command → one Run bound, no duplicate.

## Not verified

- Exact SIGKILL between open_run commit and Item bind: database-reopen recovery test PASS; real process SIGKILL at that exact point not injected.
- Two Scheduler process interleaving.
- Reject/Repair/Unknown Remote batch tests.
- Unified Task Center.
- Native Save Dialog / volume export.
- Native Index Retry.
- Multi-window draft restore.
- Structured Story Memory.
