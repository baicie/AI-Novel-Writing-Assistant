# P11.1-H2-A Lease Fencing

基线：`3e05c8ff`。代码提交：`e6ee266d`。

## Fencing 数据模型

Worker 的执行权由三部分共同决定：

- `workflow_control.lease_owner`
- `workflow_control.version`
- `workflow_control.lease_until`

`ClaimedRun` 携带：

```text
LeaseFence { owner, control_version }
```

每次 Claim 都通过 `note_lease` 递增 Control Version。因此：

- 不同 owner 接管：Version 变化。
- 同一 owner 字符串重新 Claim：Version 也变化，旧执行权不能复用。
- Lease 过期但尚未被接管：写入时校验 `lease_until < now` 拒绝。
- B 接管后：A 的旧 Version/Owner 不匹配，拒绝。

没有新增独立 fencing token；Control Version + Owner + Lease + Status 在 SQL 事务内提供等价的单调执行权。

## 原子写入

以下方法新增 `_fenced` 版本：

- start_attempt_fenced
- mark_remote_started_fenced
- save_candidate_fenced
- mark_attempt_fenced
- record_invocation_fenced
- record_quality_debt_fenced
- resolve_quality_debt_fenced
- apply_control（ControlWrite.fence）
- promote_candidate_fenced

每个写入：

1. 在同一事务内查询 Control。
2. 校验 owner、Control Version、Lease 未过期、Run 可执行。
3. 校验通过后才执行 INSERT/UPDATE。
4. 失效 Worker 返回 `PortError::Conflict`。

允许“完成自身 Step 后 Lease 被清理”的同一 Worker 继续执行下一 Step，但必须满足：

- owner 已清空且 lease 已清空；
- Control Version 未变化。

其他 owner 接管后 Version 变化，旧 Worker 仍被拒绝。

## 写权限矩阵

| 操作 | 有效 Worker | Lease 丢失的 Worker |
| --- | --- | --- |
| start_attempt | 允许 | 拒绝 |
| mark_remote_started | 允许 | 拒绝 |
| save_candidate | 允许 | 拒绝 |
| mark_attempt | 允许 | 拒绝 |
| record_invocation | 允许 | 拒绝 |
| insert_artifact | 通过 apply_control fenced | 拒绝 |
| quality_debt effect | 允许 | 拒绝 |
| apply_control | 允许 | 拒绝 |
| chapter_commit | 满足 Approval + CAS + fence 后允许 | 拒绝 |

`insert_external_artifact` 当前不由 Worker 调用，授权主体是外部导入命令，不强行套用 Worker Lease。

## 两个 Worker 竞争

测试：`crates/novel-application/tests/p11_lease_fence.rs`

时间线：

- T0 `clock=1000`，Worker A claim，lease 50ms。
- A 发出 Repair HTTP 请求，Mock Server 阻塞。
- T1 `clock=1100`，A Lease 过期。
- T2 Worker B claim，Control Version 递增；B 看到 RemoteStarted，选择 HoldInterrupted，Pause。
- T3 释放 A 的合法完整 Repair 响应。
- T4 A 的 `save_candidate_fenced` 被拒绝；A tick 返回错误。

结果：

```text
P11 lease timeline: A blocked repair=1, clock 1000->1100, B paused,
A late write rejected, candidates=1 repair_notes=0 attempts=7 debt=1
```

- Repair HTTP 次数：1（A 的请求，未重发）
- Candidate：1（仅原 Draft）
- Repair Artifact：0
- Approval：0
- Commit：0
- Revision：0
- Quality Debt：保留
- A 的 Attempt：仍为 `remote_started`
- B 的最终状态：Paused

## Repository 级验证

`crates/novel-storage-sqlite/tests/workflow_lease.rs`：

- 过期 owner 直接写入被拒绝。
- B 接管后 A 的 start_attempt/mark_remote/save_candidate/record_invocation/quality_debt/apply_control 全部 Conflict。
- A 的失效率写入后 Attempt/Candidate/Invocation/Debt 行数不变。
- B 的合法写入成功。
- A 的 promote_candidate_fenced 被拒绝；B 的成功生成 Revision。

## 部分事务状态

Candidate、Attempt、Invocation 仍是分开的事务：

- Candidate 已保存但 Attempt 未标记 `candidate_saved`：恢复按 RemoteStarted 处理，不自动复用。
- Attempt 已 `candidate_saved` 但 Invocation 缺失：H1 安全续跑校验失败，不自动复用。
- 不把部分成功伪装成完整成功。

## Plan 兼容

Fencing 只强化执行权，不改变 Workflow 业务语义：

- v1/v2/v3 的 Plan 分支保持不变。
- H1 的 v3 `candidate_saved` 安全续跑仍可用。
- RemoteStarted 结果未知仍保持 Paused。

## Gate

**VERIFIED**（仅 Lease Fencing 写入完整性）。

## 最终测试

```text
cargo test -p novel-application --test p11_lease_fence
1 passed

cargo test -p novel-storage-sqlite --test workflow_lease
2 passed

cargo test --workspace --exclude novel-http
240 passed

cargo test --workspace --all-targets
独立干净 Worktree（HEAD e6ee266d）PASS

HTTP tracked
55 passed

fmt / clippy
PASS
```

生产包：20.29 MiB，DMG 9.2 MiB；recovery_worker / WDIO / E2E key absent。


## P11.4-A Lease 自动续约

### 机制

Worker 每次 Claim 后启动一个 renewal task：

- 仅在 `lease_ms > 0` 且允许续约时启动。
- 每 `lease_ms / 3`（最少 10ms）调用 `renew_lease`。
- 只延长 `lease_until`，不改变 Control Version。
- 校验 Owner + Control Version + Run Status。
- Tick 结束（无论成功/失败/暂停）立即取消 renewal task。
- 续约失败不吞掉：当前 Tick 的后续 fenced 写入仍会失败或返回错误。

`renew_lease` 接受 run_id，避免同一 owner 多 Run 时续错 Lease。

### 长时间请求

真实 SystemClock + 真实 HTTP/SSE Server 延迟 400ms：

```text
lease=100ms
A 执行 Story Macro 400ms
B 在 250ms 尝试 Claim → Idle
A 完成
P11 renewal: long request kept lease for >3x lease window; B idle; A status=Running
```

### 重复 Claim

同一 owner 重复 Claim 会递增 Control Version，产生新的执行权：

- 旧 fence 的 start_attempt 被拒绝。
- 新 fence 可以继续写入。
- 不依赖 owner 字符串作为执行权。

### 测试

```text
cargo test -p novel-application --test p11_lease_fence
2 passed

cargo test -p novel-storage-sqlite --test workflow_lease
2 passed
```


## P11.4-B 持久化 Cancel / Re-review 崩溃

### Cancel 竞争

`persistent_cancel_wins_over_late_repair_result`：

- A 已发出 Repair 请求，Server 阻塞。
- 执行持久化 `cancel`命令。
- 重复同一 command id 返回 replayed，不产生第二次副作用。
- 释放 A 的迟到完整 Repair 响应。
- A 的 fenced 写入被拒绝。
- Run：Cancelled。
- Repair 请求：1。
- Repair Candidate：0（只有原 Draft）。
- Repair Artifact / Approval / Commit / Revision：0。
- 不出现“已取消后又提交正文”。

### Re-review 完成后 SIGKILL

`completed_rereview_sigkill_does_not_replay_review_or_commit`：

```text
P11 review-crash pid=17562 sigkill=unix_wait_status(9)
reviews=2 repairs=1 candidates=2 approvals=1 commits=0 revisions=0
```

- Re-review Artifact 已持久化后 SIGKILL。
- 新进程不重发 Re-review。
- 不重复 Candidate。
- Approval 只创建一次。
- 无人工确认前不 Commit。
- 正式 Revision 为 0。

## P11.4-C Candidate/Attempt/Invocation 原子落库

新增 `save_candidate_checkpoint`：

同一 SQLite 事务内完成：

1. 插入 Chapter Candidate；
2. Attempt `candidate_saved` + candidate_id；
3. 插入 LLM Invocation；
4. 写 checkpoint_saved 事件。

任一条件失败全部回滚：

- Attempt 不存在/不在可保存状态；
- Candidate 与 Invocation 的 Run/Step 不一致；
- Lease 失效。

测试：

```text
candidate_checkpoint_rolls_back_when_attempt_is_not_saveable
PASS
```

断言冲突后：

- Candidate = 0
- Invocation = 0
- checkpoint_saved Event = 0

Worker 的 Draft 与 Repair 保存路径均已使用该原子方法。H1 安全续跑仍要求 Candidate + Attempt + Invocation 完整，不接受部分成功。

## P11.4-B/C 测试

```text
p11_lease_fence：3 passed
workflow_lease：3 passed
p11_recovery_process：3 passed
workspace：243 passed
```
