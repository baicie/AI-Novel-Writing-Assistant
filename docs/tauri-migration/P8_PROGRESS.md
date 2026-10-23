# P8 自动导演迁移进度

## 当前阶段

P8.1–P8.3 的最小结构化后端切片已实现；P8.4 UI 未开始；P8.5 只完成 Scripted Mock 正常链和部分异常测试。

## 已完成代码

- `crates/novel-prompt/src/lib.rs`：新增 `story_macro`、`character_cast`、`volume_strategy`、`chapter_list`、`consistency_review` PromptAsset 及版本。
- `crates/novel-testkit/src/lib.rs`：新增 `ScriptedLlm`，按 Prompt marker 返回结构化测试样本。
- `crates/novel-application/src/worker.rs`：
  - `story_macro`、`character_cast`、`volume_strategy`、`chapter_list`、`consistency_review` 从固定 payload 改为真实 LLM + JSON/Schema 校验；
  - `context` 从上游 artifacts 组装；
  - 失败/非法 JSON/ schema mismatch 进入 pause，不伪造成功；
  - 卷数/章节序号唯一、章节卷归属、人物引用校验；
  - `bounded_repair` 和 `approval` 仍是后续工作。
- `crates/novel-application/src/worker/worker_validation.rs`：拆分出的结构化校验模块，避免 `worker.rs` 超过维护阈值。
- `crates/novel-application/tests/p8_director_loop.rs`：
  - `ScriptedLlm` 返回真实结构样本；
  - 非法 Story Macro 输出进入 pause，不持久化成功 artifact；
  - 创建小说/章节 → Story Macro → Character Cast Gate → Volume Gate → Chapter Plan → Context → Draft → Review → Approval → Commit；
  - 断言 artifact 内容、人物关系、Gate、最终正文、revision 和 SQLite 重开。
- `crates/novel-application/tests/recovery.rs`：为结构化审校步骤补齐测试种子，保持取消/恢复测试覆盖真实流程。

## 验证

```bash
cargo test --workspace --exclude novel-http
# 217 passed

cargo test -p novel-application --test p8_director_loop
# 2 passed

cargo test -p novel-application --lib structured_validation_tests
# 4 passed

cargo test -p novel-application --test recovery
# 6 passed
```

## 尚存占位

- `bounded_repair`：仍为占位 Step；没有真实修复循环/质量债。
- `approval`：Gate 产物仍是 plumbing payload；真正审批由 storage approve 完成。
- P8 UI：DirectorNovelPage/DirectorPanel/DirectorGate 尚未接 typed Tauri Transport；旧 API 路径在桌面模式下返回 NOT_MIGRATED。
- World/State/History/Knowledge Context：未进入 context payload。
- 旧版 User Edited / Replan / Reject / Regenerate 语义未完成。

## 真实 Provider 边界

本轮没有调用真实付费模型。`ScriptedLlm` 是确定性测试 Provider，不是生产 Provider；所有 P8 测试结论只证明合同和数据库/工作流行为。
