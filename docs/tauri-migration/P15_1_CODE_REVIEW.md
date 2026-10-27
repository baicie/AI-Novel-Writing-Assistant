# P15.1 Code Review

## P1 — Historical schema upgrade was untested

- File: `crates/novel-storage-sqlite/src/db.rs`, `tests/p151_historical_migration.rs`.
- Root cause: no historical fixture path; only latest-schema opens.
- Fix: single `MIGRATION_SCRIPTS` list; test-harness-only `open_at_migration`; fixture through 0019 with novel/revision/draft, then normal upgrade to 0021.
- Test: integrity_check, foreign_key_check, revision content, draft, idempotent reopen.
- Commit: `832b088e`.
- Residual: interrupted migration/schema corruption matrix NOT_RUN.

## P1 — Backup had no product entry

- Files: `src-tauri/src/commands/backup.rs`, `crates/novel-storage-sqlite/src/db.rs`, `BackupPanel.tsx`.
- Root cause: only test-level VACUUM INTO.
- Fix: app-data `backups/` directory, VACUUM INTO + temp/rename, list, read-only integrity verification; no arbitrary frontend path.
- Test: Native `test:backup` PASS.
- Commit: `edda2017`.
- Residual: no restore UI/transaction; backup interruption not tested.

## P2 / blockers

- Multi-window Native, Batch exact SIGKILL/Unknown/Reject/Repair, Index Worker SIGKILL/lease takeover, OS Save Dialog manual, full journey, old-schema interruption matrix.
- `p11_lease_fence` flake remains unresolved; SQLite busy/locked classification added earlier.
