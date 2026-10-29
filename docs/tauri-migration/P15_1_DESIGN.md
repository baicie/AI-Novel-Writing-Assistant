# P15.1 Design Notes

## Historical migration

`MIGRATION_SCRIPTS` is now a single reproducible list. A `test-harness`-only constructor opens a database with a chosen migration prefix, allowing a real historical fixture (pre-structured-facts, through 0019) to be upgraded by the normal production `open` path. The test checks schema version, integrity, foreign keys, official revision content, local draft and idempotent reopen.

## Backup

Production backup uses SQLite `VACUUM INTO` to a sibling temp file under the application data `backups/` directory, then renames the completed backup. The Native UI can create, list and verify backups; verification opens read-only and runs `PRAGMA integrity_check`. A restore UI/transaction is not yet implemented, so this is a backup safety feature, not full restore product UX.

## Remaining

Multi-window Native, Batch process SIGKILL/Unknown/Reject/Repair, Index Worker SIGKILL/lease takeover, OS Save Dialog manual acceptance, full shared-fixture journey and complete old-schema fault matrix.
