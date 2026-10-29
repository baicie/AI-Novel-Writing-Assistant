# Release Gates

| Gate | Status | Reason |
| --- | --- | --- |
| G1 Fact conflict atomic | VERIFIED | fact version CAS + transaction tests |
| G2 Context As-Of | VERIFIED | temporal/source/rejected tests |
| G3 Relationship history | PARTIAL | backend + As-Of; no Native termination E2E |
| G4 Native Save | PARTIAL | OS dialog not automated; writer tested |
| G5 Multi-window | PARTIAL | backend only |
| G6 Index recovery | PARTIAL | failure→retry→FTS PASS; SIGKILL absent |
| G7 Batch reliability | PARTIAL | 10 chapters/restart; Unknown/Reject/Repair absent |
| G8 Performance | PARTIAL | scale/context/FTS numbers only |
| G9 Native journey | PARTIAL | split module coverage |
| G10 RC package | PARTIAL | unsigned internal app/DMG; no Beta sign-off |

Decision: `BETA_RC_BLOCKED`.
