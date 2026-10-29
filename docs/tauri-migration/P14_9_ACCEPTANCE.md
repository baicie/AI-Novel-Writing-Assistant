# P14.9 Acceptance

- A: UNRESOLVED — lease flake not reproduced; no code change.
- B: IMPLEMENTED/VERIFIED — per-fact version, concurrent conflict CAS, As-Of/context alignment, rejected/disputed exclusion, A→B→A and relationship termination/re-establish tests.
- C: PARTIAL — native save dialog not OS-automated; multi-window Native not run.
- D: PARTIAL — index failure/retry/FTS verified; index SIGKILL, batch Unknown/Reject/Repair, exact process SIGKILL not.
- E: IMPLEMENTED/PARTIAL — relationship termination semantics and button; no Native termination E2E.
- F: IMPLEMENTED/PARTIAL — existing 100/500/1000 synthetic benchmark executed; dataset lacks all new domain tables.
- G: IMPLEMENTED/PARTIAL — VACUUM INTO consistent backup, restore copy, migration-idempotent reopen; old-schema upgrade and interrupted backup/restore not run.
- H: PARTIAL — modules pass separately, no one shared data-dir journey.
- I: IMPLEMENTED / PARTIAL — 294 workspace, 353 all-targets, production PASS.

## Final evidence

- workspace: 294 passed; backup test separately 1 passed.
- clean all-targets: 353 passed, 0 failed.
- frontend: 186 passed; typecheck/build PASS.
- fmt/clippy PASS (warnings).
- Native memory 2 PASS; index retry 1 PASS; task center 1 PASS; export 3 PASS; batch 5 PASS; restart PASS.
- production 22.76 MiB app, 10 MiB DMG, audit PASS, 103/103.
- Lease flake UNRESOLVED.


## Remaining

Native OS save dialog, two-window draft, index worker SIGKILL/lease takeover, batch Unknown/Reject/Repair, exact open→bind process SIGKILL, full journey, old DB upgrade.
