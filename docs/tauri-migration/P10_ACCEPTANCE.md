# P10 Acceptance

基线：`2bf8d95c`。本轮代码提交见下方。

## Gate 状态

| Gate | 状态 | 依据 |
| --- | --- | --- |
| G1 Repair 异常/取消/跨进程恢复安全 | **PARTIAL** | residual pause、预算耗尽、非法 JSON、无重复已完成远端请求已验证；repair 超时/SSE/cancel/跨进程专项 E2E 未完成 |
| G2 Remote Retry 与 Quality Repair 预算分离 | **VERIFIED** | Plan v3 + `repair_budget`；`remote_retry_budget_does_not_authorize_quality_repair` |
| G3 Quality Debt / Context 事实可追踪一致 | **PARTIAL** | 质量债、review version、context trace、重复 issue ID 校验；accepted 生命周期/RAG 未完成 |
| G4 Confirm After Edit 真实 Native UI | **VERIFIED（核心）** | WDIO 4 passing + 重启 1 passing；Rust CAS 1 test |
| G5 正式正文/候选/Local Draft/作者编辑保护 | **VERIFIED（回归）** | P8 Director、Editor SIGKILL、Local Draft CAS、Confirm Edit、Commit 均通过 |

## 测试结果

```text
cargo test --workspace --exclude novel-http
233 passed

cargo test --workspace --all-targets
独立干净 Worktree 待最终复跑（见最终报告）

已跟踪 novel-http target
55 passed

pnpm --filter @ai-novel/client test
186 passed

pnpm typecheck / build
PASS

WDIO test:director
4 passing

WDIO confirm-edit-restart
1 passing

WDIO safety/cancel
各 1 passing

WDIO safety / cancel / editor / editor-restart
通过
```

## 生产

- `.app`：20.08 MiB
- DMG：9.1 MiB
- `hdiutil verify`：PASS
- Tauri release audit：PASS
- 二进制无 `tauri_plugin_wdio` / `wdio-webdriver` / E2E key / E2E endpoint

## 未完成

- P10.1 专项故障注入（timeout/SSE EOF/cancel-after-repair、真实进程崩溃）
- P10.2 accepted debt、RAG/完整 Context Broker、敏感 promptText 导出审计
- P10.3 双窗口、编辑中取消、保存失败、正式章节外部更新
- 真实 Provider/付费/真实用户库验收
