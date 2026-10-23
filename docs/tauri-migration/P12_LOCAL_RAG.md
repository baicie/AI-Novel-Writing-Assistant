# P12.1 Local Knowledge / Chinese RAG / Context Policy

## 现状

已有本地 SQLite 知识库：

- `documents` / chunks / embedding
- FTS token 检索
- 向量 cosine
- RRF 融合与时间衰减
- `assemble` 显式来源优先、owner 硬过滤、预算截断
- Director Context 真实读取 SQLite、正式 Revision、人物/世界事实
- Trace 记录来源、选择/排除原因与 hash

P12.1 本轮改进：

- 中文 tokenizer 对孤立 CJK 单字保留 token，避免单字人名/生僻词在 FTS 查询中丢失。
- 新增中文 Context Policy benchmark：正式前文章节进入，未来章节和未确认 Candidate 被排除，预算不超限。

## 未完成

- 独立“本地知识库管理” UI（当前通过既有知识/世界/人物能力间接使用）。
- 更完整的中文分词/停用词/同义词词典。
- 独立 RAG 检索页面与人工召回评估工具。
- Embedding 的本地模型装配仍是既有 Provider 路径，未新增本地模型管理。

## 测试

```text
cargo test -p novel-context
5 passed

chinese_context_policy_benchmark_excludes_future_and_candidate
PASS
```

## 最终状态

**PARTIAL**：本地 SQLite/FTS/vector/RRF/Context Policy 已有基础；本轮补中文单字 token 与 Policy benchmark。完整本地知识库管理 UI 与中文词典未实现。
