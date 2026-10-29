CREATE TABLE memory_extraction_job (
  id TEXT PRIMARY KEY,
  command_id TEXT NOT NULL UNIQUE,
  novel_id TEXT NOT NULL,
  chapter_id TEXT NOT NULL,
  chapter_version INTEGER NOT NULL,
  source_content_hash TEXT NOT NULL,
  extraction_version TEXT NOT NULL,
  provider TEXT NOT NULL,
  model TEXT NOT NULL,
  endpoint TEXT NOT NULL,
  status TEXT NOT NULL,
  error TEXT,
  raw_output TEXT,
  usage_json TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE TABLE memory_extraction_candidate (
  id TEXT PRIMARY KEY,
  job_id TEXT NOT NULL REFERENCES memory_extraction_job(id),
  novel_id TEXT NOT NULL,
  chapter_id TEXT NOT NULL,
  chapter_version INTEGER NOT NULL,
  source_content_hash TEXT NOT NULL,
  fact_type TEXT NOT NULL,
  truthfulness TEXT NOT NULL,
  entity_name TEXT,
  entity_id TEXT,
  value TEXT NOT NULL,
  evidence_quote TEXT NOT NULL,
  evidence_start INTEGER,
  confidence REAL,
  status TEXT NOT NULL,
  ambiguous INTEGER NOT NULL DEFAULT 0,
  version INTEGER NOT NULL DEFAULT 0,
  decision_reason TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE INDEX memory_candidate_chapter ON memory_extraction_candidate(novel_id, chapter_id, status);
CREATE INDEX memory_job_chapter ON memory_extraction_job(novel_id, chapter_id, created_at);
