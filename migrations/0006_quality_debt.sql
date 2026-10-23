-- Chapter quality debt is append-only history. Resolving it never deletes the issue.
CREATE TABLE chapter_quality_debt (
  id TEXT PRIMARY KEY,
  novel_id TEXT NOT NULL,
  chapter_id TEXT NOT NULL,
  run_id TEXT NOT NULL,
  issue_id TEXT NOT NULL,
  issue_type TEXT NOT NULL,
  severity TEXT NOT NULL,
  description TEXT NOT NULL,
  evidence TEXT NOT NULL,
  suggestion TEXT NOT NULL,
  source_ref TEXT,
  review_version INTEGER NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('open', 'resolved', 'accepted')),
  resolution_evidence TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE (run_id, issue_id)
);
CREATE INDEX chapter_quality_debt_chapter ON chapter_quality_debt(chapter_id, status);
