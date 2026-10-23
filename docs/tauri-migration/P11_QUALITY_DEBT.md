# P11.3 Quality Debt Lifecycle

## 当前状态

**PARTIAL（审计）**

已具备：

- `chapter_quality_debt` 持久化
- `UNIQUE(run_id, issue_id)`
- `open / resolved / accepted` 状态字段
- `resolution_evidence`
- review_version
- 重复 issueId 不同内容拒绝
- 质量债与 Workflow 停止信号分离

已确认缺陷/缺口：

1. **模型 issueId 不保证跨审校稳定**
   - 未提供稳定 id 时，按 issue 内容哈希生成；证据变化会成为新问题。
   - 当前没有独立的 Review Issue Occurrence 表。
2. **accepted 无命令/UI**
   - 表结构允许 accepted，但没有 Rust Command、CAS、操作者、理由、决策版本。
3. **解决证据缺乏 review artifact 外键**
   - 只有文本 evidence，没有显式 artifact id。
4. **跨轮关联依赖模型 id**
   - 不能可靠把同一业务问题跨审校关联起来。

## 后续最小方案

- 新增 `review_issue_occurrence`（review artifact/attempt 维度）
- Quality Debt 独立稳定 id
- 确定性 matching key（type + sourceRef + normalized evidence），不确定则保持两条
- `accept_quality_debt` Command：CAS + operator + reason + decision version
- resolved/accepted 必须绑定证据来源和时间
- 历史不删除、不覆盖

## 本轮结论

P11.3 Gate **未完成**，不得标记 VERIFIED。
