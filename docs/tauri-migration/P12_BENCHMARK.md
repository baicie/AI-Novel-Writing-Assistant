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

**PARTIAL**：确定性 Context Policy benchmark 通过；长篇小说连续生成与质量基线未实现。


## 10 章 Context Continuity Benchmark（本轮新增）

`crates/novel-application/tests/p12_long_context.rs`：

- 10 章小说；前 8 章有正式 Revision；
- 目标第 9 章；
- 未来第 10 章、未确认 Candidate 均存在；
- 真实 SQLite Context Assembly。

结果：

```text
P12 long-context benchmark:
recent_recall=3/3
earlier_omitted_by_policy=5
candidate_leak=0
future_leak=0
chars=915
trace=director-context-v1
```

说明：

- 当前 Context Policy 仅取最近 3 章正式正文；
- 更早章节没有摘要迁移，因此被策略省略；
- 这证明未来章节与 Candidate 不泄漏，但也暴露“长篇早期事实”缺口；
- 仍需 recent summary / arc summary / 事实检索来覆盖 10/50/100 章。
