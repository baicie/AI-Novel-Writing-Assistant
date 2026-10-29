ALTER TABLE knowledge_document ADD COLUMN source_revision TEXT;
ALTER TABLE knowledge_document ADD COLUMN source_content_hash TEXT;
ALTER TABLE knowledge_document ADD COLUMN index_version INTEGER NOT NULL DEFAULT 1;
ALTER TABLE knowledge_document ADD COLUMN indexed_at INTEGER;
ALTER TABLE knowledge_chunk ADD COLUMN source_revision TEXT;
ALTER TABLE knowledge_chunk ADD COLUMN chunk_hash TEXT;
