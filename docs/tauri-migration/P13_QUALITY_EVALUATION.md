# P13.3 Quality Debt Association & Adversarial Evaluation

## 状态：PARTIAL（沿用 P11.3 核心，P13.3 对抗集未实施）

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
