# P15.8 Beta RC 验收记录

起始 HEAD：`1c5bfb17`　最终 HEAD：`974e9145`

## 门禁

| Gate | 状态 | 真实证据 |
| --- | --- | --- |
| Native Index Restart | VERIFIED | 三阶段真实两进程 SIGKILL → 新进程接管，`claim_epoch` 两次递增，FTS 只返回 v2 |
| WAL Crash Restore | VERIFIED | 12 条单元用例（含截断备份无结构、只读目标、切换失败）+ 2 条进程级 SIGKILL（备份写到一半、请求恢复后）+ 磁盘写满（16MB 映像写满后备份失败） |
| Desktop Performance | VERIFIED | 100/500/1000 延迟、冷热分离、CPU/内存曲线、空载 5 次冷启动（中位数 322/327/333 ms）、30 分钟空闲内存 |
| Shared Author Journey | VERIFIED | 同一 SQLite / 同一 Novel ID 七阶段全通过 |
| Quality Debt Security | VERIFIED | 3 条负向用例 + 两步确认界面 |
| Full Regression | VERIFIED | workspace 与 all-targets（detach worktree，`cargo_exit=0`、53 个 test-result ok）；前端 186 passed + build；Native 用例含 dual-instance |
| Production Audit | VERIFIED | app 23.27 MiB、审计 PASS、111 命令 / 111 能力、7 个 e2e 钩子字符串全 0 |
| Native Save | BLOCKED（人工） | 导出链路 3 passing；系统保存对话框 MANUAL_NOT_RUN |

## 结论

`BETA_RC_BLOCKED`：唯一未关闭的门禁是 macOS 系统保存对话框的人工验收。

环境类故障（磁盘写满、双实例同时运行）本轮已补上真实用例，不再是缺口。