# P10.1 Repair Reliability

## 1. 风险矩阵（基于 HEAD f44e5ca5 实际源码）

| 风险 | 状态 | 证据 / 处理 |
| --- | --- | --- |
| `max_automatic_retries` 同时承担远端重试和质量修复 | **FIXED** | 新增 `repair_budget`，Plan v3；v2 保留旧语义 |
| 旧 v1/v2 Run 被静默套用新修复规则 | **FIXED（设计）** | Plan v3；v2 用 `max_automatic_retries` 作为遗留修复预算；v1 仍 fail-closed |
| Repair 请求与远程请求共享预算 | **FIXED** | 远端 retry 只看 `max_automatic_retries`；质量修复只看 `repair_budget` |
| residual issues 后 Worker tick 返回 Conflict | **FIXED** | `pause_safety` 处理已持久化的暂停，不再误报；`f44e5ca5` |
| Repair 超时 | **PARTIAL** | Provider 错误路径有暂停，但无专项真实超时测试 |
| Repair SSE 提前 EOF | **PARTIAL** | Provider 层已有错误分类；未做专项修复测试 |
| Repair 期间 Cancel | **PARTIAL** | Cancellation 覆盖所有 LLM 调用；无 repair 专项 E2E |
| Cancel 后迟到 token | **PARTIAL** | 通用取消/Channel 逻辑；无 repair 专项测试 |
| Repair candidate 落库后进程崩溃 | **PARTIAL** | candidate 已持久化；无跨进程 repair 恢复 E2E |
| Re-review 完成后进程崩溃 | **PARTIAL** | Artifact/Attempt 持久化；无跨进程 E2E |
| Lease 过期旧 Worker 迟到提交 | **PARTIAL** | 现有 Lease/CAS 测试；无 repair 专项 |
| 恢复重复已完成远端请求 | **VERIFIED（现有 recovery 测试）** | `RemoteStarted/CandidateSaved/Interrupted` 进入 HoldInterrupted，不自动重发 |
| 预算耗尽 | **VERIFIED** | `repair_budget=0` / 旧默认 0 → Safety pause，不调用 repair |
| 非法审校 JSON | **VERIFIED** | P8 validation test → paused，不变成 issues=0 |

## 2. 三类预算

| 预算 | 字段 | 语义 |
| --- | --- | --- |
| Remote request retry | `max_automatic_retries` (0/1) | 只用于用户明确重试远程请求；超时/不确定结果不自动重试 |
| Quality repair iteration | `repair_budget` (0/1) | 只用于 Plan v3 的 bounded repair |
| Total generation tokens | `token_budget` + `context_token_budget` | Provider output / Context input，独立 |

- `repair_budget` 冻结在 v3 RunContract；旧合同缺省为 0。
- Plan v2 Run 仍用 `max_automatic_retries` 作为遗留修复预算，避免历史 Run 行为突变。
- Plan v1 Run 的 bounded_repair 仍暂停，不自动修复。

## 3. 状态边界

持久化事实：

- review artifact / review version
- quality debt open/resolved
- repair candidate
- repair_notes
- re-review consistency_review v+1
- approval gate
- chapter_commit

每次远端调用：

- 独立 attempt id
- 独立 request id
- invocation 记录
- 失败/取消/超时 pause，不写成功 artifact

恢复：

- phase `remote_started/candidate_saved/interrupted` → HoldInterrupted
- 不自动重复可能已计费的请求
- 只有明确用户 retry 或本地恢复才继续

## 4. 测试

```text
cargo test -p novel-application --test p9_repair_flow
3 passed

- clean repair + re-review clean + commit
- residual issues + Safety pause + no commit
- repair_budget=0 + remote retry=1 → no repair prompt
- 原/修复 candidate 保留
```

`pause_safety` 回归：residual issues 后 run 处于 paused，worker 不再返回 Conflict。

## 5. 未完成

- repair 超时/SSE EOF/cancel 后迟到结果的专项测试
- repair candidate 已落库后真实进程崩溃恢复 E2E
- re-review 完成后跨进程恢复 E2E
- 修复进行中 Lease 过期旧 worker 的专项测试
