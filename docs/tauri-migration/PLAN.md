# Tauri 2 + Rust 原生迁移计划

## 目标切片

1. **P0 Tauri 壳**：移除默认 Node sidecar 与本地 HTTP 依赖，React/Vite 由 Tauri WebView 加载，启用最小权限与严格 CSP。
2. **P1 书稿内核**：SQLite/SQLx 的小说、卷、章、章节修订；外键、WAL、busy timeout；版本检查；长正文重启读回测试。
3. **P2 模型网关**：OpenAI-compatible reqwest + SSE；超时、取消、有界重试、错误分类、预算上限；Tauri Channel 流式适配。
4. **P3 前端接入**：统一 `desktopTransport`，核心书稿 API 走 Tauri Commands；保留原 React/Plate 页面和 HTTP 页面兼容边界。
5. **P4 旧库导入**：只读旧库、一致性备份、ID 映射、数量与长文校验、未支持字段报告。
6. **P5 工作流**：Run 状态机、事件持久化、命令幂等、CAS、Approval Gate、checkpoint、取消/恢复测试。
7. **P6 检索与交付**：FTS5、向量适配、构建、测试、报告。

## 边界

- 不把 Axum/Express 作为新桌面默认通信路径。
- 发布包不包含 Node/Electron/Sidecar。
- 不覆盖用户未提交修改；旧数据迁移只读、先备份。
- 未完成项必须进入 `COMPATIBILITY.md` 和 `BLOCKERS.md`，不得静默降级。

## 验收顺序

`cargo fmt --check` → `cargo check --workspace` → 目标 crate 测试 → Tauri 编译/打包 → 前端 typecheck/build → 手工 GUI 验收（若环境允许）。
