# P9 Business Migration Plan

基线：`e7fe2379`（P9 开始时）。本文件以实际源码为准，不代表旧版所有能力已经迁移。

## 1. 旧版入口与 Rust 当前实现

| 业务能力 | 旧版入口 | Rust 当前 | 合同差异 | 数据依赖 / Prompt | 迁移方案 | 风险 | 验收 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Context | `server/src/prompting/prompts/novel/context/*`、`application/factsLoader.ts` | `crates/novel-application/src/context.rs`（P9.1） | 旧版多块 context policy/预算；Rust 现为固定优先级、真实来源、trace、估算 token | SQLite world/character/chapter，workflow artifacts；`chapter-draft-v2` | 已完成最小纵向切片；知识库/RAG 保留扩展点 | 旧版复杂 policy 未全迁移 | P9.1 tests |
| Story/Cast/Volume/Chapter plan | `director/steps/*` | `worker_structured` + prompt v1 | 基本最小结构已迁移 | artifacts + SQLite | 保持 v1 prompt；新 run 可扩展 | 旧 schema 覆盖范围更广 | P8/P9 |
| Consistency Review | old review issues/quality loop | `consistency_review` | 当前仅 issues 数组校验；无持久质量债、无 re-review | 候选正文 + context | P9.2 | 失败/非法输出必须不变成 clean | P9.2 |
| Bounded Repair | old repair/patch loop | `bounded_repair` 当前 fail-closed | 无真实 Repair Plan/Candidate/Re-review | candidate, attempts, artifacts | P9.2，默认预算 0 仍安全暂停 | 旧 Run 不能静默换执行规则 | P9.2 |
| Quality Debt | old quality debt | 未持久化 | 无状态/证据/版本 | SQLite | P9.2 新增 | 不能第二套账本 | P9.2 |
| Approval decisions | old gate decisions | 只有 approve | 缺 Reject/Regenerate/Confirm After Edit | artifacts/candidates/control CAS | P9.3 | 不能映射成 approve | P9.3 |
| Director UI | `client/src/features/directorNext` | Native MVP approve only | 决策入口不全 | Tauri commands | P9.3 | 按钮必须来自合法动作投影 | P9.3 |

## 2. 不可破坏的约束

- RunContract 冻结；旧 Run 的 `plan_version` 不做静默迁移。
- 正式正文只走 CAS；Local Draft 独立，不被 AI 删除。
- 模型失败、超时、SSE 中断、非法 JSON 不等于 `issues:0`。
- 修复只处理候选，不直接覆盖正式正文。
- 任何自动修复都受冻结的重试/预算策略限制。
- 取消、Lease 丢失、过期 Gate 后不能提交。

## 3. 分阶段

1. **P9.1 Context**：真实来源、预算、trace、draft 使用 context。
2. **P9.2 Review/Repair**：质量债、有界修复、re-review、取消/崩溃测试。
3. **P9.3 Decisions**：Reject/Regenerate/Confirm After Edit 与 Native UI 合法动作投影。

## 4. 当前状态

- P9.1：IMPLEMENTED / 测试中
- P9.2：NOT_STARTED（本轮继续）
- P9.3：NOT_STARTED（本轮继续）
