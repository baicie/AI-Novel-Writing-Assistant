-- P17.2 P1.3：选区改写的计费状态记录
-- 只记录「这次请求走到了哪一步」，不保存正文、提示词或任何密钥。
-- sending = 已提交、可能已发出；进程在请求中途退出时会停在这里，读出来就是「结果未确认」。

CREATE TABLE IF NOT EXISTS chapter_selection_rewrite_attempt (
    request_id TEXT PRIMARY KEY,
    novel_id TEXT NOT NULL,
    chapter_id TEXT NOT NULL,
    state TEXT NOT NULL,
    error_summary TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_selection_rewrite_attempt_chapter
    ON chapter_selection_rewrite_attempt(novel_id, chapter_id, updated_at);
