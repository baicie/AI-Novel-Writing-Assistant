-- P16.2B：凭据变更意图表（跨 SQLite 与密钥库的补偿日志）
-- 注意：这里只记录「要做什么」，绝不记录密钥正文。

CREATE TABLE IF NOT EXISTS credential_intent (
    id TEXT PRIMARY KEY,
    provider_id TEXT NOT NULL,
    secret_ref TEXT NOT NULL,
    action TEXT NOT NULL,
    state TEXT NOT NULL,
    attempts INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_credential_intent_state ON credential_intent(state);
