# P8 验收结论

## 总体状态

**P8：PARTIAL。P8.1 后端纵向切片 IMPLEMENTED，P8.2/P8.3 最小结构化合同 IMPLEMENTED，P8.4 UI NOT_STARTED，P8.5 异常覆盖 PARTIAL。**

不得把本地 Mock/结构化 E2E 说成真实 Provider 或真实 GUI 已验收。

## 阶段结论

| 阶段 | 状态 | 证据 | 未完成 |
| --- | --- | --- | --- |
| P8.1 Story Macro | VERIFIED（结构化最小合同） | `story_macro` 真 LLM 调用接口 + schema 校验 + SQLite artifact；P8 E2E 断言 expansion/decomposition | 旧 Prompt 逐字语义、用户锁定字段、自动有限修复 |
| P8.1 Character Cast | IMPLEMENTED / PARTIAL | 3 成员 + 2 关系校验；Gate 确认；P8 E2E 断言 | 拒绝、重新生成、人物事实表落库 |
| P8.2 Volume Strategy | IMPLEMENTED / PARTIAL | volumes schema、唯一序、Gate；E2E | 旧版卷关系表、重新规划、人工编辑保护 |
| P8.2 Chapter Plan | IMPLEMENTED / PARTIAL | chapterOrder 唯一、空计划拒绝、卷/人物引用校验；E2E | 受影响下游局部失效、重试去重、编辑器写回 |
| P8.3 Context | PARTIAL | artifact 级上下文组装，记录 tokenBudget | 世界/角色状态/历史正文/知识库/完整预算裁剪/脱敏 trace |
| P8.3 Consistency Review | IMPLEMENTED（最小合同） | 结构化 issues、空 clean、malformed/失败 pause；E2E clean 路径 | 旧版完整审校 Prompt、真实问题样本、质量债 |
| P8.3 Bounded Repair | NOT_IMPLEMENTED | `bounded_repair` 仍是显式占位 Step | 修复预算、原始候选、修订历史、人工审批 |
| P8.4 Tauri UI | NOT_STARTED | typed transport 已有 openRun/approve/cancel/replay，但 Director UI 未接 | 页面、Gate、取消/暂停/恢复、进度、质量、历史、跳转 |
| P8.5 E2E happy path | VERIFIED（Scripted Mock） | `structured_director_loop...` 1 test | 真实 Provider |
| P8.5 异常覆盖 | PARTIAL | 已有取消、SSE 中断、非法 JSON/schema 单测、审批等待、恢复测试 | 审批与取消竞态、连续确认、重规划编辑冲突、真实进程崩溃、预算耗尽全链路 |

## 测试命令

```bash
cargo test --workspace --exclude novel-http
# 216 tests PASS（P8 实现后）

cargo test -p novel-application --test p8_director_loop
# 1 PASS

cargo test -p novel-application --lib structured_validation_tests
# 4 PASS

cargo test -p novel-application --test recovery
# 6 PASS
```

## Mock 与真实 Provider

- P8 E2E 使用 `ScriptedLlm`，返回真实结构的 Story Macro/Cast/Volume/Chapter/Review JSON，然后走真实 Rust Runtime、真实 SQLite、真实 Workflow/Lease/CAS/Gate/Commit。
- 这不是真实 Provider 成功；真实模型 JSON 遵从度、usage、限流、网络中断仍需批准后验证。
- `MockLlm` 不再用于 P8 结构化步骤；恢复测试中只用于 draft/故障注入。

## GUI

- WebView Channel/Commands 已有 P7 真实 smoke。
- P8 自动导演页面和 Gate 未迁移，状态 **NOT_STARTED / BLOCKED**；没有用路由 smoke 冒充页面审批联调。

## 结论

P8 核心后端切片已经不再是占位产物，但完整 P8 自动导演（旧版 UI、真实 Prompt 全语义、修复循环、重规划保护）尚未完成，不能宣布 P8 全部 VERIFIED。
