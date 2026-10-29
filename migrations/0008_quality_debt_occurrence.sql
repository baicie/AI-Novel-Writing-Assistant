-- Every review occurrence is retained separately from the stable quality-debt row.
CREATE TABLE review_issue_occurrence (
  id TEXT PRIMARY KEY,
  novel_id TEXT NOT NULL,
  chapter_id TEXT NOT NULL,
  run_id TEXT NOT NULL,
  model_issue_id TEXT,
  stable_issue_key TEXT NOT NULL,
  issue_type TEXT NOT NULL,
  severity TEXT NOT NULL,
  description TEXT NOT NULL,
  evidence TEXT NOT NULL,
  suggestion TEXT NOT NULL,
  source_ref TEXT,
  review_version INTEGER NOT NULL,
  created_at INTEGER NOT NULL
);
CREATE INDEX review_issue_occurrence_run ON review_issue_occurrence(run_id, review_version);
ALTER TABLE chapter_quality_debt ADD COLUMN decision_version INTEGER NOT NULL DEFAULT 0;
ALTER TABLE chapter_quality_debt ADD COLUMN accepted_reason TEXT;
