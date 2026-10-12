ALTER TABLE provider_credential ADD COLUMN model TEXT NOT NULL DEFAULT '';

ALTER TABLE provider_credential ADD COLUMN allow_localhost_http INTEGER NOT NULL DEFAULT 0;

INSERT INTO schema_migration(version, applied_at) VALUES ('0002', 0);
