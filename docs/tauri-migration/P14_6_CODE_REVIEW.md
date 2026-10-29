# P14.6 Code Review

## P1 — Task Center contained mutating actions

- File: `client/src/pages/tasks/NativeTaskCenterPage.tsx`, `crates/novel-storage-sqlite/src/native_tasks.rs`
- Root cause: first Native projection exposed pause/resume/cancel/retry as task-center actions, conflicting with the project rule that the task center is a read-only record and mutations belong on the source page.
- Fix: projection now exposes only navigation actions; the Native page only refreshes/filters and navigates to the source page.
- Test: `test:task-center` asserts no mutation buttons and source navigation.
- Commit: `023bca15`.
- Residual: user request asked for task-center operations; project rule takes precedence. Mutations remain available on Director/Batch/index source pages.

## P1 — Workflow chapter id not a column

- File: `crates/novel-storage-sqlite/src/native_tasks.rs`
- Root cause: initial projection selected `r.chapter_id`, but workflow_run has no such column; chapter comes from the frozen contract JSON.
- Fix: `json_extract(r.contract_json, '$.targetChapterId')` with a left join.
- Test: `native_tasks` integration test.
- Commit: `c6a02e15`.

## P1 — As-of query lost superseded historical facts

- File: `crates/novel-storage-sqlite/src/structured_memory.rs`
- Root cause: as-of filtering required current `active` status, so a fact superseded at a later chapter disappeared when querying an earlier chapter.
- Fix: as-of selects the latest confirmed fact at or before target order regardless of later supersession; source revision must still be the current official revision.
- Test: `structured_facts_supersede_at_as_of_and_reject_stale_revision`.
- Commit: `7ee6a2c5`.

## P1 — ChapterSnapshot volume metadata missing

- Fix: additive optional `volume_id` in `ChapterSnapshot`, populated by chapter queries, with editor test constructor updated.
- Commit: `cc4b578d`, `b52a7a27`.

## P2 / not verified

- Native save dialog is implemented with Tauri dialog plugin; WDIO cannot automate the OS dialog. Rust atomic writer is tested; dialog interaction is not E2E verified.
- Native Index Retry UI exists; failure injection and SIGKILL Native E2E not run.
- Multi-window draft, batch Unknown/Reject/Repair, 100/500/1000 performance NOT_RUN.
- Structured alias/relationship facts are schema-compatible but not exposed in Native UI; location/life only.
