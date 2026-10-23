# P12.2 Long-form Continuity & Quality Benchmark

## 本轮状态

**PARTIAL（基线）**

已增加确定性 Context Policy benchmark：

- 3 个已确认前文章节；
- 1 个未来章节；
- 1 个未确认 Candidate；
- owner 硬过滤 + 预算截断。

断言：

- 前文章节全部被纳入（recall=3/3）；
- 未来章节 0；
- Candidate 0；
- 输出不超过预算。

这不是完整长篇小说质量 Benchmark：

- 尚未跑 10/50/100 章连续生成；
- 尚未统计跨章人物一致率、伏笔回收率、时间线冲突率；
- 尚未建立固定小说语料 baseline 与回归阈值。

## 下一步合同

- 固定长篇小说种子 + 10 章黄金大纲；
- 每章运行 Repair/Review；
- 记录 issue 类型分布、修复率、连续章 fact 命中；
- 以确定性 Mock Provider 与真实 Provider 分层；
- 保存结果 JSON 与基线差异。

## 最终状态

**PARTIAL**：100 章 Context Continuity baseline 与确定性 clean/repair/residual 质量 baseline 通过；真实长篇生成质量与阈值仍未实现。


## 100 章 Context Continuity Benchmark（本轮新增）

`crates/novel-application/tests/p12_long_context.rs`：

- 100 章小说；前 90 章有正式 Revision；
- 目标第 91 章；
- 未来第 100 章、未确认 Candidate 均存在；
- 真实 SQLite Context Assembly。

结果：

```text
P12 long-context benchmark:
recent_recall=3/3
early_arc_summary=true
candidate_leak=0
future_leak=0
chars=935
trace=director-context-v1
```

说明：

- 当前 Context Policy 仅取最近 3 章正式正文；
- 最近 3 章保留正式正文；
- 更早章节从冻结 chapter_list 摘要生成 `chapter_arc_summary`，100 章测试可召回前情节点；
- 未来章节与 Candidate 不泄漏；
- 仍需更完整的 arc/fact 检索与质量指标。


## 确定性质量 Baseline（本轮新增）

`p9_repair_flow::deterministic_quality_benchmark_clean_repair_and_residual`：

```text
P12 quality benchmark:
clean_commit=1
repaired_commit=1
residual_open_debt=2
```

覆盖：

- 干净 Review：正常 Commit；
- 可修复 Review：Repair 后二次 Review clean 并 Commit；
- 残留问题：不 Commit，质量债保持 open。

这是确定性 Mock 质量基线，不替代真实长篇生成质量评估。


## 10 章 Mock 连续生成 Benchmark（本轮新增）

`crates/novel-application/tests/p12_long_generation.rs`：

- 同一本小说、10 个章节；
- 每章独立 Run；
- 真实 Runtime / Worker / Gate / Candidate / Commit；
- ScriptedLlm 提供确定性章节正文与 clean Review；
- 不做真实付费模型调用。

输出：

```text
P12 long-generation benchmark:
chapters=10
commits=10
recent_context=true
repair_budget=0
```

断言：

- 10 章全部生成正式 Revision；
- 每章 Commit 1 次；
- 后续章节 Context 包含前章正文；
- Repair 预算 0 时不触发修复；
- 无残留错误 Commit。

仍不是完整质量 Benchmark：

- 未统计人物一致率/伏笔回收率/时间线冲突率；
- Mock 正文不评估文学质量；
- 尚未跑 50/100 章。
