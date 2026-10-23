# P11 Acceptance

基线：`3a6622ec`。最终代码/测试 HEAD：`fbb34d2b`。

## 状态

| 阶段 | 状态 | 说明 |
| --- | --- | --- |
| P11.1 Repair fault/recovery | **PARTIAL** | 真实 HTTP SSE EOF/Cancel/成功 + **Repair Candidate SIGKILL 新进程恢复 PASS**；Lease/timeout/re-review crash 未完成 |
| P11.2 Candidate edit journal | **PARTIAL** | SQLite Journal + CAS + Native UI + SIGKILL 恢复通过；两窗口/Gate 冲突/失败注入未完成 |
| P11.3 Quality Debt lifecycle | **PARTIAL** | 现有质量债/重复 ID 保护；occurrence/accepted Command/UI 未完成 |

## P11.1 测试

```text
cargo test -p novel-application --test p11_repair_faults
3 passed
```

- repair SSE EOF → paused，无 repair candidate/commit
- cancel during repair SSE → cancelled，迟到 token 不提交
- repair success → re-review clean → commit

未完成：

- repair candidate 落库后真实 SIGKILL
- re-review 完成后 SIGKILL
- lease 过期旧 Worker 迟到写入
- repair timeout
- 重复 Cancel

## P11.1-H0（本轮唯一新增 Gate）

**VERIFIED（仅 after_repair_candidate_persisted 检查点）**

- 真实 SIGKILL 退出状态 9
- 新进程恢复为 Paused
- Repair 请求 1→1
- Candidate 2，原/修复保留
- 无 Commit/Revision
- Quality Debt 保留

不把其他故障点宣称为 VERIFIED。

## P11.2-W1

| Gate | 状态 | 证据 |
| --- | --- | --- |
| G1 两窗口 writer identity 不同 | **PARTIAL** | sessionStorage 窗口级身份；无 Native 双窗口驱动 |
| G2 两份编辑独立持久化/清理 | **VERIFIED** | SQLite 多 writer 测试 |
| G3 Gate 变化旧草稿保留/过期确认拒绝 | **VERIFIED** | list + 旧 Gate 不可恢复；Confirm CAS 回归 |
| G4 新进程发现并恢复旧窗口草稿 | **VERIFIED** | WDIO SIGKILL + 新 writer 恢复 Commit |
| G5 Director/Editor 回归 | **VERIFIED（回归）** | 现有 E2E |

## P11.2

**PARTIAL / core VERIFIED**

- `candidate_edit_draft` additive migration
- local revision CAS
- Tauri Commands
- Native UI 自动保存/恢复
- WDIO SIGKILL 后新进程恢复并 Commit 通过
- 未完成：两窗口、Gate 变化冲突、失败注入、编辑中 Cancel

## P11.3

未实施 Review Issue Occurrence、accepted Command/UI，Gate 不通过。

## 回归

- Rust workspace：251 passed
- 干净 Worktree all-targets（HEAD fbb34d2b）：PASS
- 已跟踪 HTTP：55 passed
- frontend tests：186 passed
- typecheck：PASS
- build：PASS
- fmt：PASS
- clippy：PASS（既有 warning）
- Tauri production bundle audit：PASS
- 生产 `.app`：20.29 MiB，DMG 9.2 MiB

Native E2E 沿用 P10 最终构建（P11 仅新增测试，无应用代码变更）。

## P11.2-W1 最终

- 代码提交：`22e5d2c4`
- Candidate Journal writer identity：sessionStorage（窗口级）
- Discovery：novel + run 范围列出全部草稿
- Native SIGKILL 新进程恢复：PASS
- Native 双窗口同时操作：PARTIAL（嵌入式 WebDriver 单窗口）

## P11.1-H1

**VERIFIED（仅 v3 Repair Candidate 已保存安全续跑）**

- Repair Candidate + Invocation + Attempt 校验后复用
- Repair HTTP 1 → 1
- Re-review 仅新增 1 次
- Approval 1
- Commit/Revision 0
- RemoteStarted 未知结果保持 Paused，不重发
- 重复恢复幂等
- 旧 Plan 不走新语义

## P11.1-H1 最终证据

- Case A SIGKILL PID 7090，退出状态 9
- Repair HTTP 1 → 1
- Re-review HTTP 1 → 2
- Candidate 2
- Approval 1
- Commit 0
- Revision 0
- Quality Debt open 1
- Run WaitingApproval
- 二次恢复 tick 幂等
- Case B RemoteStarted PID 7091，退出状态 9；恢复 Paused；Repair 1 → 1；Candidate 1；Invocation 0；Commit/Revision 0
- 生产 `.app` 20.25 MiB

## P11.1-H2-A Lease Fencing

**VERIFIED**

- 两个 Worker 确定性竞争测试 PASS
- 过期 owner 写入全部 Conflict
- B 接管后 A 不能写 Candidate/Attempt/Invocation/Debt/Control/Revision
- A late Repair 响应未重发、未生成 Candidate
- B 保持 Paused
- Repository 级逐操作 fencing 测试 PASS
- H1 安全续跑、RemoteStarted 暂停、p9/p11/p10 回归 PASS

## P11.4-A Lease 自动续约

**VERIFIED**

- Long model request 期间自动续约。
- 3x lease 窗口后 B 仍不能 Claim。
- 续约不改变 Control Version，不影响 fence。
- 同 owner 重复 Claim 产生新执行权，旧 fence 失效。
- 续约任务在 Tick 结束取消。

## P11.4-B 持久化 Cancel / Re-review 崩溃

**VERIFIED**

- 持久化 Cancel 与迟到 Repair 竞争安全。
- Cancel 命令幂等。
- Re-review Artifact 完成后 SIGKILL 不重复 Review。
- 无 Commit/Revision。

## P11.4-C 原子落库

**VERIFIED**

- Candidate + Attempt CandidateSaved + Invocation 单事务。
- Attempt 不可保存时全回滚。
- H1 安全续跑使用完整事实。

## P11.3 Quality Debt

**PARTIAL / 核心已实现**

- Review Issue Occurrence 表
- 稳定 issue key（不依赖模型 issueId）
- accepted Command + decision_version CAS + 原因
- accepted 不改变 resolved/历史
- Native 接受按钮
- 跨两轮审校的端到端 fixture 与完整详情 UI 未完成

## P11.4-A / B / C

- A Lease 自动续约、长时间请求、重复 Claim：**VERIFIED**
- B 持久化 Cancel 竞争、Re-review 完成后 SIGKILL：**VERIFIED**
- C Candidate/Attempt/Invocation 原子落库：**VERIFIED**

## P11.3

- Review Occurrence、稳定 key、accepted CAS、Native 接受按钮：**VERIFIED（核心）**
- 跨两轮审校端到端 fixture：**VERIFIED**
- 详情 UI（来源/证据/建议/处理记录/接受原因）：**IMPLEMENTED**
- 完整时间轴历史 UI：**PARTIAL**

## P12.1 / P12.2

- Local RAG / Chinese tokenizer / Context Policy baseline：**PARTIAL**
- 中文孤立 CJK token 已修复；未来章节/Candidate 排除 benchmark 通过。
- Long-form 10/50/100 章质量 benchmark：**NOT_STARTED**
- 生产 `.app` 20.44 MiB。
