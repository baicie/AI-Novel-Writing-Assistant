# P14.5 Progress

Start HEAD: `27cf1bf5`

| Phase | Status | Commit | Evidence |
| --- | --- | --- | --- |
| A Audit | IMPLEMENTED | 07842a2c | safe stale-scheduler cleanup |
| B Crash/10 chapters | IMPLEMENTED | ac6bed5d / c3ca0e11 | 10 chapters Native PASS; opened-before-bind reopen recovery PASS |
| C Task Center | NOT_STARTED | — | — |
| D Native Export | PARTIAL | previous | range/book; volume/native dialog missing |
| E Index Retry Native | PARTIAL | previous | Rust worker only |
| F Multi-window draft | PARTIAL | previous | no two-window E2E |
| G Structured Memory | NOT_STARTED | — | — |
| H Acceptance | IN_PROGRESS | — | full suite/build |

## Native evidence

- batch.spec.mjs: create, 3 chapters Native approve, 10 chapters Native approve, pause/resume, cancel — PASS.
- batch restart runner: SIGKILL, new process, complete 3 chapters — PASS.

## Remaining

- Exact open_run→bind SIGKILL harness.
- Task Center, Native export save/volume, Index Retry Native, multi-window, structured memory.

## 2026-10-26 00:10 Final

- workspace: 285 passed
- clean all-targets: 344 passed
- Native batch: 5 PASS (create, 3 chapters, 10 chapters, pause/resume, cancel)
- Batch two-process restart: PASS
- chapter history / export Native: PASS
- editor/candidate crash/restart: PASS
- production .app: 21.99 MiB; DMG 9.8 MiB; hdiutil VALID; audit PASS
- Commands 99 / Capabilities 99
- Exact open_run→bind SIGKILL harness, Task Center, Native Save/Volume, Native Index Retry, two-window restore, structured memory remain incomplete.

## 2026-10-26 00:40

- exact crash window test `p145_batch_crash_binding`: open_run committed, binding absent, reopen, replay same command, one Run binded — PASS.
- workspace: 286 passed.
- all-targets in progress.
- Native H/E/F/G/I phases still not started or partial.

## 2026-10-26 01:05 Final

- workspace: 286 passed
- clean all-targets: 345 passed
- exact open_run-before-bind recovery test: PASS
- Native batch 5 PASS; batch two-process restart PASS
- production .app 21.99 MiB; DMG 9.8 MiB; audit PASS; commands 99/99
- not started: Task Center, Native Save/volume, Native Index Retry, multi-window, structured memory
