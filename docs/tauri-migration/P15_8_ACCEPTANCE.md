# P15.8 Beta RC 验收记录

起始 HEAD：`1c5bfb17`　最终 HEAD：`26ce5478`

## 门禁

| Gate | 状态 | 真实证据 |
| --- | --- | --- |
| Native Index Restart | VERIFIED | 三阶段真实两进程 SIGKILL → 新进程接管，`claim_epoch` 两次递增，FTS 只返回 v2 |
| WAL Crash Restore | PARTIAL | `cargo test -p novel-desktop --lib` 11 passed（9 条窗口/失败用例，含只读目标与切换失败）；未做进程级全矩阵 |
| Desktop Performance | PARTIAL | 100/500/1000 三档 + 首次调用与稳态分离 + 3 次冷启动 + 30 分钟空闲内存；缺 CPU 持续曲线 |
| Shared Author Journey | VERIFIED | 同一 SQLite / 同一 Novel ID 七阶段全通过 |
| Quality Debt Security | VERIFIED | 3 条负向用例 + 两步确认界面 |
| Native Save | BLOCKED（人工） | 导出链路 3 passing；系统保存对话框 MANUAL_NOT_RUN |
| Full Regression | VERIFIED | workspace 与 all-targets（detach worktree 内，`cargo_exit=0`、53 个 test-result ok）全绿；前端 186 passed + build；13 个 Native 用例复跑 |
| Production Audit | VERIFIED | app 23.27 MiB、审计 PASS、111 命令 / 111 能力、e2e 钩子字符串全 0 |

## 结论

`BETA_RC_BLOCKED`：唯一无法由自动化关闭的是 macOS 系统保存对话框的人工验收。
剩余两项 PARTIAL 是覆盖度问题，不是失败：WAL 缺进程级全矩阵（备份写入中 SIGKILL、磁盘满、双实例），
性能缺 CPU 持续曲线。