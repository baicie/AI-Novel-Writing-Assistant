# P15.8 Beta RC 验收记录

起始 HEAD：`1c5bfb17`　最终 HEAD：见 `git log -1`

## 门禁

| Gate | 状态 | 真实证据 |
| --- | --- | --- |
| Native Index Restart | VERIFIED | 三阶段真实两进程 SIGKILL → 新进程接管，`claim_epoch` 两次递增，FTS 只返回 v2 |
| WAL Crash Restore | PARTIAL | `cargo test -p novel-desktop --lib` 9 passed（7 条崩溃窗口），未 checkpoint 的提交仍在保护副本 |
| Desktop Performance | PARTIAL | 100/500/1000 三档 + 3 次冷启动 + 30 分钟空闲内存观察；缺冷热分离与 CPU 曲线 |
| Shared Author Journey | VERIFIED | 同一 SQLite / 同一 Novel ID 七阶段全通过 |
| Quality Debt Security | VERIFIED | 3 条负向用例 + 两步确认界面 |
| Native Save | PARTIAL | 导出链路 3 passing；系统对话框 MANUAL_NOT_RUN |
| Full Regression | PARTIAL | workspace / all-targets / 前端全绿；未在独立 worktree 重跑 |
| Production Audit | VERIFIED | 构建 + 审计 + e2e 钩子 0 命中 + 111/111 |

## 结论

`BETA_RC_BLOCKED`：唯一无法由自动化关闭的是 macOS 系统保存对话框的人工验收；
另有冷热分离性能统计、WAL 进程级全矩阵与独立 worktree 回归属于本轮未做项。