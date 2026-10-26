# P15.0 Progress

Start (actual): `612fe766`

| Stage | Status | Commit | Evidence |
| --- | --- | --- | --- |
| S0 Baseline/Gap | IMPLEMENTED | — | actual Git + P14.9 inherited |
| S1 Lease/SQLite | UNRESOLVED | 02d2e589 | busy/locked classified; stress 24 executions pass; historical failure not reproduced |
| S2 Fact CAS/Context | IMPLEMENTED / VERIFIED | inherited 216e60f2 / 46d62e25 | fact version, concurrent conflict, As-Of/context tests |
| S3 Batch Unknown/Reject/Repair | PARTIAL | — | no new Native tests |
| S4 Batch exact SIGKILL | NOT_STARTED | — | SQLite reopen only |
| S5 Index SIGKILL/Fencing | PARTIAL | inherited | failure/retry/FTS PASS; SIGKILL no |
| S6 Multi-window Draft | PARTIAL | inherited | backend draft tests; no two-window Native |
| S7 Native Save Dialog | PARTIAL | inherited | Rust atomic writer; OS dialog manual |
| S8 Old DB Migration | PARTIAL | — | additive migrations; old fixture not built |
| S9 Backup/Restore | IMPLEMENTED / PARTIAL | inherited af5d254a | VACUUM INTO + restore test; no UI |
| S10 Relationship History | IMPLEMENTED / PARTIAL | inherited 46d62e25 | As-Of 4/9/13; no Native termination click |
| S11 Long Context | PARTIAL | inherited | structured fact context tests; no 30–50 chapter fixture |
| S12 Search/Diff | IMPLEMENTED / PARTIAL | 995d932a | history diff Native PASS; global body search absent |
| S13 Workspace UX | PARTIAL | — | inherited; no full UX audit |
| S14 Performance | IMPLEMENTED / PARTIAL | inherited | 100/500/1000 scale/FTS numbers |
| S15 Native Journey | PARTIAL | — | modules separately pass |
| S16 RC Packaging | IMPLEMENTED / PARTIAL | — | app/DMG/audit PASS; unsigned internal build |
| S17 Acceptance | IN_PROGRESS | — | final suite |

## P15 commits

- `995d932a` chapter history diff.
- `02d2e589` SQLite busy/locked error classification.

## Blockers

`p11_lease_fence` root cause UNRESOLVED; Native Save OS dialog, two-window draft, index SIGKILL, Batch Unknown, exact process kill, old DB upgrade and full journey not executed.

## 2026-10-27 final

- 295 workspace / 354 all-targets passed.
- Chapter history diff Native E2E PASS.
- SQLite busy/locked mapping added.
- Production RC app/DMG rebuilt.
- Release gate remains BETA_RC_BLOCKED.
