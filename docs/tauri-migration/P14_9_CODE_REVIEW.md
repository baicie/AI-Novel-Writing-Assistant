# P14.9 Code Review

## P1 — Context As-Of used current fact status

- Files: `crates/novel-storage-sqlite/src/novels.rs`, `structured_memory.rs`.
- Root cause: Context required `status='active'`, so after a later temporal supersede an earlier chapter lost its historically valid fact.
- Fix: shared `visible_facts_at` selects latest single-valued fact per entity/type from all confirmed facts at/before target, while source revision validity is checked separately; relationships are selected per directed pair/type and terminated relationships omitted.
- Tests: `location_return_and_relationship_termination_follow_as_of`, `context_only_uses_confirmed_active_facts_at_target`.
- Commit: `46d62e25`.

## P1 — Relationship termination had no semantics

- Files: `structured_memory.rs`, `StructuredMemoryPanel.tsx`.
- Root cause: relationship facts were only additive; no termination fact, no per-pair latest selection.
- Fix: author-confirmed `{active:false}` relationship fact supersedes the matching active relation through the same effective order; As-Of/Context pick latest per directed pair and exclude termination records; Native button creates the termination fact.
- Test: timeline test (ally at 3, terminated at 8, re-ally at 12; As-Of 4/9/13).
- Commit: `46d62e25`.
- Residual: no Native click E2E for termination.

## P1 — No per-fact version CAS

- Files: migration `0021_structured_fact_version.sql`, `memory.rs`, `structured_memory.rs`.
- Root cause: only conflict version was CAS; two conflicts could race on the same fact.
- Fix: add `version`; fact transitions increment it and require expected status/truthfulness; old/new updates verify rows affected. Second conflict on a changed old fact fails and rolls back.
- Test: `concurrent_conflicts_do_not_lose_fact_updates`.
- Commit: `216e60f2`.

## P2 — Lease flake unresolved

- `p11_lease_fence` failed once with `StorageCorrupt` at line 424 in a full suite. 12 sequential + 8 parallel stress runs (24 executions) passed; underlying SQLite extended error is hidden by AppError mapping.
- Status: UNRESOLVED, no speculative fix.

## P2 / residual

- Native Save OS dialog not automated.
- Multi-window, index SIGKILL, Batch Unknown/Reject/Repair, exact process SIGKILL, full journey, old-schema migration not run.
- Relationship termination UI not Native-click tested.
