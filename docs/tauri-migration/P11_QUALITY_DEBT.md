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


## P11.3 实现（本轮）

### 数据模型

新增 `review_issue_occurrence`：

- 每次审校实际发现的问题一条；
- 绑定 run/chapter/novel、review_version、model_issue_id、stable_issue_key；
- 保留 type/severity/description/evidence/suggestion/sourceRef；
- 不删除历史 occurrence。

Quality Debt 增加：

- `decision_version`：accepted 决策 CAS 版本；
- `accepted_reason`：作者接受原因。

### 稳定关联

`stable_issue_key = hash(normalized type + sourceRef + normalized evidence)`。

- 不依赖模型 `issueId`。
- 证据/引用无法确定相同则生成不同 key，保持两个独立问题。
- 相同 key 的 open debt 只更新审校版本/严重度/证据，不覆盖 accepted/resolved。
- 同一轮重复 occurrence 都保留，Quality Debt 去重。

### 生命周期

- open：审校发现。
- resolved：无问题审校通过；记录 resolution_evidence（含审校版本）。
- accepted：Native Command `accept_quality_debt`，必须 expected `decision_version` + 非空原因。
- accepted 不等于已修复；状态独立保留。
- 历史不删除。

### Native UI

质量问题行显示状态；open 状态提供“接受并保留”，调用真实 Tauri Command，失败展示冲突。

### 测试

```text
cargo test -p novel-storage-sqlite --test workflow_lease
- quality_debt_accept_is_versioned_and_preserves_history PASS

cargo test -p novel-application --lib stable_issue
- stable_issue_identity_ignores_model_issue_id_and_normalizes_evidence PASS

p9_repair_flow / p11_repair_faults
PASS（occurrence 写入与审校流程）
```

未完成：

- 同一业务问题跨两次审校的端到端断言（目前由稳定 key 与 occurrence 表设计保证，未加专门 fixture）。
- 质量债详情 UI 仅显示摘要、状态和接受按钮，未展示完整证据/解决历史。
- resolved 与具体 Review Artifact 外键仍未显式关联（使用 review_version 文本证据）。
