# P15.2 Code Review

## P1 — Restore could not run while Runtime owned the DB

- Files: `src-tauri/src/commands/backup.rs`, `src-tauri/src/lib.rs`.
- Fix: restore is staged into a pending intent; applied in Tauri setup before SQLite/Runtime/scheduler open. Original DB moved to protection copy; schema/integrity checked; crash-between-move covered by tests.
- Tests: `startup_restore_switches_staged_database_and_keeps_protection_copy`, `crash_between_original_move_and_staged_switch_restores_original`, Native backup request.
- Commit: `4acd0350`.
- Residual: no automatic restart UI; staged-loss case leaves original but app may refuse startup.

## P1 — No true process-level Batch crash test

- Files: `batch.rs`, `batch_crash_worker.rs`, `p152_batch_process_sigkill.rs`.
- Fix: debug-only barrier after open_run commit; test-harness worker binary; child killed by parent; parent replays same command and binds existing Run.
- Commit: `d44caf87`.
- Residual: Unknown/Reject/Repair and scheduler race not covered.

## P1 — Index crash test did not verify search results

- File: `index_worker_crash.rs`.
- Fix: after recovery assert FTS keyword hit and source revision/chunk.
- Commit: `f5a191b7`.

## P2 / blockers

- Native two-window still absent.
- OS Save Dialog manual gate not run.
- Batch Unknown/Reject/Repair, migration failure matrix, shared-fixture journey, desktop performance metrics.
