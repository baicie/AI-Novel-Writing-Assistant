-- Sync-A1-R2：修正历史调用记录的用量分类。
--
-- 0028 给 usage_state 加了 NOT NULL DEFAULT 'known'。SQLite 在执行 ADD COLUMN
-- 时把已有行全部填成默认值，于是 0027 之前的历史调用（包括 provider 未返回
-- usage、token 全为 NULL 的行）升级后都被当成「用量已确认」。
--
-- 0028 已经发布，不在原地改写；这里用一条新的版本化迁移做一次性回填。
--
-- 规则：只有 prompt_tokens 与 completion_tokens **都存在**的历史行才是 known。
-- 任一侧为 NULL 说明 provider 没有给出完整用量，属于 unknown——部分已知不等于
-- 完整已知，否则「输出未知」会长期被当成 0。
--
-- 边界：
--   * 只处理 audit_status = 'succeeded' 的行。0028 之后由新代码写入的
--     provider_failed / cancelled 等行已经带有正确分类，不在此列。
--   * 明确为 'unbilled' 的行不动。
--   * 不清空已有数值：降级只改变费用可见性，不丢数据。

UPDATE llm_invocation
SET usage_state = 'unknown'
WHERE usage_state = 'known'
  AND audit_status = 'succeeded'
  AND (prompt_tokens IS NULL OR completion_tokens IS NULL);
