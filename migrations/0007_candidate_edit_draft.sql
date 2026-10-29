-- Unconfirmed author edits of an AI candidate, separate from chapter_local_draft.
CREATE TABLE candidate_edit_draft (
  novel_id TEXT NOT NULL,
  chapter_id TEXT NOT NULL,
  run_id TEXT NOT NULL,
  gate_id TEXT NOT NULL,
  client_id TEXT NOT NULL,
  base_candidate_id TEXT NOT NULL,
  base_content_sha256 TEXT NOT NULL,
  content TEXT NOT NULL,
  local_revision INTEGER NOT NULL,
  control_version INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  PRIMARY KEY (run_id, gate_id, client_id)
);
