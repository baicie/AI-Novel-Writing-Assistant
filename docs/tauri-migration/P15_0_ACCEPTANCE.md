# P15.0 Acceptance

**Gate: BETA_RC_BLOCKED**

Verified/implemented this sprint:
- SQLite busy/locked no longer mapped to StorageCorrupt; corrupt remains distinguishable.
- Chapter history diff Native E2E PASS.
- Final: 295 workspace passed; 354 all-targets passed; history diff Native E2E PASS.
- Production .app/DMG rebuilt and audited.

Not verified:
- S1 lease root cause.
- Native OS Save Dialog.
- Two-window Native draft safety.
- Index Worker SIGKILL/lease takeover.
- Batch Unknown/Reject/Repair.
- Exact open_run→bind process SIGKILL.
- Old-schema upgrade.
- Full single-fixture journey.

## Final evidence

- `cargo test --workspace --exclude novel-http`: 295 passed.
- clean worktree all-targets: 354 passed.
- frontend: 186 passed; typecheck/build PASS.
- Native chapter history diff: PASS.
- Native structured memory: 2 PASS.
- Native index retry: 1 PASS.
- Native task center/export/batch/restart: PASS.
- production: 22.76 MiB app, 10 MiB DMG, hdiutil VALID, audit PASS, 103/103.
- Beta RC gate: **BETA_RC_BLOCKED**.
