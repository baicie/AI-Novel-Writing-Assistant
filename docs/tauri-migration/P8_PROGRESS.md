# P8 自动导演迁移进度

## 当前阶段

P8.1–P8.3 的最小结构化后端切片已实现；P8.4 已交付 Tauri Director MVP（页面、读模型、transport、路由 smoke）；P8.5 完成 Scripted Mock 正常链、fail-closed 和部分异常测试。

## 已完成代码

- `crates/novel-prompt/src/lib.rs`：新增 `story_macro`、`character_cast`、`volume_strategy`、`chapter_list`、`consistency_review` PromptAsset 及版本。
- `crates/novel-testkit/src/lib.rs`：新增 `ScriptedLlm`，按 Prompt marker 返回结构化测试样本。
- `crates/novel-application/src/worker.rs`：
  - `story_macro`、`character_cast`、`volume_strategy`、`chapter_list`、`consistency_review` 从固定 payload 改为真实 LLM + JSON/Schema 校验；
  - `context` 从上游 artifacts 组装；
  - 失败/非法 JSON/ schema mismatch 进入 pause，不伪造成功；
  - 卷数/章节序号唯一、章节卷归属、人物引用校验；
  - `bounded_repair` 已 fail-closed；`approval` 仍使用既有 gate plumbing。
- `crates/novel-application/src/worker/worker_validation.rs`：拆分出的结构化校验模块，避免 `worker.rs` 超过维护阈值。
- `crates/novel-application/tests/p8_director_loop.rs`：
  - `ScriptedLlm` 返回真实结构样本；
  - 非法 Story Macro 输出进入 pause，不持久化成功 artifact；
  - 创建小说/章节 → Story Macro → Character Cast Gate → Volume Gate → Chapter Plan → Context → Draft → Review → Approval → Commit；
  - 断言 artifact 内容、人物关系、Gate、最终正文、revision 和 SQLite 重开。
- `crates/novel-application/tests/recovery.rs`：为结构化审校步骤补齐测试种子，保持取消/恢复测试覆盖真实流程。
- `crates/novel-application/tests/p8_director_loop.rs`：新增审校问题 fail-closed 测试，断言不生成 `repair_notes`/`chapter_commit`、不覆盖正文。
- `crates/novel-application/src/director.rs` + `dto.rs` + `ports.rs` + `novel-storage-sqlite/src/workflow.rs`：新增 RunDetail 投影（control、gate、cursor、candidate、artifacts、usage）。
- `src-tauri/src/commands/run.rs`：新增 `get_active_run`、`get_run_detail`；`approve_run` 支持可选 `expectedVersion` CAS。
- `client/src/features/directorNext/TauriDirectorPage.tsx`：Tauri 前端 MVP，支持章节/模型启动、状态/事件/产物/候选展示、approve/cancel/resume、完成后正文入口。
- `client/src/features/directorNext/TauriDirectorHistoryPage.tsx`：真实运行历史。
- `client/src/features/directorNext/DirectorRoute.tsx`/`DirectorHistoryRoute.tsx`：桌面路由分流，Web 继续旧页面。
- `client/src/platform/desktopTransport.ts`：typed RunDetail/GetActive/GetDetail/带 cleanup 的 subscribe。
- `scripts/smoke-tauri-director.sh`：Release WebView 路由/启动表单 smoke。
- `src-tauri/tests/runtime_provider.rs`：Tauri composition root 的 `llm_provider()` 真实 HTTP/SSE 测试。
- `crates/novel-application/tests/runtime_provider_http.rs`：真实 OpenAI-compatible Provider + Runtime + SQLite 的 Story Macro HTTP 结构化测试。

## 验证

```bash
cargo test --workspace --exclude novel-http
# 221 passed

cargo test -p novel-application --test p8_director_loop
# 3 passed

cargo test -p novel-application --lib structured_validation_tests
# 4 passed

cargo test -p novel-application --test recovery
# 6 passed
```

## 尚存占位

- `bounded_repair`：已改为 fail-closed 安全暂停；没有真实自动修复循环/质量债修复。
- `approval`：Gate 产物仍是 plumbing payload；真正审批由 storage approve 完成。
- P8 UI：新增 Tauri MVP 页面/历史页；真实 UI 启动 Run、审批、取消、恢复的自动化点击未完成；旧 Workspace/角色/质量详情未迁移。旧 API 路径在桌面模式下返回 NOT_MIGRATED。
- World/State/History/Knowledge Context：未进入 context payload。
- 旧版 User Edited / Replan / Reject / Regenerate 语义未完成。

## 真实 Provider 边界

本轮没有调用真实付费模型。`ScriptedLlm` 是确定性测试 Provider，不是生产 Provider；所有 P8 测试结论只证明合同和数据库/工作流行为。
