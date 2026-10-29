# P14.7 Design

## Structured facts and As-Of

`structured_story_fact` keeps stable `entity_id`, novel id, fact type, structured JSON, source chapter/revision, evidence, truthfulness, effective order, status and superseded_by.

Two independent mechanisms are kept apart:

- Temporal supersede: a later effective chapter replaces an earlier active fact for normal state progression. As-Of still returns the earlier fact when its effective order is at/before the target and no newer fact is visible.
- Source invalidation: every as-of/list query requires the fact's `source_revision` to match the chapter's current official revision. A changed chapter removes the old evidence from valid context even if the target is earlier.

As-Of selects the latest confirmed fact at/before target order with a deterministic tiebreak (`effective_order`, `updated_at`, `created_at`, `id`), and does not filter on current `status` so later temporal supersede does not erase earlier history.

## Conflict reuse

Structured facts reuse `memory_conflict` / `memory_conflict_decision`; no second conflict state machine. A same-effective-order incompatible fact is inserted as `proposed/disputed` and a `memory_conflict` row stores `fact_ids_json=[old,new]` with empty candidate list. Resolution runs in the existing transaction:

- `confirm_new` / `supersede_with_evidence`: old superseded, new confirmed active;
- `keep_existing` / `reject_extraction`: new rejected, old active;
- `mark_disputed`: new disputed, old active;
- CAS on conflict version, commandId/requestHash idempotency, source revision validation, decision history.

Location conflicts are same order + different location. Life conflicts are alive↔dead only; unknown never conflicts. Relationship conflicts are same target and ally/enemy inversion. Normal movement/order changes and resurrection across chapters are temporal progression, not same-position conflict.

## Alias / Relationship

Alias uses stable entity id plus `{alias}`; same alias can belong to different entities and is never auto-merged. Relationship uses `{relation,targetEntityId,targetName}` with directed types parent_of/sibling_of/ally_of/enemy_of/mentor_of/partner_of. Native UI exposes add/confirm for both; as-of applies identical source/effective-order rules.
