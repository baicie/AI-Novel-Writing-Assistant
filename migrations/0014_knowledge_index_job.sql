CREATE TABLE knowledge_index_job (
  id TEXT PRIMARY KEY,
  command_id TEXT NOT NULL UNIQUE,
  novel_id TEXT NOT NULL,
  source_type TEXT NOT NULL,
  source_id TEXT NOT NULL,
  source_revision TEXT NOT NULL,
  index_version INTEGER NOT NULL,
  status TEXT NOT NULL DEFAULT 'pending',
  attempt_count INTEGER NOT NULL DEFAULT 0,
  max_attempts INTEGER NOT NULL DEFAULT 5,
  next_run_at INTEGER NOT NULL DEFAULT 0,
  last_error_category TEXT,
  last_error TEXT,
  lease_owner TEXT,
  lease_until INTEGER,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE INDEX knowledge_index_job_due ON knowledge_index_job(status, next_run_at);
CREATE INDEX knowledge_index_job_novel ON knowledge_index_job(novel_id, created_at);
