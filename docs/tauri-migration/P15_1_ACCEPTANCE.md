# P15.1 Acceptance

- G1: VERIFIED for 0019 → 0021 upgrade with existing data; fault injection during migration NOT_RUN.
- G2: PARTIAL; no two-window Native.
- G3: PARTIAL/BLOCKED; Batch process SIGKILL and Unknown not run.
- G4: PARTIAL; index worker crash test inherited, Native SIGKILL not rerun.
- G5: PARTIAL; OS Save Dialog manual.
- G6: IMPLEMENTED/PARTIAL; backup create/list/verify Native PASS; restore UI absent.
- G7: PARTIAL.
- G8: PARTIAL.
- G9: IMPLEMENTED / PARTIAL — 295 workspace, 354 all-targets, production audit PASS.

## Final evidence

- workspace 295 passed; all-targets 354 passed.
- historical 0019 fixture → 0021: PASS (integrity/fk/data/draft/idempotent reopen).
- Native backup create/list/verify: PASS.
- Native history diff, memory, index retry, task center, export, batch, restart: PASS.
- frontend 186 passed; typecheck/build PASS; fmt/clippy PASS.
- production 22.96 MiB app, 10 MiB DMG, 106 commands/106 capabilities, audit PASS.
- `BETA_RC_BLOCKED`: multi-window, Batch process SIGKILL/Unknown, Index SIGKILL, OS Save Dialog, restore UI and full journey remain.


Release decision remains **BETA_RC_BLOCKED**.
