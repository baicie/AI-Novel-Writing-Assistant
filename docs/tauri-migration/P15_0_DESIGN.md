# P15.0 Design Summary

## Baseline

Actual start HEAD: `612fe766` (includes P14.9). Worktree contains only the protected untracked `crates/novel-http/tests/integration_runs.rs`.

## Authority and state

- Workflow state remains `workflow_control`; frozen `RunContract`; command id/request hash idempotency.
- Batch owns only scheduling rows; Run binding uses lease/claim epoch/version.
- Chapter official body remains `chapter_revision`; local drafts are per client identity.
- Structured story facts own domain facts; `memory_conflict` remains the only conflict authority.
- Context reads confirmed facts with current source revision and As-Of visibility; rejected/disputed/stale facts are excluded.
- Index jobs own index status; production app runs the index worker and expired-lease recovery.
- Backup uses SQLite `VACUUM INTO`; restore copies a validated backup into an isolated location.

## This sprint additions

- SQLite busy/locked errors are no longer reported as storage corruption.
- Chapter history UI can compare a historical revision against the current official revision.
- Existing fact version CAS, context As-Of and relationship termination from P14.9 are inherited.

## Remaining failure windows

Native OS Save Dialog interaction, real two-window Native editing, worker SIGKILL/lease takeover, Batch Unknown/Reject/Repair, exact open→bind process SIGKILL, old-schema upgrade, full shared-fixture journey.
