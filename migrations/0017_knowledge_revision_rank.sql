-- Source revision rank lets index writes reject stale cross-process writers
-- without relying on a process-local lock.
ALTER TABLE knowledge_document ADD COLUMN source_revision_rank INTEGER NOT NULL DEFAULT 0;
