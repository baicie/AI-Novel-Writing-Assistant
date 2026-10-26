# P14.9 Acceptance

- A: UNRESOLVED — lease flake not reproduced; no code change.
- B: IMPLEMENTED/VERIFIED — per-fact version, concurrent conflict CAS, As-Of/context alignment, rejected/disputed exclusion, A→B→A and relationship termination/re-establish tests.
- C: PARTIAL — native save dialog not OS-automated; multi-window Native not run.
- D: PARTIAL — index failure/retry/FTS verified; index SIGKILL, batch Unknown/Reject/Repair, exact process SIGKILL not.
- E: IMPLEMENTED/PARTIAL — relationship termination semantics and button; no Native termination E2E.
- F: IMPLEMENTED/PARTIAL — existing 100/500/1000 synthetic benchmark executed; dataset lacks all new domain tables.
- G: IMPLEMENTED/PARTIAL — VACUUM INTO consistent backup, restore copy, migration-idempotent reopen; old-schema upgrade and interrupted backup/restore not run.
- H: PARTIAL — modules pass separately, no one shared data-dir journey.
- I: IN_PROGRESS — final all-targets/build pending.

## Remaining

Native OS save dialog, two-window draft, index worker SIGKILL/lease takeover, batch Unknown/Reject/Repair, exact open→bind process SIGKILL, full journey, old DB upgrade.
