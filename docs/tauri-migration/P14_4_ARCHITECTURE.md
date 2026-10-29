# P14.4 Batch Reliability Architecture

## Audit answers

1. **Authority**: `workflow_control`/`workflow_run` own Run state; `chapter_revision` owns正文. `batch_run`/`batch_run_item` own Batch scheduling state. Batch never writes Run status or chapter正文.
2. **Binding transaction**: `BatchRepository::bind_item_run` updates `batch_run_item.run_id/status=running` and clears the scheduler lease in one SQLite transaction. `run_id` is UNIQUE.
3. **Crash after open_run before bind**: next tick finds `run_id IS NULL` and retries `open_run` with deterministic command id `batch:{batch}:item:{item}:open`; Workflow command replay returns the existing Run; then CAS binds.
4. **Lease isolation**: Run Worker lease is `workflow_control.lease_owner/until`; Batch Scheduler lease is `batch_run.lease_owner/until/claim_epoch`. They are independent.
5. **Cancel vs open**: cancel first uses existing Run cancel command, then CAS sets Batch cancelled. If a Run was opened while cancellation races, binding fails the lease/version fence and the orphan Run is cancelled via the legal command.
6. **Waiting approval**: Batch reads authoritative `RunDetailView`; only `succeeded` with official正文 advances. `waiting_approval` maps to Batch waiting, not success.
7. **Other windows**: Native list/detail polling reads SQLite; Batch actions re-read current version before command.
8. **RemoteStarted Unknown**: Batch never resumes a Run by creating a new command. It only observes existing RunControl; Unknown/HoldInterrupted remains manual.
9. **Frozen order**: Batch items persist ordered chapter IDs; later UI reordering doesn't change existing items.
10. **Failed chapter context**: next item only opens after previous item succeeded; failed item stops Batch.

## State machine

```text
batch: created -> running -> waiting_approval | paused | cancelled | failed | completed
item: pending -> running -> waiting_approval -> succeeded | failed | cancelled
```

## Safety invariants

- no auto-approval;
- deterministic open commandId;
- item/run unique binding;
- lease owner + claim_epoch + version CAS;
- one active Run per novel remains enforced by open_run;
- cancel uses existing domain command;
- no direct Batch writes from React.
