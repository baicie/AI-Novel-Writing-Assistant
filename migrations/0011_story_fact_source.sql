ALTER TABLE character_state ADD COLUMN source_chapter_id TEXT;
ALTER TABLE timeline_event ADD COLUMN source_chapter_id TEXT;
ALTER TABLE plot_thread ADD COLUMN source_chapter_id TEXT;
CREATE INDEX character_state_source_chapter ON character_state(source_chapter_id);
CREATE INDEX timeline_event_source_chapter ON timeline_event(source_chapter_id);
