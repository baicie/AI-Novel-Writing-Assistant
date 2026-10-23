# P11.1 Repair Recovery

基线：`3a6622ec`。H0 提交：`f5c991e1`。

## 1. 本轮真实故障测试

新增 `crates/novel-application/tests/p11_repair_faults.rs`：

- 真实 `OpenAiCompatibleProvider`
- 本机 TCP HTTP/SSE Server
- 真实 SQLite
- 真实 Runtime / Worker / Gate / Candidate / Artifact

测试：

```text
cargo test -p novel-application --test p11_repair_faults
3 passed
```

覆盖：

1. **Repair SSE 提前 EOF**
   - Provider 返回部分 token 后断开。
   - Run `ProviderUnavailable` 暂停。
   - 无 `repair_notes`、无 `chapter_commit`。
   - 只有原 Draft Candidate，无修复 Candidate。
2. **Repair 期间 Cancel 且迟到 token**
   - Repair SSE 已输出首个 token。
   - 取消 Run，Server 继续持有连接。
   - Run 为 `cancelled`。
   - 无正式 Revision、无 Commit。
3. **Repair 正常完成**
   - 第一次 Review issues。
   - 真实 HTTP repair 返回修复候选。
   - 二次 Review clean。
   - Approval 后 Commit。
   - 两次 Review 调用可追踪。

## 2. 故障注入矩阵

| 边界 | 状态 | 证据 |
| --- | --- | --- |
| Repair Provider 调用前 | **PARTIAL** | 预算 0 时不调用，已测 |
| RemoteStarted 标记后 | **PARTIAL** | 现有 recovery/attempt 测试；无 repair 专项 |
| SSE 首个 token 后 | **VERIFIED** | cancel test |
| SSE 中途断开 | **VERIFIED** | EOF test |
| Provider 完整返回后 | **PARTIAL** | 真实 HTTP success |
| Repair Candidate 写入 SQLite 后 | **NOT_VERIFIED** | 无跨进程 SIGKILL repair test |
| Re-review RemoteStarted 后 | **PARTIAL** | attempt 状态会 HoldInterrupted；无 repair 专项 |
| Re-review Artifact 写入后 | **NOT_VERIFIED** | 无跨进程 test |
| Quality Debt 更新前后 | **PARTIAL** | 质量债测试；无崩溃点测试 |
| Approval Gate 创建前后 | **PARTIAL** | Gate E2E；无崩溃点测试 |

## 3. 核心不变量

已验证：

- Cancel 后迟到 token 不 Commit。
- Repair SSE EOF 不产生 Candidate/Commit。
- 正常修复需要二次 Review clean 才能 Approval。
- 失败/取消路径保留原 Candidate、Quality Debt。
- `repair_budget` 与 `max_automatic_retries` 独立。
- 已完成的 Request 恢复不会自动重发（现有 recovery tests）。

未验证：

- Repair Candidate 落库后真实 SIGKILL 新进程恢复。
- Re-review 完成后 SIGKILL。
- Lease 过期旧 Worker 迟到写入 repair Candidate/Artifact。
- Repair timeout（真实超时，而非 EOF）。
- 重复 Cancel 幂等专项。
- Quality Debt 崩溃前后无丢失/重复清除。

## 4. 远端调用次数

- Repair success：repair 1 次 + re-review 1 次，均独立 request id/attempt。
- Repair EOF：repair 1 次，失败后不自动重发。
- Cancel：repair 1 次，取消后不重发。
- 无真实付费模型调用。

## 5. Gate

**PARTIAL**。未达到“中断/取消/失去 Lease/跨进程恢复全部有真实证据”的 VERIFIED 条件。


## P11.1-H0 跨进程 SIGKILL 证据

测试命令：

```bash
cargo test -p novel-storage-sqlite --features test-harness \
  --test p11_recovery_process -- --nocapture
```

结果：

```text
P11 child_pid=91637 sigkill_status=ExitStatus(unix_wait_status(9)) repairs=1 reviews=1
P11 recovery status=Paused repairs_after=1 reviews_after=1 candidates=2 commits=0 revisions=0 debt=1
test ... ok
```

检查点 `after_repair_candidate_persisted`：

- 独立 `recovery_worker` 进程。
- 真实 HTTP/SSE Mock Server 在父进程存活。
- Repair Candidate 已落库。
- Step Attempt phase = `candidate_saved`。
- Repair Invocation 已记录。
- 父进程读取 SQLite 后对子进程执行 SIGKILL；`unix_wait_status(9)`。
- 新进程使用同一 SQLite 与 owner 恢复。
- 恢复为 `Paused`（HoldInterrupted）。
- Repair 请求数 1 → 1。
- Re-review 请求数 1 → 1（崩溃前未开始）。
- Candidate 数量 2，内容/hash 一致。
- chapter_commit=0，chapter_revision=0。
- Quality Debt=1 保留。

Feature 隔离：

- `test-harness` 仅在显式启用时构建 `recovery_worker`。
- 普通构建不包含 test-only binary。
- Worker 的 checkpoint hook 默认 no-op。


## P11.1-H1 Repair Candidate Safe Resume

### Attempt 状态转换

| 阶段 | 持久化事实 | 恢复动作 |
| --- | --- | --- |
| 尚未开始远端请求 | Attempt `started`，无 Request ID | 可复用本地准备状态 |
| RemoteStarted，结果未知 | Attempt `remote_started` + request_id | **HoldInterrupted / Paused，不重发** |
| Provider 返回但未持久化 | Attempt `remote_started` | 同上，按未知处理 |
| Repair Candidate 已保存 | Attempt `candidate_saved` + candidate_id + request_id + Invocation | v3 且校验通过：安全续跑；否则暂停 |
| Repair Step 已确认 | Attempt `committed` + repair_notes | 按 Plan 继续 |
| Re-review 已开始 | Re-review Attempt `remote_started` | HoldInterrupted |
| Re-review 已完成 | 新 Review Artifact | 按 Plan 继续 |
| Approval 已开启 | Control Gate | 等待作者确认，不再调用 HTTP |

**不能仅凭 Candidate 存在推断后续事实完整。** 只有 `candidate_saved` + Invocation + candidate/hash + 当前 Plan 全部匹配才续跑。

### 安全续跑校验

`try_resume_repair_candidate` 要求：

- Plan 为 `novel-production-v3`
- 冻结 `repair_budget > 0`
- latest Attempt：
  - step_id = `bounded_repair`
  - phase = `candidate_saved`
  - candidate_id / request_id 存在
- Control 当前 cursor 仍允许 `bounded_repair`
- latest Candidate 就是 Attempt 绑定的 Candidate
- Candidate 内容 SHA-256 与持久化 hash 一致
- `llm_invocation` 存在且 request_id/step_id 精确匹配
- 同一 scope 不存在已完成的 `repair_notes`
- 存在原 Draft Candidate，用于 repair_notes 审计
- Claim 已获得 Lease，Run 未被 Cancel
- 无更新 Attempt（latest 即为该 Attempt）

满足后：

- 不调用 Repair Provider
- 不新建 Repair Candidate
- 不重新消费 repair budget
- 用既有 Attempt 完成 Repair Step（写 repair_notes）
- 只发起一次 Re-review
- 进入 Approval Gate 后停止，不自动 Commit

RemoteStarted / 旧 Plan / 校验失败：保持 HoldInterrupted 或 DataIntegrity 暂停。

### H1 跨进程测试

命令：

```bash
cargo test -p novel-storage-sqlite --features test-harness \
  --test p11_recovery_process -- --nocapture
```

结果：

```text
Case A: child_pid=7090 sigkill_status=9 repairs=1 reviews=1
        recovery status=WaitingApproval repairs_after=1 reviews_after=2 candidates=2 commits=0 revisions=0 debt=1

Case B: child_pid=7091 sigkill_status=9 repair_requests=1
        recovery status=Paused repairs_after=1 reviews_after=1 candidates=1 invocations=0 commits=0 revisions=0 debt=1
```

Case A：

- Repair Candidate 已保存后 SIGKILL。
- 新进程安全续跑。
- Repair HTTP：1 → 1。
- Re-review HTTP：1 → 2（仅一次新增）。
- Candidate：2。
- Repair Attempt：1。
- Approval Artifact：1。
- Chapter Commit：0。
- Chapter Revision：0。
- Quality Debt：1，保持 open。
- Run：WaitingApproval。
- 第二次恢复 tick：请求数、Candidate、Approval 均不变。

Case B：

- Repair 请求发出后 SIGKILL，结果未知。
- 不重发 Repair。
- 不生成 Repair Candidate。
- 不生成 Invocation（不伪造 0 usage）。
- 不进入 Re-review。
- Run Paused（HoldInterrupted）。
- Request ID 保留在 Attempt 中。
- 无 Commit/Revision。

### 旧 Plan

- v1：无自动 repair，安全暂停。
- v2：沿用 frozen legacy 语义，CandidateSaved 不进入 v3 安全续跑。
- v3：启用本恢复语义。
