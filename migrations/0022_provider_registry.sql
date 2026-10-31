-- P16.1 Provider / Model registry（纯增量，老版本忽略新列与新表）

ALTER TABLE provider_credential ADD COLUMN protocol TEXT NOT NULL DEFAULT 'openai-compatible';
ALTER TABLE provider_credential ADD COLUMN display_name TEXT;
ALTER TABLE provider_credential ADD COLUMN is_builtin INTEGER NOT NULL DEFAULT 0;
ALTER TABLE provider_credential ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0;
ALTER TABLE provider_credential ADD COLUMN updated_at INTEGER NOT NULL DEFAULT 0;

CREATE TABLE IF NOT EXISTS provider_model (
    provider_id TEXT NOT NULL,
    model_id TEXT NOT NULL,
    display_name TEXT,
    context_window INTEGER,
    max_output_tokens INTEGER,
    supports_streaming INTEGER,
    supports_tools INTEGER,
    supports_reasoning INTEGER,
    input_modalities TEXT,
    reasoning_efforts TEXT,
    source TEXT NOT NULL DEFAULT 'manual',
    user_overrides TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (provider_id, model_id)
);

CREATE INDEX IF NOT EXISTS idx_provider_model_provider ON provider_model(provider_id);

CREATE TABLE IF NOT EXISTS model_binding (
    task_type TEXT PRIMARY KEY,
    provider_id TEXT NOT NULL,
    model_id TEXT NOT NULL,
    reasoning_effort TEXT,
    updated_at INTEGER NOT NULL
);
