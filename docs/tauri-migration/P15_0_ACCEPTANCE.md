# P15.0 Acceptance

**Gate: BETA_RC_BLOCKED**

Verified/implemented this sprint:
- SQLite busy/locked no longer mapped to StorageCorrupt; corrupt remains distinguishable.
- Chapter history diff Native E2E PASS.
- Existing 294 workspace / 353 all-targets baseline inherited; final suite refreshed at this HEAD in I1.
- Production .app/DMG rebuilt and audited.

Not verified:
- S1 lease root cause.
- Native OS Save Dialog.
- Two-window Native draft safety.
- Index Worker SIGKILL/lease takeover.
- Batch Unknown/Reject/Repair.
- Exact open_run→bind process SIGKILL.
- Old-schema upgrade.
- Full single-fixture journey.
