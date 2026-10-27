# P15.2 Progress

Start HEAD: `5e009c20`

| Stage | Status | Commit | Evidence |
| --- | --- | --- | --- |
| S0 Audit | IMPLEMENTED | — | real Git/source baseline |
| S1 Backup Restore | IMPLEMENTED / VERIFIED（启动恢复） | 4acd0350 | Native request + startup apply; crash-between-move tests; no in-process restart E2E |
| S2 Multi-window | PARTIAL | — | no real second Native window |
| S3 Batch SIGKILL | VERIFIED（open→bind） | d44caf87 | child process barrier + real SIGKILL + same Run/Command reuse |
| S3 Unknown/Reject/Repair | BLOCKED / PARTIAL | — | not implemented |
| S4 Index SIGKILL | VERIFIED（backend） | f5a191b7 | real worker SIGKILL, fencing, reindex, FTS hit |
| S5 Native Save | PARTIAL | inherited | OS dialog manual not run |
| S6 Migration failures | PARTIAL | 832b088e inherited | normal upgrade only |
| S7 Journey | PARTIAL | — | split modules |
| S8 Performance | PARTIAL | inherited | scale metrics |
| S9 Acceptance | IN_PROGRESS | — | full suite |

## Notes

- Batch barrier is compiled only under `debug_assertions`; worker binary requires `test-harness`.
- Backup restore is staged before restart and applied in setup before any SQLite/Runtime/scheduler starts.
- Index crash test now additionally verifies FTS search after recovery.

## 2026-10-27 final

- 297 workspace / 356 all-targets passed.
- Backup restore startup protocol tests PASS; Native request PASS.
- Batch real process SIGKILL PASS; index worker SIGKILL + FTS PASS.
- Production 23.19 MiB app, 10 MiB DMG, 107/107, audit PASS.
- Multi-window and Batch Unknown/Reject/Repair still blocked.
