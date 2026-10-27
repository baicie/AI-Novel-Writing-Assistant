# P15.1 Release Gates

| Gate | Status | Blocker |
| --- | --- | --- |
| old DB upgrade | VERIFIED (basic) | migration interruption matrix missing |
| two-window draft | PARTIAL | no Native double-window |
| Batch SIGKILL/Unknown | BLOCKED | process harness missing |
| Index SIGKILL | PARTIAL | Native process recovery not rerun |
| Native Save | PARTIAL | OS dialog manual not run |
| Backup/Restore | PARTIAL | no restore UI |
| Full journey | PARTIAL | no shared fixture |
| Performance | PARTIAL | app startup/export/RSS missing |
| RC audit | PARTIAL | final suite |

Decision: **BETA_RC_BLOCKED**.
