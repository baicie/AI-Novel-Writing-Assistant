# P13.8 Large Adversarial Retrieval Benchmark

## 状态：PARTIAL

当前数据集：

- `p13-adversarial-v1`
- 8 cases
- 8 docs

指标：

```text
recall=1.00
precision=0.88
mrr=0.94
ndcg=0.94
source_revision_accuracy=1.00
future_leak=0
```

覆盖：

- 同名不同人、冒充者、传闻/事实、地点、别名、旧设定修改、已故回忆、未来计划。

未达到要求：

- 50 章、50 问题；
- 多核心人物、倒叙、 unreliable narrator、跨 50 章伏笔；
- 无答案问题；
- Lexical / Vector / Hybrid / Hybrid+Policy 对比；
- 生成质量合同。

不得将 8 case 结果当作完整 P13.8 完成。
