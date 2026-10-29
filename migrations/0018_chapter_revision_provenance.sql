-- Revision provenance stays additive. The historical origin CHECK remains intact.
ALTER TABLE chapter_revision ADD COLUMN origin_detail TEXT NOT NULL DEFAULT '';
ALTER TABLE chapter_revision ADD COLUMN restored_from_revision_id TEXT;
ALTER TABLE chapter_revision ADD COLUMN command_id TEXT;
ALTER TABLE chapter_revision ADD COLUMN restored_by TEXT;
CREATE UNIQUE INDEX idx_chapter_revision_command ON chapter_revision(command_id) WHERE command_id IS NOT NULL AND command_id != '';
