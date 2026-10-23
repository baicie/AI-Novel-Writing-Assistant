# P9 Acceptance

基线：P9.1 `b6e13d92`；P9.2/P9.3 代码提交 `388058ab`。

## P9.1 Context

状态：**IMPLEMENTED / 部分 VERIFIED**（231 Rust tests 中的 P9 context/flow 通过）

自动证据：

- `p9_context`：3 tests
  - 世界、角色、前序正式正文进入上下文
  - 未来章节、未确认候选被排除
  - 跨小说隔离
  - 缺失来源显式 trace
  - 预算裁剪
  - 确定性 hash
- `p9_context_flow`：真实 Mock Provider + SQLite，v2 draft prompt 包含世界/角色/前3章，context artifact 重启后可读

未完成：

- 旧版完整 Context Broker policy、知识库/RAG、独立用户补充设定入口
- 真实 tokenizer 精确计数

## P9.2 Review / Repair

状态：**IMPLEMENTED / PARTIAL**

自动证据：

- `p9_repair_flow`：2 tests
  - issues → repair candidate → re-review clean → commit repaired
  - residual issues → Safety pause，无 commit，quality debt open
  - 原/修复候选均保留
- P8 `issues > 0 + max_retries=0` fail-closed 仍通过
- 质量债 migration、Rust repository、RunDetail 投影、Native UI 质量问题列表

未完成：

- repair timeout/SSE/cancel after-cancel E2E
- repair 中断进程恢复专门 E2E
- 质量债 UI
- residual pause 后让 worker tick 完全安静（当前可能出现一次 Conflict 传播，run 已 paused）

## P9.3 Decisions

状态：**IMPLEMENTED（backend）/ PARTIAL（UI）**

自动证据：

- `p9_decisions`：Reject、Confirm After Edit、legalActions、历史保留
- Tauri commands + capabilities 已增加
- Native UI Reject/Regenerate 按钮
- WDIO Director spec：3 passing，包含 Reject、Regenerate、历史 stale artifact 验证

未完成：

- Confirm After Edit 编辑 UI
- 过期 Gate/双窗口/断线/审批取消竞态 E2E

## 全阶段质量（最终）

在 HEAD `388058ab` 上：

- Rust workspace：**231 passed**
- 干净 Worktree `cargo test --workspace --all-targets`：**PASS**
- 已跟踪 novel-http targets：**55 passed**
- 前端 tests：**186 passed**
- frontend typecheck：**PASS**
- frontend build：**PASS**
- fmt/clippy：**PASS（既有 warning）**
- Native Director WDIO：**3 passing**（含 Reject/Regenerate）
- Cancel：**1 passing**
- Safety：**1 passing**
- Editor autosave/refresh：**2 passing**
- Editor 跨进程重启：**1 passing**
- Editor SIGKILL：crash harness 预期非零，新进程恢复 **1 passing**
- 生产 `.app`：**20.00 MiB**
- DMG：**9.1 MiB**
- 生产隔离审计：**PASS**（无 WDIO/E2E Key/WebDriver marker）

## 状态声明

不宣称旧版自动导演完整迁移；不宣称真实付费 Provider 验收；不以 Mock E2E 代替真实模型/人工写作验收。
