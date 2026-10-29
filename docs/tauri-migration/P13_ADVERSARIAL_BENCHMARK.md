# P13.8 Large Adversarial Retrieval Benchmark

## 状态：PARTIAL

本轮新增 `p13-large-v1` 合成标注集：

- 50 章连续故事文档；
- 50 个问题：40 个正例 + 10 个无答案；
- 4 类：线索 20、地点 10、别名 10、无答案 10；
- 每个问题记录 query、expected chunk、target chapter、forbidden future plan；
- 文档带固定 `source_revision`，另有 5 个 future_plan 干扰源。

## 结果

```text
dataset=p13-large-v1
chapters=50
questions=50
answered=40
recall@10=1.00
precision@5=0.20
mrr=1.00
ndcg=1.00
source_revision_accuracy=1.00
future_leak=0
no_answer_accuracy=1.00
```

分类：

| 类别 | cases | hit | recall |
|---|---:|---:|---:|
| clue | 20 | 20 | 1.00 |
| location | 10 | 10 | 1.00 |
| alias | 10 | 10 | 1.00 |
| no_answer | 10 | 0 (correctly empty) | — |

JSON：`docs/tauri-migration/p13-large-benchmark-v1.json`

P14.0-D 增加 FTS5 `bm25` 排序与 `chunk_id` tie-break 后，该冻结集 MRR 从 0.76 提升到 1.00；这是排序修复，不代表真实语义检索能力。

## 重要限制

这不是完整的 P13.8 验收：

- 仅比较 Lexical/FTS；未实现 Vector、Hybrid RRF、Hybrid+Context Policy 的同快照对照；
- 问题是合成标注，核心是“唯一事实标签 + 共享人物/地点干扰”，不是人工语义标注；
- 未包含 50 章伏笔跨章回收、不可靠叙述者、倒叙等全部类别；
- 未建立 JSON/Markdown 之外的可复现实验脚本；
- 不将本结果外推为真实文学检索质量。

指标定义：

- recall@10：40 个正例中 expected chunk 出现在 top10；
- precision@5：命中正例按 1/5 计；
- MRR / nDCG：按 expected chunk 首次排名；
- no-answer：10 个查询检索结果为空即正确。
