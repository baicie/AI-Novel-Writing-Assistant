# P14.1 Progress

## 2026-10-25

初始 HEAD：`3eddd908`

| 阶段 | 状态 | 说明 |
| --- | --- | --- |
| S0 Audit | IMPLEMENTED | `P14_1_DESIGN.md` |
| S1 Crash Recovery | VERIFIED | 上一轮 Runner + 本轮 evidence |
| S2 Index Worker | PARTIAL | 自动 Worker/SIGKILL 已实现；Native Retry WDIO 未完成 |
| S3 Integrity | IMPLEMENTED | G1/G2 测试通过；新增跨进程 revision rank CAS |
| S4 Chapter History | VERIFIED | 后端恢复事务 + Native 历史面板 + E2E PASS |
| S5 Export | IMPLEMENTED / PARTIAL | 单章 Markdown/TXT 导出 + Native 按钮；范围/按卷未完成 |
| S6 Multi-Chapter | NOT_STARTED | — |
| S7 Task Center | NOT_STARTED | — |
| S8 Story Memory | NOT_STARTED | — |
| S9 Acceptance | PENDING | — |

## S4 tests

```text
chapter_history ... ok
```

## Next

1. 运行 S4 E2E。
2. 全量 Rust/Frontend 测试。
3. S5/S6/S7/S8 按资源推进，未完成不标 VERIFIED。

## 2026-10-25 12:35

- HEAD `2dfc7fa8`
- S4 E2E `chapter-history` PASS
- S5 E2E `export` PASS
- S3 新增跨进程索引测试，修复 rank CAS
- S1/S2 既有证据有效
- S5 范围/按卷/文件对话框未完成

## 2026-10-25 12:52

- workspace: 281 passed, 1261.23s
- chapter-history E2E PASS
- export E2E PASS
- cross-process index test PASS
- all-targets in progress
