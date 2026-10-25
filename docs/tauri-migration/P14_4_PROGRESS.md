# P14.4 Progress

Start HEAD: `1e3b109f` (actual worktree; reported 0aaf03a7 docs)

| Phase | Status | Evidence |
| --- | --- | --- |
| A Audit | IMPLEMENTED | P14_4_ARCHITECTURE.md |
| B Crash-safe binding | IMPLEMENTED | deterministic replay; E2E two-process restart |
| C Batch control | IMPLEMENTED | pause/resume/cancel Native E2E |
| D Native Batch | VERIFIED | 4 Native tests |
| E Task Center | NOT_STARTED | — |
| F Native Export | PARTIAL | existing range/book; volume/save dialog missing |
| G Index Retry | PARTIAL | Rust recovery; Native missing |
| H History Safety | PARTIAL | existing probe; multi-window tests missing |
| I Story Memory | NOT_STARTED | — |
| J Acceptance | IN_PROGRESS | full suite/build |

## Native batch E2E 2026-10-25

```text
batch create 3 chapters
3 chapters via Native approve buttons -> 3 distinct runs/revisions
cancel current batch
pause -> no new run -> resume -> cancel
process SIGKILL -> new process -> resume -> complete 3 chapters
```

## Safety fixes

- removed pre-open active-run shortcut that blocked crash replay;
- bind conflict cancels orphan Run through legal command;
- failed/cancelled Run marks Item terminal before Batch failure;
- cancel supports fresh-version CAS retry;
- persistent BatchWorkspace + background active-run polling.
