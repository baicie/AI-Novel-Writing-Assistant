# P15.2 Acceptance

- S1: Backup restore request + startup switch; original/protection copies; startup crash recovery unit tests; Native request E2E. In-process automatic restart not implemented.
- S3: precise open_run→bind process SIGKILL PASS; one Run/one Item binding; same command ID.
- S4: real index worker SIGKILL, expiry recovery, new epoch, chunks/revision/FTS PASS.
- S2: Native two-window NOT_VERIFIED.
- S5: OS Save Dialog MANUAL_NOT_RUN.
- S7: shared-fixture journey NOT_VERIFIED.
- Final: 297 workspace, 356 all-targets, frontend 186, production 23.19 MiB, 107 commands/107 capabilities.
- Release decision: **BETA_RC_BLOCKED**.
