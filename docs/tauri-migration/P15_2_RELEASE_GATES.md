# P15.2 Release Gates

| Gate | Status | Blocker |
| --- | --- | --- |
| Backup restore | VERIFIED (startup protocol) | no automatic restart UI |
| Multi-window draft | BLOCKED | no real two-window E2E |
| Batch open→bind SIGKILL | VERIFIED | — |
| Batch Unknown/Reject/Repair | BLOCKED | not tested |
| Index SIGKILL | VERIFIED（backend） | Native process rerun not done |
| Native Save | BLOCKED（manual） | OS dialog manual not run |
| Historical migration | PARTIAL | failure matrix missing |
| Full journey | BLOCKED | no shared fixture |
| 1000 desktop performance | PARTIAL | startup/RSS/export missing |

Decision: **BETA_RC_BLOCKED**.
