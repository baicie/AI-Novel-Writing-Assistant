# P15.4 Acceptance

- Backup restore restart: **VERIFIED**.
- Batch Reject safe-stop: **PARTIAL** (Native test; no regeneration/repair continuation).
- Batch Unknown: **BLOCKED**; no request-count process harness.
- Native two-window: **BLOCKED** by current Tauri WDIO driver (only `main`).
- Index SIGKILL: **VERIFIED backend** (P15.2); Native restart not rerun.
- Native Save: **BLOCKED** (OS dialog manual).
- Full journey/performance: **PARTIAL/BLOCKED**.
- Release decision: **BETA_RC_BLOCKED**.
