# P13.3 Quality Debt Association & Adversarial Evaluation

## 状态：PARTIAL

本轮新增：

- 同一问题不同描述不改变稳定 key。
- 相同描述不同证据生成不同 key（保持独立问题）。
- Source 变化生成不同 key。
- 作者 accepted 后同一问题再次出现，不会静默重新 open。
- accepted ≠ resolved 回归。

仍未实施完整 adversarial retrieval 集与指标。

已有：

- Issue Occurrence
- 稳定 key：type + sourceRef + normalized evidence
- accepted decision_version CAS
- 跨两轮审校测试：4 occurrence → 2 debt
- accepted ≠ resolved

未实施：

- 同名不同人、改名、倒叙、不可靠叙述者、谣言/事实冲突、旧设定修改、跨 50 章伏笔等 adversarial fixtures。
- Recall@K / Precision@K / MRR / nDCG / Source Revision Accuracy / Future Leakage / Stale Index Leakage。
- Conflict Recall / False Positive Rate / Evidence Attribution Accuracy / Repair Residual Rate。
- 版本化 JSON/Markdown 评测报告。

不得用 ScriptedLlm clean 输出计算真实模型冲突发现率。


## 对抗检索基线（本轮）

`crates/novel-application/tests/p13_adversarial_retrieval.rs`：

- 同名不同人；
- 传闻 vs 存活事实；
- 未来计划文档；
- source revision 检查。
- owner 硬过滤阻止 future_plan 进入确认事实。

```text
P13 adversarial: cases=4 correct=4 mrr=1.00 future_leak=0
```

这是小型确定性 fixture，不代表完整 50 章伏笔/不可靠叙述者评测。
