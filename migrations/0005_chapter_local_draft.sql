-- Local author drafts are intentionally separate from chapter_revision.
CREATE TABLE chapter_local_draft (
  novel_id TEXT NOT NULL,
  chapter_id TEXT NOT NULL,
  client_id TEXT NOT NULL,
  content TEXT NOT NULL,
  base_official_version INTEGER NOT NULL,
  base_updated_at TEXT NOT NULL,
  local_revision INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  PRIMARY KEY (novel_id, chapter_id, client_id)
);
