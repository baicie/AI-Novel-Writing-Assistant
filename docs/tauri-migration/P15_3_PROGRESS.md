# P15.3 Progress

Start HEAD: `612e7b7d`

| Gate | Status | Commit | Evidence |
| --- | --- | --- | --- |
| Native Two-window | **BLOCKED** | — | WDIO Tauri driver reports only `main`; `newWindow` returns `main` (probe spec) |
| Batch Unknown | **BLOCKED** | — | no controllable hanging provider/counting test |
| Batch Reject/Repair | **PARTIAL** | inherited | existing UI; no Batch Native reject/repair E2E |
| Native Backup Restore | **VERIFIED** | f3d49d61 | request → SIGKILL → new process startup → restored content + protection backup |
| Index Native Recovery | **PARTIAL** | inherited | backend SIGKILL/FTS verified; Native process SIGKILL not rerun |
| Historical Migration | **PARTIAL** | inherited | 0019→0021; earlier/fault matrix absent |
| Native Save | **BLOCKED** | inherited | OS Save Dialog manual not run |
| Shared-fixture Journey | **BLOCKED** | — | no single-fixture E2E |
| Desktop Performance | **PARTIAL** | inherited | scale/Context/FTS only |
| Release Audit | **PARTIAL** | — | 297/356, production build |

## New in P15.3

- `run-tauri-backup-restore-e2e.sh`: real Native restore request, SIGKILL, restart, content check, protection backup check.
- Window probe confirms Tauri embedded WebDriver exposes only the main window; two-window Native automation is not available through current driver.

## Blockers

- Native two window cannot be automated by this Tauri WDIO driver; requires product-level multi-window support or different driver.
- Batch Unknown/Reject/Repair and Native index process recovery remain.
- OS Save Dialog manual.
