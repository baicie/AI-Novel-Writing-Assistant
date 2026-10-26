# P14.8 Progress

Start HEAD: `25cc15b5`

| Phase | Status | Commit | Evidence |
| --- | --- | --- | --- |
| A Memory Audit | IMPLEMENTED | 88a5c5eb | fact-pair validation, context integration |
| B Native Save | PARTIAL | previous | dialog + atomic writer; OS dialog not WDIO |
| C Index Retry | PARTIAL | previous | worker/UI; no failure injection |
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
