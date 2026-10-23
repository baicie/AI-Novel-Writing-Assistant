# P9.2 Consistency Review + Bounded Repair

## 1. Review 合同

`consistency_review` 现在区分：

- `review_succeeded_no_issues`：`issues: []`
- `review_succeeded_with_issues`：结构合法、issues > 0
- Provider 失败/超时/SSE 中断：暂停 `ProviderUnavailable`，不写入成功审校产物
- 非法 JSON 或缺字段：暂停 `DataIntegrity`，不转换成 `issues: 0`

每个 Issue 规范化为：

- `issueId`：模型提供或由内容哈希稳定生成
- `type`
- `severity`
- `description`（缺省用 evidence）
- `evidence`
- `suggestion`
- `sourceRef`（可空）

## 2. Quality Debt

新增迁移 `0006_quality_debt.sql`：

- 绑定 `novel_id / chapter_id / run_id`
- 保存 issue 类型、严重程度、description、evidence、suggestion、source ref
- 保存审校 artifact 版本
- 状态：`open / resolved / accepted`
- `UNIQUE(run_id, issue_id)`，重复审校不会创建重复行
- 解决只写 `resolved + resolution_evidence`，不删除历史
- `RunDetailView.qualityDebt` 暴露给 Native/客户端

## 3. Bounded Repair

新计划版本：

- `novel-production-v2`：新 Run
- `novel-production-v1`：旧 Run 保留原计划语义；`bounded_repair` 继续人工暂停

执行链：

```text
Draft Candidate
→ Consistency Review
→ Quality Debt
→ bounded_repair（仅当 issues > 0）
→ Repair Candidate（新 candidate，原候选保留）
→ Re-review（consistency_review v+1）
→ 问题消失 → approval
→ 问题仍在 → Safety Pause（不 Commit）
```

约束：

- `max_automatic_retries` 是冻结合同中的修复预算；默认 0 时仍安全暂停，不自动修复。
- 每次模型调用有 attempt、request id、usage、invocation 记录。
- 修复失败/超时/取消不会 Commit。
- 原 candidate 与修复 candidate 都保留。
- 修复后强制重新审校；不自动提升正文。
- 正式正文仍通过 `promote_candidate` 和 Approval Gate。

## 4. 已测试

`crates/novel-application/tests/p9_repair_flow.rs`：

- issues > 0 → repair candidate → re-review clean → approval → commit 修复候选
- issues > 0 → repair → re-review still issues → Safety Pause，无 commit，quality debt 保持 open
- 原候选和修复候选同时保留
- 两次 consistency_review artifact 可追踪
- `max_automatic_retries=0` 由 P8 test 保持 fail-closed
- 真实 SQLite/Mock Provider，无真实付费模型调用
- workspace 231 tests 全通过

## 5. 未完成 / 风险

- 修复超时、SSE 中断、取消后迟到结果没有专门 E2E。
- 进程崩溃发生在 repair 中间时的恢复未专门测试（依赖 attempt/WAL，但有风险）。
- 旧 Run v1 仅验证计划版本隔离设计，未做完整恢复 E2E。
- 质量债 UI 已展示 issue 类型/严重度/状态，但尚无解决证据详情页。
- 当前 residual issues 后 worker tick 可能返回一次状态冲突（run 已 paused）；不影响数据，但需要后续修正错误传播。
