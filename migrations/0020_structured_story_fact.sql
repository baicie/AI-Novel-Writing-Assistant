CREATE TABLE structured_story_fact (
  id TEXT PRIMARY KEY,
  novel_id TEXT NOT NULL REFERENCES novel(id),
  entity_id TEXT NOT NULL,
  fact_type TEXT NOT NULL,
  value_json TEXT NOT NULL,
  source_chapter_id TEXT NOT NULL REFERENCES chapter(id),
  source_revision INTEGER NOT NULL,
  evidence_quote TEXT NOT NULL,
  truthfulness TEXT NOT NULL,
  effective_chapter_order INTEGER NOT NULL,
  status TEXT NOT NULL,
  superseded_by TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE INDEX structured_fact_as_of
  ON structured_story_fact(novel_id, entity_id, fact_type, status, effective_chapter_order);
CREATE INDEX structured_fact_chapter
  ON structured_story_fact(novel_id, source_chapter_id);
