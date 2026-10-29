# P14.3 Progress

## 2026-10-25

Start HEAD: `f0140edd`

| Phase | Status | Evidence |
| --- | --- | --- |
| A Audit/Architecture | IMPLEMENTED | `P14_3_ARCHITECTURE.md` |
| B Batch Domain | IMPLEMENTED | migration 0019, 2 repository tests |
| C Scheduler | IMPLEMENTED / PARTIAL | background tick, idempotent open, E2E 3 chapters |
| D Native Workspace | IMPLEMENTED | BatchWorkspace, UI E2E |
| E Task Center | NOT_STARTED | — |
| F Export | PARTIAL | range/whole; volume/native save missing |
| G History Safety | PARTIAL | provenance/draft; multi-window tests missing |
| H Full Acceptance | IN_PROGRESS | native 3-chapter PASS; final suite pending |
| I Index Retry | PARTIAL | Rust SIGKILL; Native WDIO missing |
| J Story Memory | NOT_STARTED | — |

## E2E evidence

`batch.spec.mjs`:

- create batch for 3 chapters PASS;
- 3 chapters run through existing gates; 3 distinct Run IDs; 3 official revisions; full-book export PASS;
- cancel current batch PASS.

## Known limitations

- Batch pause/resume E2E missing.
- Batch-level Skip/Retry missing.
- Batch crash window deterministic SIGKILL not yet added.
- 10 chapter stress not run.
- Task Center not unified.

## 2026-10-25 19:45 Final

- Native 3-chapter batch E2E: PASS
  - 3 distinct Run IDs
  - 3 official chapter revisions
  - approval executed through Native `director-approve-button`, not direct Rust test writes
  - full-book export after completion contains chapters
- Batch cancel through existing Run cancel: PASS
- Batch domain: 2 repository tests PASS
- Full workspace: 285 passed
- Clean worktree all-targets: 344 passed
- editor/candidate crash/restart: PASS on final UI
- production .app: 21.96 MiB
- DMG: 9.7 MiB; hdiutil VALID; audit PASS
- Commands 99 / Capabilities 99
- Main remaining gap: batch restart/resume E2E; pause E2E; deterministic crash window; 10-chapter stress; Task Center.

## 2026-10-25 19:50

- Native batch UI approval E2E: 3 passing
  - create batch via Native button
  - each chapter approved via Native approve button
  - 3 official revisions and export verified
  - cancel through Native button PASS
- HEAD code: `1e3b109f` (test-only after be8ad002)
- production app built from code `be8ad002`; 21.96 MiB / DMG 9.7 MiB
