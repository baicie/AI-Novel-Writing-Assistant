# P14.3 Native Multi-Chapter Architecture

## Main goal

Native 一次创建 3 章 Batch，按章节串行走既有单章 RunContract；每章独立生成、Review、Repair、Approval、Commit。

## Authority

| Data | Authority |
| --- | --- |
| Batch state | `batch_run` |
| Batch item/run binding | `batch_run_item` |
| Batch audit | `batch_run_event` |
| Chapter content | existing `chapter_revision` |
| Run state | existing `workflow_control` / `workflow_run` |
| Approval | existing single-run gate |
| Context | existing Context Assembly, confirmed chapters only |

Batch never writes chapter content or Run status directly. It calls `DirectorService::open_run`, and observes `RunDetailView` / `get_chapter`.

## State machine

```text
created -> running
running -> waiting_approval | paused | cancelled | failed | completed
waiting_approval -> running (after run succeeds / user action)
paused -> running (Resume)
terminal: cancelled / failed / completed
```

`batch_run_item`: pending -> running -> waiting_approval -> succeeded, or failed/cancelled.

## Schema

Migration `0019_batch_run.sql`:

- batch_run: command_id unique, request_hash, status, cursor, version, claim epoch, lease.
- batch_run_item: ordinal/chapter unique, run_id unique.
- batch_run_event: sequence events.

## Creation contract

- 2..=50 ordered chapter IDs, no duplicates;
- all chapters belong to novel (Tauri adapter validates);
- request hash includes ordered chapter IDs and full run request;
- transaction creates Batch + Items + event;
- same commandId+hash replays; same commandId different hash conflicts.

## Scheduler

Tauri background tick every 2s:

1. list runnable created/running/waiting_approval batches;
2. claim with owner + lease + claim_epoch;
3. current item:
   - succeeded -> advance cursor;
   - bound run -> inspect authoritative Run:
     - succeeded + official content -> item succeeded, advance;
     - waiting_approval -> batch waiting_approval;
     - paused -> paused;
     - failed/cancelled -> batch failed;
     - queued/running -> running;
   - no run -> prior item succeeded, no active novel run -> deterministic commandId
     `batch:{batch}:item:{item}:open` -> open_run -> bind run_id.

Crash after open_run before bind retries same commandId; `open_run` returns existing Run.

## Cancel/Pause

- Pause stops opening new Runs; current Run keeps existing semantics.
- Cancel calls existing Run cancel command for current item, then sets Batch cancelled.
- No direct Run status writes.

## Deferred / limitations

- No batch-level automatic approval (must use existing Run gate).
- Batch Retry/Skip and Task Center integration not implemented.
- Batch cancellation may race with an already-terminal Run; the existing command is idempotent.
