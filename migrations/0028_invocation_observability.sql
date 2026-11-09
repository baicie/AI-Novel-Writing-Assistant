-- Upstream-Sync-A1：把「一次物理 Provider 请求」作为独立调用记录。
--
-- 背景：llm_invocation 原先按 step 记账，结构化修复请求被折叠进首次请求，
-- 失败请求完全不落库。费用因此无法按物理调用追溯。
--
-- 纯增量：
--   * audit_status  记录这次物理调用的结果分类，旧数据默认 'succeeded'；
--   * usage_state   known / unknown / unbilled，未知不能当 0 计费；
--   * 辅助列用于在运行详情里还原调用链，不保存提示词或响应正文。

ALTER TABLE llm_invocation ADD COLUMN attempt_id TEXT;
ALTER TABLE llm_invocation ADD COLUMN audit_status TEXT NOT NULL DEFAULT 'succeeded';
ALTER TABLE llm_invocation ADD COLUMN usage_state TEXT NOT NULL DEFAULT 'known';
ALTER TABLE llm_invocation ADD COLUMN duration_ms INTEGER;

CREATE INDEX IF NOT EXISTS idx_llm_invocation_run_started
    ON llm_invocation(run_id, started_at);
