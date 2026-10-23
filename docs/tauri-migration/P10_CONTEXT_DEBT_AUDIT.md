# P10.2 Context / Quality Debt Audit

## 1. Quality Debt

当前表：

- `UNIQUE(run_id, issue_id)`
- 状态 `open/resolved/accepted`
- resolution_evidence 只增不覆盖
- review_version 记录来源

修复：

- 同一审校中重复 `issueId`：
  - 完全相同问题去重
  - 内容不同则 validation error，不能合并不同问题
- 模型未给 id 时，ID 由 type + evidence + suggestion 稳定哈希生成；evidence 改变视为新问题，不静默合并。
- 跨审校关联仍依赖模型提供稳定 `issueId` 或 sourceRef；没有可靠 id 时会产生新 issue，这是保守行为。

未完成：

- 人工 accepted 状态命令/UI
- issue 解决证据详情
- 新审校对旧 open issue 的自动关联策略

## 2. Context 审计

- Context 来源来自 SQLite/Artifact 读取；读取失败向上传播。
- 未来章节、候选正文、跨小说不进入 Context。
- `chapter-draft-v1` 旧语义保留；`chapter-draft-v2` 才使用 context。
- 输入预算 `context_token_budget` 与输出 `token_budget` 独立。
- 超预算：必要来源错误，可选来源 `over_budget`，不会无限追加。
- Context Trace 只存 hash/摘要 + promptText 于受权 artifact；普通日志只记录 redacted prompt 前 80 字符。
- promptText 按创作敏感数据对待；当前无额外导出/加密，依赖本地 SQLite 权限。
- 正式 Revision、候选、Local Draft 三套来源分离。
- CJK 1 token/字符只是估算；不宣称对应所有 Provider 的精确 window。

## 3. 测试

- P9 context 3 tests
- P9 context flow 1 test
- duplicate issue ID unit test
- P10 full workspace 233 tests

## 4. 未完成

- 完整旧 Context Broker/RAG
- 真实 tokenizer 精确计数
- Context artifact 的敏感数据加密/导出审计
- accepted quality debt 生命周期
