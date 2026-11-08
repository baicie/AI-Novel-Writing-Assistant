# LLM 物理调用计费可追溯性

## 背景

一次章节生产里，模型请求不只发生一次：结构化步骤失败后会有一次格式修复请求，Provider 也可能在返回结果前超时或断开。这些请求在费用上彼此独立，但在运行记录里曾经被合并成一条，或者干脆不出现：

- 结构化修复调用的用量被累加进首次请求，`usage.calls` 恒为 1。用户看到的"1 次调用"其实是 2 次物理请求。
- Provider 返回 `Unavailable { billed: true }` 时没有任何记录。这类请求可能已经到达服务端并计费，但本地看不到。

结果是一样的：费用静默消失或偏低，用户无法判断一本小说到底调用了多少次模型、花了多少钱，也无法在"这一步很贵"时定位到具体是哪次请求。

## 决策

**一次物理 Provider 请求产生一条 `llm_invocation` 记录。** 记录的是调用事实，不是业务步骤结果。

因此：

- 结构化首次请求与格式修复请求各记一条，各自持有自己的 `request_id`。
- 失败的请求同样落库，包括可能已计费的失败。
- 结果分类与用量可见性是两个独立维度：结果可以失败，费用可以是"未知"。

把"结果"和"计费"分开，是因为它们回答不同问题。`status` 回答"这次调用发生了什么"，`usage_state` 回答"这次调用要不要计入费用汇总"。混在一起会让"未知"被迫选边站，通常就变成了 0。

## 当前规则

### 结果分类（`InvocationOutcome`）

`crates/novel-domain/src/observability.rs` 定义，`worker.rs::invocation_outcome_for_error` 把 `LlmError` 映射过去。这是对已结构化错误做确定性归类，不属于意图识别，也不参与重试、暂停或质量政策判断。

| 结果 | 触发条件 |
| --- | --- |
| `succeeded` | 首次请求成功返回 |
| `repaired` | 修复请求成功 |
| `repair_failed` | 修复请求返回但结构仍不合法 |
| `unauthorized` | 401/403 |
| `rate_limited` | 429 |
| `timed_out` | 超时 |
| `cancelled` | 调用方取消 |
| `provider_failed` | 请求已发出但结果不可用（连接中断、首字节超时、5xx with `billed: true`） |
| `rejected` | 发出前被拒（空密钥、预算越界、`billed: false`） |

### 用量可见性（`InvocationUsageState`）

| 状态 | 含义 | 典型结果 |
| --- | --- | --- |
| `known` | Provider 返回了用量，可参与费用汇总 | `succeeded`、`repaired` |
| `unknown` | 可能已计费但无法确认，**不能当 0** | `timed_out`、`provider_failed`、`repair_failed`、`cancelled` |
| `unbilled` | 明确未产生费用 | `unauthorized`、`rate_limited`、`rejected` |

**取消按 `unknown` 处理。** `LlmError::Cancelled` 只在客户端侧产生，无法证明上游没有收到请求并计费。把它归为 `unbilled` 会让费用汇总系统性偏低；归为 `unknown` 至少诚实。

### 聚合

`run_usage` 按 `usage_state` 分组：

```
calls          所有物理调用条数（含修复与失败）
known_calls    用量已确认
unknown_calls  可能已计费、无法估算
unbilled_calls 明确未计费
prompt_tokens  只累加 known 行
```

`prompt_tokens` 只统计 `known`，是为了让"部分未知"不会被平均成一个看起来精确的数字。前端在 `unknown_calls > 0` 时显式提示"`N` 次调用费用未知"。

### 边界

- **不持久化 prompt / response 全文。** 只保留 80 字符脱敏摘要（`input_summary`）。结构化失败的原文另存为 `structured_failure` 产物，由既有诊断链路负责，不做全量历史留存。理由是避免 SQLite 无界增长与密钥泄漏面。
- **不做首 token 延迟记录。** 当前 SSE 路径没有首字节时间戳，`duration_ms` 是整次调用耗时。首 token 延迟是独立的后续项。
- **取消记录不受栅栏约束。** `record_invocation` 是非栅栏写入；取消竞争下补写记录不能因为控制版本变化而丢失。
- **不改变任何控制流。** 记账发生在暂停/取消之前，但不影响 `StopSignalKind`、重试预算或 `RunContract`。

## 示例

推荐：

- 新增一次 Provider 调用时，同时决定它的 `InvocationOutcome`，让 `usage_state` 由结果推导，而不是在每个分支手写。
- 失败路径先 `record_failed_invocation`，再 `pause`/`write(Cancel)`。
- 新增结果分类时，先回答"这次调用是否可能已经计费"，再决定 `usage_state`。

禁止：

- 把多次物理调用的 usage 相加写进一条记录。
- 用 `0` 表示"不知道"。
- 在成功路径以外的分支跳过记账。

## 失败模式

| 现象 | 排查路径 |
| --- | --- |
| 运行显示 1 次调用但实际发生 2 次 | 检查修复分支是否把 `repair_response.usage` 累加进首次 `usage` |
| 费用汇总明显偏低 | 查看 `unknown_calls` 是否为 0；若为 0 而存在超时/断开，说明失败路径没有记账 |
| 修复流量在运行记录里看不到 | 确认修复请求使用独立 `request_id` 并有自己的 `record_invocation` |
| 历史库升级后旧记录消失 | 检查 0028 是否为纯增量；`usage_state` 默认 `known`，无用量旧行按 `unknown` 统计 |

不能用"把未知写成 0"来让报表好看，也不能用"合并成一条"来减少记录数。

## 相关模块

- `crates/novel-domain/src/observability.rs`：结果分类与用量状态契约
- `crates/novel-application/src/worker.rs`：`invocation()`、`record_usage_before_stop`、`record_failed_invocation`、`record_repair_invocation`、`invocation_outcome_for_error`
- `crates/novel-application/src/dto.rs`：`RunUsageView`
- `crates/novel-storage-sqlite/src/workflow.rs`：`insert_invocation`、`run_usage`
- `migrations/0028_invocation_observability.sql`
- `client/src/features/directorNext/TauriDirectorPage.tsx`：未知费用提示

## 来源文档

- Upstream-Sync-A1 审计：`docs/architecture/UPSTREAM-SYNC-A1-AUDIT.md`
- 章节生产链：`chapter-production-chain.md`
- 自动导演运行时：`auto-director-runtime.md`
- 书级用量日志（旧服务端内核的台账口径）：[小说 AI 用量与调用日志边界](./novel-ai-usage.md)。
  两者是不同实现：该页描述 `LlmInvocationUsageRecord` 书级投影，本页描述 Rust 内核
  `llm_invocation` 的逐次记账与费用可见性。
