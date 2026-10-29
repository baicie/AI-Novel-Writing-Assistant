# P15.4 Progress

Start HEAD: `a1e1592d`

| Gate | Status | Commit | Evidence |
| --- | --- | --- | --- |
| Batch Unknown Remote | BLOCKED | — | no hanging HTTP provider + cross-process request count |
| Batch Reject/Repair | PARTIAL | 862c61db | Native Reject stops Batch progression; no regenerate/repair E2E |
| Native Multi-window | BLOCKED | — | WDIO Tauri driver exposes only `main`; `newWindow` returns `main` |
| Native Backup Restore | VERIFIED（重启切换） | f3d49d61 | request → SIGKILL → restart → old content + protection backup |
| Backup WAL crash matrix | PARTIAL | — | VACUUM snapshot + startup switch; no full WAL boundary matrix |
| Native Index Restart | PARTIAL | inherited | backend SIGKILL/FTS; Native process not rerun |
| Shared Author Journey | BLOCKED | — | no single-fixture journey |
| Desktop Performance | PARTIAL | inherited | scale/Context/FTS only |
| Native Save | BLOCKED | inherited | OS Dialog manual not run |
| Production Audit | PARTIAL | inherited | 297/356, app/DMG audit |

## Notes

- Window probe (`window-probe.spec.mjs`) confirms only one handle and `newWindow` returns main.
- Reject test confirms Batch does not open chapter 2/3 after rejecting chapter 1.
- Backup restore E2E shares one data directory across a real process restart.
