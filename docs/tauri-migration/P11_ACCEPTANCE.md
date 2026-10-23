# P11 Acceptance

基线：`3a6622ec`。代码/测试提交见最终报告。

## 状态

| 阶段 | 状态 | 说明 |
| --- | --- | --- |
| P11.1 Repair fault/recovery | **PARTIAL** | 真实 HTTP SSE EOF、Cancel 迟到 token、Repair success 已测；跨进程 SIGKILL/Lease/timeout 未完成 |
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

## P11.2 / P11.3

未实施 SQLite candidate journal、accepted quality debt Command/UI，因此 Gate 不通过。

## 回归

待最终运行：

- workspace tests
- clean all-targets
- HTTP tracked
- frontend test/typecheck/build
- fmt/clippy
- Native E2E
- production bundle audit
