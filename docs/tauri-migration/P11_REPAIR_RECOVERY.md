# P11.1 Repair Recovery

基线：`3a6622ec`。

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
