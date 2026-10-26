# P14.9 Progress

Start HEAD: `27182905`

| Phase | Status | Commit | Evidence |
| --- | --- | --- | --- |
| A Lease audit | UNRESOLVED | af5d254a | 12 sequential + 8 parallel p11 runs passed; one historical full-suite failure not reproduced |
| B Memory/Context | IMPLEMENTED / VERIFIED | 216e60f2 / 46d62e25 | fact version CAS, context As-Of, concurrent conflict and timeline tests |
| C Native Save/Window | PARTIAL | previous | OS dialog not automated; no two-window E2E |
| D Index/Batch faults | PARTIAL | 97450076 | index failure/retry/FTS PASS; SIGKILL/Unknown/Reject/Repair NOT_RUN |
| E Relationship UX | IMPLEMENTED / PARTIAL | 46d62e25 | termination fact, As-Of, Native button; Native termination E2E not run |
| F Performance | IMPLEMENTED / PARTIAL | existing | 100/500/1000 existing benchmark executed |
| G Migration/Backup | IMPLEMENTED / PARTIAL | af5d254a | VACUUM INTO backup + restore + reopen PASS; old-schema upgrade NOT_RUN |
| H Native Journey | PARTIAL | — | pieces pass; one shared fixture journey not built |
| I Acceptance | IN_PROGRESS | — | full/all-targets/build |

## Lease stress

`scripts/p11-lease-stress.sh` 12 rounds PASS. Eight parallel test-binary runs (24 test executions) PASS. One prior full-suite `StorageCorrupt` at `p11_lease_fence.rs:424` was not reproduced; underlying SQLite error text is hidden by AppError mapping. Status UNRESOLVED, not verified.

## Performance (existing p13 scale baseline, 2026-10-26)

- 100 chapters: build 284ms, context P50 4.58ms/P95 4.84ms, FTS P50 1.32ms/P95 2.57ms, DB 5.44MB.
- 500: build 1549ms, context P50 13.14ms/P95 13.56ms, FTS P50 9.72ms/P95 12.62ms, DB 25.2MB.
- 1000: build 2969ms, context P50 24.70ms/P95 26.09ms, FTS P50 19.0ms/P95 23.85ms, DB 49.9MB.
- These are Mock/SQLite synthetic measurements; not real model cost.

## 2026-10-27 final

- Workspace 294 passed; clean all-targets 353 passed.
- Native memory/index/task/export/batch/restart all PASS on current build.
- Production 22.76 MiB app, 10 MiB DMG, 103/103, audit PASS.
- Lease flake remains UNRESOLVED after 12 sequential and 8 parallel stress runs.
