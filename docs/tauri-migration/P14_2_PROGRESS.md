# P14.2 Progress

## 2026-10-25

起始 HEAD：`b8261fb5`

| 阶段 | 状态 | 说明 |
| --- | --- | --- |
| S0 Audit | IMPLEMENTED | `P14_2_ARCHITECTURE.md` |
| S1 History Safety | VERIFIED | provenance + 草稿决策 + E2E PASS |
| S2 Export | PARTIAL | 单章/范围/全书/选择 ID；按卷/文件对话框未完成 |
| S3 Batch Orchestrator | NOT_STARTED | — |
| S4 Batch Native UI | NOT_STARTED | — |
| S5 Task Center | NOT_STARTED | — |
| S6 Index Recovery Native | PARTIAL | Rust SIGKILL/Worker 已有；Native Retry WDIO 未完成 |
| S7 Structured Memory | NOT_STARTED | — |
| S8 Author Journey | PARTIAL | S1/S2/既有旅程通过；批量未实现 |
| S9 Acceptance | IN_PROGRESS | 最终测试/生产构建 |

## S1 tests

```text
chapter_history 2 passed
E2E chapter-history 1 passing
```

## S2 tests

```text
E2E export 2 passing（单章/范围/全书/隔离）
```

## Not done

- S2 按卷、Native 文件保存对话框
- S3/S4 批量编排与工作台
- S5 Task Center
- S7 结构化记忆
- S6 Native Index Retry

## 2026-10-25 Final

- workspace tests: 282 passed (before opaque-revision guard)
- clean all-targets: 341 passed (before opaque guard; final rerun pending)
- chapter-history E2E PASS
- export E2E PASS (2 tests)
- editor/candidate crash/restart PASS
- production .app 21.54 MiB; DMG 9.6 MiB; audit PASS
- Commands 92 / capabilities 92
- S3/S4/S5/S7 NOT_STARTED
- S6 Native Retry missing

## 2026-10-25 17:35 Final

- final code HEAD: `65e09e36`（docs commit follows）
- clean worktree all-targets: **342 passed, 0 failed**
- workspace tests: 282 passed（opaque guard 前；guard 由 all-targets 覆盖）
- chapter-history E2E: PASS
- export E2E: 2 PASS（单章/范围/全书/隔离）
- editor + candidate crash/restart: PASS（final UI build）
- production .app: 21.54 MiB
- DMG: 9.6 MiB, hdiutil VALID
- audit: PASS
- Commands 92 / Capabilities 92（execute_index_job 为内部 helper）
- S3/S4/S5/S7: NOT_STARTED
- S6: Native Index Retry 未完成
- S2: 按卷 + Native 文件保存对话框未完成
