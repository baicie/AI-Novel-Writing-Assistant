# P14.7 Acceptance

| Phase | Status | Evidence |
| --- | --- | --- |
| A Audit | IMPLEMENTED | temporal supersede vs source invalidation split verified |
| B Memory Conflict | VERIFIED | structured location/life conflicts reuse memory_conflict; atomic resolve tests + Native conflict E2E |
| C Alias / Relationship | IMPLEMENTED / PARTIAL | directed relationship + alias backend/UI; coexistence test; no relation-termination UI |
| D Native Save | PARTIAL | dialog + atomic writer implemented; OS dialog not WDIO automated |
| E Index Retry | PARTIAL | Native page/worker exists; no failure-injection E2E |
| F Multi-window | PARTIAL | no two-window E2E |
| G Batch Reliability | PARTIAL | 10 chapters/restart/reopen; Unknown/Reject/Repair missing |
| H Performance | NOT_STARTED | — |
| I Native Journey | PARTIAL | task/export/memory integrated; no one-shot journey |
| J Final Acceptance | IN_PROGRESS | full suite finalizing |

## Key answers

1. Location/life conflict adjudication: yes, same effective chapter incompatible facts become a proposed fact + memory_conflict; author can confirm new/keep existing/mark disputed/reject.
2. Early As-Of: yes; latest confirmed at/before target, source revision must still be official. Temporal supersede no longer removes earlier history from as-of.
3. Source revision invalidation: yes; changed official revision removes fact from valid list/as-of until reconfirmed.
4. Alias does not merge people: yes; alias is a stable-entity attribute; same string can belong to different entities.
5. Directed relationship: yes; relation + targetEntityId; mentor_of and ally_of coexist, not auto-mirrored.
6. Native Save dialog: implemented; OS dialog automation not available. Manual steps are required.
7. Index Retry actual indexing: existing Worker/UI; no injected-failure Native E2E.
8. Two-window draft: not verified.
9. Batch Unknown: not verified.
10. 1000 chapters: not tested.
