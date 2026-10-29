# P14.8 Progress

Start HEAD: `25cc15b5`

| Phase | Status | Commit | Evidence |
| --- | --- | --- | --- |
| A Memory Audit | IMPLEMENTED / VERIFIED | 88a5c5eb | fact-pair validation, context integration, context exclusion test |
| B Native Save | PARTIAL | previous | dialog + atomic writer; OS dialog not WDIO |
| C Index Retry | VERIFIED (failure + retry + FTS) | 97450076 | injected failure, Native retry, FTS search PASS; SIGKILL not run |
| D Multi-window | PARTIAL | previous | backend draft tests only |
| E Batch Faults | PARTIAL | previous | reopen test, 10 chapters/restart |
| F Memory UI | IMPLEMENTED / PARTIAL | 737465ae | alias/relationship UI; conflict panel |
| G Performance | NOT_STARTED | — | — |
| H Native Journey | PARTIAL | — | task/export/memory linked |
| I Migration/Beta | PARTIAL | — | additive migration, no old-DB upgrade test |
| J Acceptance | IN_PROGRESS | — | final suite pending |

## A findings

- Structured conflict resolution now validates fact ids: exactly two, same novel, same entity/type, old confirmed active, new proposed/disputed, new source revision current.
- Rejected/disputed/stale facts are excluded from `list_structured_facts_for_context`.
- Context assembly now includes confirmed active structured facts with source trace.
- Remaining: structured facts have no per-fact version CAS; conflict CAS is on memory_conflict version.

## Not done

- Native save OS dialog automation, index failure injection, real two-window Native test, batch Unknown/Reject/Repair, 100/500/1000 benchmark, full journey, old DB migration test.

## 2026-10-26 final

- A: structured facts now enter Context only when confirmed/active with current source revision; rejected/disputed/stale excluded.
- C: Native index failure injection, retry from source panel, and FTS search verified. `reset_index_job_for_retry` now accepts `retry_wait`; a matching ready document completes the retry with its current revision rather than being marked obsolete.
- Full workspace: 292 passed. Clean all-targets: 351 passed.
- Native save OS dialog, two-window, batch Unknown/Reject/Repair, performance still incomplete.
