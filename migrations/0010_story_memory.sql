ALTER TABLE character_state ADD COLUMN source_revision TEXT;
ALTER TABLE character_state ADD COLUMN status TEXT NOT NULL DEFAULT 'confirmed';
ALTER TABLE character_state ADD COLUMN updated_at INTEGER NOT NULL DEFAULT 0;
ALTER TABLE timeline_event ADD COLUMN source_revision TEXT;
ALTER TABLE timeline_event ADD COLUMN status TEXT NOT NULL DEFAULT 'confirmed';
ALTER TABLE timeline_event ADD COLUMN participants_json TEXT;
ALTER TABLE timeline_event ADD COLUMN location TEXT;
ALTER TABLE timeline_event ADD COLUMN updated_at INTEGER NOT NULL DEFAULT 0;
CREATE TABLE plot_thread (
  id TEXT PRIMARY KEY,
  novel_id TEXT NOT NULL REFERENCES novel(id),
  title TEXT NOT NULL,
  body TEXT NOT NULL,
  opened_chapter_order INTEGER NOT NULL,
  resolved_chapter_order INTEGER,
  status TEXT NOT NULL DEFAULT 'open',
  source_revision TEXT,
  evidence_json TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
