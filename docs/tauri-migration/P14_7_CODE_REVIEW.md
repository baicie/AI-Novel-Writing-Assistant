# P14.7 Code Review

## P1 — As-Of conflated temporal supersede with source invalidation

- Files: `crates/novel-storage-sqlite/src/structured_memory.rs:109-150`.
- Root cause: as-of required current `active` status, so a fact superseded at a later chapter disappeared for earlier queries.
- Fix: as-of does not filter current status; it requires source revision to be the current official revision and picks the latest confirmed fact at/before target with stable tiebreak.
- Test: `structured_facts_supersede_at_as_of_and_reject_stale_revision`.
- Commit: `737465ae`/`fb1938ef`.

## P1 — Alias and relationship facts superseded each other

- File: `crates/novel-storage-sqlite/src/structured_memory.rs:156-260`.
- Root cause: generic supersede all active facts of same entity+type; adding a second alias or another directed relationship silently retired the first.
- Fix: only location/life_state are single-valued; alias/relationship coexist. As-of returns all active alias/relationship facts; location/life still latest.
- Test: `aliases_and_relationships_coexist_and_do_not_merge_people`.
- Commit: `fb1938ef`.

## P1 — Structured conflicts could not use fact-only resolution

- File: `crates/novel-storage-sqlite/src/memory.rs:519-575,953`.
- Root cause: existing resolver assumed at least one candidate; fact-only conflict had empty candidates.
- Fix: branch on empty candidate list; apply new/keep/dispute/reject to fact pair in the same transaction as conflict CAS/decision history.
- Test: `conflict_is_atomic_and_resolves_new_or_existing`.
- Commit: `737465ae`.

## P2 / residual

- Native Save dialog cannot be automated by WDIO; only Rust atomic writer and export content tested.
- Index Retry failure injection, multi-window drafts, batch Unknown/Reject/Repair, 100/500/1000 performance, relation termination UI not delivered.
- Task Center remains read-only per project rule; workflow actions stay on source pages.
