# P14.6 Design

## Task Center

`TaskCenterService` remains a projection. New `NativeTaskRepository` queries existing `workflow_control`, `batch_run`, `knowledge_index_job`, `memory_extraction_job`, `quality_debt`; no new task state table. `NativeTaskSummary` exposes type/status/actions/route. Actions are navigation-only because the product task center is read-only; source pages execute domain commands.

## Export

Formal revisions only. Volume export filters chapter rows by persisted `chapter.volume_id`, ordered by `volume.sort_order`, `chapter.sort_order`, stable id. Native save uses Tauri dialog plugin; the selected path is passed to a Rust atomic writer (sibling temp file, flush/sync, rename). Frontend never supplies an arbitrary path.

## Structured memory

Additive migration `0020_structured_story_fact.sql`: stable `entity_id`, type, structured JSON, source chapter/revision, evidence, truthfulness, effective order, status, superseded_by. Author confirmation writes `confirmed`; prior active facts at or before the new order are superseded in one transaction. As-of queries return the latest confirmed fact at or before target order and require the source revision to still be the official revision. No AI direct-confirm path.
