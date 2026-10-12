CREATE TABLE schema_migration (
  version TEXT PRIMARY KEY,
  applied_at INTEGER NOT NULL
);

CREATE TABLE novel (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'draft',
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE volume (
  id TEXT PRIMARY KEY,
  novel_id TEXT NOT NULL REFERENCES novel(id),
  title TEXT NOT NULL,
  sort_order INTEGER NOT NULL,
  summary TEXT NOT NULL DEFAULT '',
  UNIQUE (novel_id, sort_order)
);

CREATE TABLE chapter (
  id TEXT PRIMARY KEY,
  novel_id TEXT NOT NULL REFERENCES novel(id),
  volume_id TEXT REFERENCES volume(id),
  title TEXT NOT NULL,
  sort_order INTEGER NOT NULL,
  official_revision_id TEXT,
  protection TEXT NOT NULL DEFAULT 'none' CHECK (protection IN ('none', 'user_edited', 'published')),
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE (novel_id, sort_order)
);

CREATE TABLE chapter_revision (
  id TEXT PRIMARY KEY,
  chapter_id TEXT NOT NULL REFERENCES chapter(id),
  version INTEGER NOT NULL,
  origin TEXT NOT NULL CHECK (origin IN ('user', 'ai_candidate', 'promoted')),
  content TEXT NOT NULL,
  content_sha256 TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  UNIQUE (chapter_id, version)
);

CREATE TABLE chapter_candidate (
  id TEXT PRIMARY KEY,
  chapter_id TEXT NOT NULL REFERENCES chapter(id),
  run_id TEXT NOT NULL,
  step_id TEXT NOT NULL,
  content TEXT NOT NULL,
  content_sha256 TEXT NOT NULL,
  interrupted INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE world_fact (
  id TEXT PRIMARY KEY,
  novel_id TEXT NOT NULL REFERENCES novel(id),
  body TEXT NOT NULL,
  source TEXT NOT NULL,
  valid_from INTEGER,
  valid_until INTEGER,
  trust TEXT NOT NULL,
  created_at INTEGER NOT NULL
);

CREATE TABLE character (
  id TEXT PRIMARY KEY,
  novel_id TEXT NOT NULL REFERENCES novel(id),
  name TEXT NOT NULL,
  role TEXT NOT NULL DEFAULT '',
  profile_json TEXT NOT NULL DEFAULT '{}',
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE character_state (
  id TEXT PRIMARY KEY,
  character_id TEXT NOT NULL REFERENCES character(id),
  chapter_order INTEGER,
  body TEXT NOT NULL,
  created_at INTEGER NOT NULL
);

CREATE TABLE timeline_event (
  id TEXT PRIMARY KEY,
  novel_id TEXT NOT NULL REFERENCES novel(id),
  chapter_order INTEGER,
  body TEXT NOT NULL,
  created_at INTEGER NOT NULL
);

CREATE TABLE workflow_run (
  id TEXT PRIMARY KEY,
  novel_id TEXT NOT NULL REFERENCES novel(id),
  plan_version TEXT NOT NULL,
  contract_json TEXT NOT NULL,
  created_at INTEGER NOT NULL
);

CREATE TABLE workflow_control (
  run_id TEXT PRIMARY KEY REFERENCES workflow_run(id),
  novel_id TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('queued', 'running', 'waiting_approval', 'paused', 'succeeded', 'failed', 'cancelled')),
  version INTEGER NOT NULL DEFAULT 0,
  pause_json TEXT,
  gate_json TEXT,
  cursor_step_id TEXT,
  failure_reason TEXT,
  lease_owner TEXT,
  lease_until INTEGER,
  stop_signal_json TEXT
);

CREATE UNIQUE INDEX one_active_run_per_novel
ON workflow_control(novel_id)
WHERE status IN ('queued', 'running', 'waiting_approval', 'paused');

CREATE TABLE workflow_command (
  command_id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES workflow_run(id),
  kind TEXT NOT NULL,
  request_hash TEXT NOT NULL,
  result_json TEXT NOT NULL,
  created_at INTEGER NOT NULL
);

CREATE TABLE workflow_event (
  run_id TEXT NOT NULL REFERENCES workflow_run(id),
  seq INTEGER NOT NULL,
  kind TEXT NOT NULL,
  step_id TEXT,
  payload_json TEXT NOT NULL,
  occurred_at INTEGER NOT NULL,
  PRIMARY KEY (run_id, seq)
);

CREATE TABLE workflow_artifact (
  id TEXT PRIMARY KEY,
  novel_id TEXT NOT NULL REFERENCES novel(id),
  run_id TEXT REFERENCES workflow_run(id),
  type TEXT NOT NULL,
  scope TEXT NOT NULL,
  version INTEGER NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('draft', 'confirmed', 'user_edited', 'stale')),
  protected_user_content INTEGER NOT NULL DEFAULT 0,
  payload_json TEXT NOT NULL,
  content_sha256 TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  UNIQUE (novel_id, type, scope, version)
);

CREATE TABLE workflow_step_attempt (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES workflow_run(id),
  step_id TEXT NOT NULL,
  phase TEXT NOT NULL CHECK (phase IN ('started', 'remote_started', 'candidate_saved', 'committed', 'interrupted')),
  request_id TEXT,
  candidate_id TEXT,
  resolution TEXT,
  control_version INTEGER NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE quality_debt (
  id TEXT PRIMARY KEY,
  novel_id TEXT NOT NULL REFERENCES novel(id),
  run_id TEXT NOT NULL REFERENCES workflow_run(id),
  chapter_order INTEGER NOT NULL,
  code TEXT NOT NULL,
  detail TEXT NOT NULL,
  created_at INTEGER NOT NULL
);

CREATE TABLE llm_invocation (
  id TEXT PRIMARY KEY,
  request_id TEXT NOT NULL UNIQUE,
  run_id TEXT,
  step_id TEXT,
  provider TEXT NOT NULL,
  model TEXT NOT NULL,
  input_summary TEXT NOT NULL,
  started_at INTEGER NOT NULL,
  finished_at INTEGER,
  prompt_tokens INTEGER,
  completion_tokens INTEGER,
  estimated_cost_micros INTEGER,
  status TEXT NOT NULL
);

CREATE TABLE usage_ledger (
  id TEXT PRIMARY KEY,
  invocation_id TEXT NOT NULL REFERENCES llm_invocation(id),
  novel_id TEXT,
  estimated_cost_micros INTEGER,
  note TEXT NOT NULL,
  created_at INTEGER NOT NULL
);

CREATE TABLE knowledge_document (
  id TEXT PRIMARY KEY,
  novel_id TEXT NOT NULL REFERENCES novel(id),
  owner_type TEXT NOT NULL,
  title TEXT NOT NULL,
  body TEXT NOT NULL,
  version INTEGER NOT NULL,
  index_status TEXT NOT NULL,
  created_at INTEGER NOT NULL
);

CREATE TABLE knowledge_chunk (
  id TEXT PRIMARY KEY,
  document_id TEXT NOT NULL REFERENCES knowledge_document(id),
  ordinal INTEGER NOT NULL,
  body TEXT NOT NULL,
  terms TEXT NOT NULL,
  UNIQUE (document_id, ordinal)
);

CREATE VIRTUAL TABLE knowledge_fts USING fts5(
  chunk_id UNINDEXED,
  terms,
  tokenize = 'unicode61'
);

CREATE TABLE embedding (
  chunk_id TEXT PRIMARY KEY REFERENCES knowledge_chunk(id),
  model TEXT NOT NULL,
  dimensions INTEGER NOT NULL,
  normalized INTEGER NOT NULL,
  vector_blob BLOB NOT NULL
);

CREATE TABLE retrieval_trace (
  id TEXT PRIMARY KEY,
  novel_id TEXT NOT NULL,
  query_summary TEXT NOT NULL,
  chunk_ids_json TEXT NOT NULL,
  created_at INTEGER NOT NULL
);

CREATE TABLE prompt_asset (
  id TEXT NOT NULL,
  version TEXT NOT NULL,
  family TEXT NOT NULL,
  template TEXT NOT NULL,
  PRIMARY KEY (id, version)
);

CREATE TABLE provider_credential (
  id TEXT PRIMARY KEY,
  provider TEXT NOT NULL,
  secret_ref TEXT NOT NULL,
  endpoint TEXT NOT NULL,
  created_at INTEGER NOT NULL
);

CREATE TABLE legacy_id_map (
  legacy_table TEXT NOT NULL,
  legacy_id TEXT NOT NULL,
  new_table TEXT NOT NULL,
  new_id TEXT NOT NULL,
  content_sha256 TEXT,
  PRIMARY KEY (legacy_table, legacy_id)
);

CREATE TABLE migration_run (
  id TEXT PRIMARY KEY,
  source_path TEXT NOT NULL,
  source_commit TEXT NOT NULL,
  started_at INTEGER NOT NULL,
  finished_at INTEGER,
  status TEXT NOT NULL,
  report_json TEXT NOT NULL
);

CREATE TABLE document_version (
  id TEXT PRIMARY KEY,
  novel_id TEXT NOT NULL REFERENCES novel(id),
  kind TEXT NOT NULL,
  version INTEGER NOT NULL,
  body TEXT NOT NULL,
  content_sha256 TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  UNIQUE (novel_id, kind, version)
);

INSERT INTO schema_migration(version, applied_at) VALUES ('0001', 0);
