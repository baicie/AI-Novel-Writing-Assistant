# P15.5 Progress

Start HEAD: `c6617ab0`

| Gate | Status | Commit | Evidence |
| --- | --- | --- | --- |
| Batch Unknown Remote | **VERIFIED** | 5955e86b | real child worker + external TCP mock; request count 1; Run paused; no candidate; chapter 3 not started |
| Batch Reject/Regenerate | **PARTIAL** | 862c61db, 4025b55d | Native reject stops; Native regenerate+approve advances chapter 2 |
| Batch Repair | **NOT_STARTED** | — | no review/repair budget E2E |
| Native Multi-window | **PARTIAL** | a035d43d | real `editor-b` window and Tauri window API verified; second-window editor interaction not verified |
| Backup WAL/Restore | **PARTIAL** | f3d49d61 inherited | Native restart restore PASS; full WAL crash matrix not run |
| Native Index Recovery | **PARTIAL** | inherited | backend SIGKILL/FTS PASS; Native restart not rerun |
| Shared Author Journey | **BLOCKED** | — | no single-fixture complete journey |
| Desktop Performance | **NOT_STARTED** | — | no new 100/500/1000 desktop metrics |
| Native Save | **BLOCKED** | inherited | OS dialog manual not run |
| Production Audit | **IN_PROGRESS** | — | final suite/build |

## Gate 1 details

- `p155_batch_unknown.rs`: local TCP server receives real HTTP requests and never replies.
- Worker child process is SIGKILLed after `workflow_step_attempt.phase = remote_started`.
- Recovery worker pauses the Run; provider receives exactly the original request; no candidate; chapter 3 has no Run.
- Boundary: only the persisted RemoteStarted case is proven; not exactly-once for provider-received-but-not-persisted.

## Gate 2 details

- Native Reject test: reject candidate; Batch stays waiting; chapters 2/3 no Run.
- Native Regenerate test: reject → regenerate → approve → chapter 1 official revision and chapter 2 Run starts.
- Repair/quality-debt/repair-exhausted not run.

## Gate 3 details

- Product command creates a real Tauri window label `editor-b`; `list_app_windows` reports both labels; driver `browser.tauri.listWindows` also reports both.
- The second-window chapter editor did not render its editor in the test; no two-writer draft isolation claim.
- Writer identity now prefers a stable writer query param; backend CAS tests remain.

## 2026-10-27 final

- Batch Unknown exact request count == 1 verified; Run paused; no candidate; chapter 3 not started.
- Native reject→regenerate→approve advances chapter 2.
- Real Tauri `editor-b` window created and visible to Tauri window APIs; editor interaction not verified.
- 297 workspace / 356 all-targets; frontend 186; production 23.23 MiB, 110/110, audit PASS.
- Release gate remains BETA_RC_BLOCKED.
