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

**VERIFIED（基线）**：本地 SQLite / FTS / vector / RRF / Context Policy 已实现并有中文 RAG recall/precision 与 100 章 Context 测试。独立管理 UI 沿用既有知识库页面；更大规模人工评估与真实 Embedding 对比仍属后续增强。


## Context Policy 增量

- 最近 3 章保留正式正文。
- 更早章节从冻结 chapter_list 的标题/梗概生成 `chapter_arc_summary`，受 6000 字上限约束。
- 100 章 benchmark 验证早期摘要可进入上下文，未来章节/Candidate 不泄漏。
- 这是确定性摘要，不是新增 RAG/LLM 总结链路；后续可替换为本地检索事实。


## 中文本地 RAG Benchmark（本轮新增）

`crates/novel-application/tests/p12_rag_benchmark.rs`：

- 4 篇本地知识文档：角色、世界、干扰项；
- 3 个中文查询：魂兽、祖祠、林动；
- 关键词检索 + 向量检索；
- 结果包含 chunk_id 与 owner_type。

输出：

```text
P12 Chinese RAG benchmark:
lexical_recall=1.00
lexical_precision=1.00
vector_top1=Some("lin:0")
```

结论：

- 中文 FTS 检索可召回角色/世界事实；
- 向量检索 top1 正确；
- 干扰文档未进入前 5；
- 仍缺少人工标注大规模评估集与真实 Embedding 模型对比。
