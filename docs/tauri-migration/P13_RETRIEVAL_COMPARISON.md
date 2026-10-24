# P13-C3 FTS / Vector / Hybrid Retrieval Comparison

## 状态：PARTIAL

数据集：`p13-large-v1`，50 问题（40 正例 + 10 无答案），同一 50 章冻结文档。

Embedding：**deterministic mock-64 hash embedding**（本地、无付费调用）。  
因此只验证排序/融合管道，不能声称真实语义检索能力。

## 结果

| 策略 | Recall@10 | MRR | nDCG@10 |
| --- | ---: | ---: | ---: |
| FTS / Lexical | 1.00 | 0.86 | 0.86 |
| Vector（mock-64） | 0.70 | 0.48 | 0.50 |
| Hybrid RRF | 1.00 | 0.83 | 0.85 |
| Hybrid + Context Policy | 1.00 | 0.83 | 0.85 |

- 无答案：FTS 10/10 正确为空；
- Vector/Hybrid 的无答案需要 score 阈值，而当前 `SearchHit` 不返回分数，记为 N/A；
- Future leakage：本次冻结查询未检出 forbidden future 文档；
- Source revision accuracy：沿用 p13-large-benchmark-v1 的 FTS 校验 1.00。

## 结论

在冻结的 mock-64 条件下：

- FTS 的 MRR 高于 Hybrid；
- Hybrid 带来召回但未改善排序；
- 不将 Hybrid 设为默认；
- 默认保留 FTS；Vector/Hybrid 仅在引入真实 Embedding 后重新评估。

## 未完成

- 真实本地/预计算 Embedding；
- Precision@5/@10、Stale Index Leakage、Entity Disambiguation 的完整表；
- 人工语义标注和更难的 v2；
- 生成质量合同。
