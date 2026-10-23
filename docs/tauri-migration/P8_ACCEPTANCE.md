# P8 验收结论

## 总体状态

**P8：PARTIAL。P8.1 后端纵向切片 IMPLEMENTED，P8.2/P8.3 最小结构化合同 IMPLEMENTED，P8.4 Tauri Director MVP PARTIAL，P8.5 异常覆盖 PARTIAL；P8.4.1 Runtime Provider 装配已补测。**

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
| P8.3 Bounded Repair | FAIL_CLOSED（安全暂停）/ NOT_IMPLEMENTED（自动修复） | `consistency_review` 有问题时 `bounded_repair` 直接 pause，不生成 repair artifact、不提交正文；有 P8 测试 | 修复预算、原始候选、修订历史、人工审批后的自动修复 |
| P8.4 Tauri UI | PARTIAL（MVP） | 新增 `TauriDirectorPage`/历史页/typed transport/read model；Release WebView director route smoke 验证页面和启动表单渲染 | 真实 UI 启动 Run、审批、取消、恢复的自动化/人工验收未完成；完整 Workspace、拒绝/重生成未实现 |
| P8.5 E2E happy path | VERIFIED（Scripted Mock） | `structured_director_loop...` + `malformed_story_macro...` 2 tests | 真实 Provider、审批竞态等 |
| P8.5 异常覆盖 | PARTIAL | 已有取消、SSE 中断、非法 JSON/schema E2E/单测、审批等待、恢复测试 | 审批与取消竞态、连续确认、重规划编辑冲突、真实进程崩溃、预算耗尽全链路 |

## 测试命令

```bash
cargo test --workspace --exclude novel-http
# 221 tests PASS

cargo test -p novel-application --test p8_director_loop
# 3 PASS

cargo test -p novel-application --test runtime_provider_http
# 1 PASS（真实 OpenAiCompatibleProvider + local HTTP/SSE）

cargo test -p novel-desktop --test runtime_provider
# 2 PASS（Tauri composition root provider + 真实 HTTP 流取消）

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
- P8 新增真实 Release WebView Director route smoke：路由 `#/lab/director/<novel>` 可渲染，启动表单可见。
- 没有用路由 smoke 冒充真实 Run/审批/取消 UI 联调；这些仍为 **PARTIAL / 人工验收 BLOCKED**。

## 结论

P8 后端结构化步骤和原生 Director MVP 已有实际代码与自动化证据，但完整 P8 自动导演（真实 UI 操作闭环、真实 Provider、修复循环、重规划保护）尚未完成，不能宣布 P8 全部 VERIFIED。P8.4 当前为 PARTIAL。
