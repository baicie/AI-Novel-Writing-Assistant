-- Durable sequential batch orchestration. Batch never owns chapter content.
CREATE TABLE batch_run (
  id TEXT PRIMARY KEY,
  novel_id TEXT NOT NULL REFERENCES novel(id),
  command_id TEXT NOT NULL UNIQUE,
  request_hash TEXT NOT NULL,
  request_json TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('created','running','waiting_approval','paused','cancelled','failed','completed')),
  cursor INTEGER NOT NULL DEFAULT 0,
  version INTEGER NOT NULL DEFAULT 0,
  lease_owner TEXT,
  lease_until INTEGER,
  claim_epoch INTEGER NOT NULL DEFAULT 0,
  failure_reason TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE INDEX idx_batch_run_novel ON batch_run(novel_id, created_at DESC);

CREATE TABLE batch_run_item (
  id TEXT PRIMARY KEY,
  batch_id TEXT NOT NULL REFERENCES batch_run(id),
  chapter_id TEXT NOT NULL REFERENCES chapter(id),
  ordinal INTEGER NOT NULL,
  run_id TEXT,
  status TEXT NOT NULL CHECK (status IN ('pending','running','waiting_approval','succeeded','failed','cancelled')),
  version INTEGER NOT NULL DEFAULT 0,
  last_error TEXT,
  started_at INTEGER,
  completed_at INTEGER,
  UNIQUE (batch_id, ordinal),
  UNIQUE (batch_id, chapter_id),
  UNIQUE (run_id)
);
CREATE INDEX idx_batch_item_batch ON batch_run_item(batch_id, ordinal);

CREATE TABLE batch_run_event (
  id TEXT PRIMARY KEY,
  batch_id TEXT NOT NULL REFERENCES batch_run(id),
  sequence INTEGER NOT NULL,
  event_type TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  UNIQUE (batch_id, sequence)
);
