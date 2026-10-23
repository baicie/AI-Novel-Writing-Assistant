# P11 Acceptance

基线：`3a6622ec`。代码/测试提交见最终报告。

## 状态

| 阶段 | 状态 | 说明 |
| --- | --- | --- |
| P11.1 Repair fault/recovery | **PARTIAL** | 真实 HTTP SSE EOF/Cancel/成功 + **Repair Candidate SIGKILL 新进程恢复 PASS**；Lease/timeout/re-review crash 未完成 |
| P11.2 Candidate edit journal | **NOT_STARTED** | 仅审计；sessionStorage 仍未升级为 SQLite Journal |
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

## P11.2 / P11.3

未实施 SQLite candidate journal、accepted quality debt Command/UI，因此 Gate 不通过。

## 回归

- Rust workspace：236 passed
- 干净 Worktree all-targets（HEAD 065f7fa1）：PASS
- 已跟踪 HTTP：55 passed
- frontend tests：186 passed
- typecheck：PASS
- build：PASS
- fmt：PASS
- clippy：PASS（既有 warning）
- Tauri production bundle audit：PASS
- 生产 `.app`：20.08 MiB，DMG 9.1 MiB

Native E2E 沿用 P10 最终构建（P11 仅新增测试，无应用代码变更）。
