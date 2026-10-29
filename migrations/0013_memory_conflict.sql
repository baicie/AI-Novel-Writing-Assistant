CREATE TABLE memory_conflict (
  id TEXT PRIMARY KEY,
  novel_id TEXT NOT NULL,
  conflict_type TEXT NOT NULL,
  entity_key TEXT NOT NULL,
  chapter_id TEXT NOT NULL,
  chapter_order INTEGER NOT NULL,
  candidate_ids_json TEXT NOT NULL,
  fact_ids_json TEXT NOT NULL DEFAULT '[]',
  status TEXT NOT NULL DEFAULT 'open',
  version INTEGER NOT NULL DEFAULT 0,
  description TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE TABLE memory_conflict_decision (
  id TEXT PRIMARY KEY,
  conflict_id TEXT NOT NULL REFERENCES memory_conflict(id),
  command_id TEXT NOT NULL UNIQUE,
  decision TEXT NOT NULL,
  chosen_candidate_id TEXT,
  reason TEXT,
  operator TEXT NOT NULL,
  conflict_version INTEGER NOT NULL,
  created_at INTEGER NOT NULL
);
CREATE INDEX memory_conflict_novel ON memory_conflict(novel_id, status);
