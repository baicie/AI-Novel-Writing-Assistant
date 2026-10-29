# P14.8 Code Review

## P1 — Structured facts never entered Context

- Files: `crates/novel-application/src/ports.rs`, `context.rs`, `crates/novel-storage-sqlite/src/novels.rs`.
- Root cause: P14.7 facts were displayed and adjudicated but Context Assembly only read legacy story facts.
- Fix: add `list_structured_facts_for_context`; only confirmed+active facts with current source revision and as-of selection are serialized as a context source. Context trace records `structured_story_facts`.
- Test: `context_only_uses_confirmed_active_facts_at_target`, plus existing `p9_context_flow`.
- Commit: `88a5c5eb`.

## P1 — Fact-only conflict decision trusted factIds blindly

- File: `crates/novel-storage-sqlite/src/memory.rs:953`.
- Root cause: resolver accepted any two fact ids; could mix novels/entities/types or resolve confirmed facts.
- Fix: require exactly two distinct ids, same novel/entity/type, old confirmed+active, new proposed+disputed, new source revision current; updates are CAS-conditioned.
- Test: `conflict_is_atomic_and_resolves_new_or_existing` (invalid path coverage remains partial).
- Commit: `88a5c5eb`.

## P1 — Retry did not accept retry_wait

- Files: `crates/novel-storage-sqlite/src/index_jobs.rs`, `src-tauri/src/commands/knowledge.rs`.
- Root cause: UI exposed Retry for `retry_wait`, storage only allowed `failed/obsolete`.
- Fix: include `retry_wait`; a ready document matching the job revision completes with evidence instead of being marked obsolete.
- Test: Native `test:index-retry` PASS with injected first failure, retry, FTS search.
- Commit: `97450076`.

## P2 / residual

- OS Save Dialog still not WDIO-automated; manual steps required.
- Index test does not SIGKILL the production worker; lease recovery/Takeover Native is not proven.
- Multi-window draft, batch Unknown/Reject/Repair, 100/500/1000 performance, old DB migration not run.
- Relationship termination UI absent.
