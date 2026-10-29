# P15.3 Acceptance

- Native backup restore: **VERIFIED** for request→SIGKILL→restart→staged switch→content/protection backup. WAL/checkpoint boundary is covered by SQLite VACUUM backup and startup tests, not a live WAL crash matrix.
- Native two-window: **BLOCKED** by Tauri WDIO driver (only one window handle; `newWindow` returns main).
- Batch exact open→bind SIGKILL: **VERIFIED** (P15.2).
- Batch Unknown/Reject/Repair: **PARTIAL/BLOCKED**.
- Index SIGKILL: **VERIFIED backend**; Native process restart not rerun.
- Native Save: **BLOCKED** (manual OS dialog).
- Complete journey/performance: **PARTIAL/BLOCKED**.

Release decision: **BETA_RC_BLOCKED**.
