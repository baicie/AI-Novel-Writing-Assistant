# P13.0 Latest Code Audit

基线：`37c86123`。

## 已审计合同

- Lease Renew：Worker Tick 内启动/取消续约任务；续约不改变 Control Version。
- Cancel / Fencing：持久化 Cancel 使用 Control 状态；Worker 写入校验 Owner + Version + Lease。
- `save_candidate_checkpoint`：Candidate + Attempt + Invocation + Event 单事务。
- RecoveryDecision：RemoteStarted / CandidateSaved / Committed 分支，v1/v2/v3 兼容。
- Quality Debt：Occurrence + 稳定 key + accepted decision_version。
- FTS / Vector / Embedding / RRF / Context Assembly。
- `chapter_arc_summary` 从冻结 chapter_list 生成。

## 真实缺陷

### P1-1 索引重建不是幂等更新

原 `index_document` 直接 INSERT 固定主键；同一 document 重新索引会主键冲突，旧 chunk 无法替换。

状态：**FIXED**（`knowledge.rs` 重建为 building → 替换派生行 → ready）。

### P1-2 旧索引在重建期间仍可命中

原查询不过滤 `index_status`，且重建无 building 状态。

状态：**FIXED**（查询只接受 `index_status='ready'`；重建失败保持 building，不可命中）。

### P1-3 删除来源没有索引清理

无 `delete_document` 方法，删除正式来源后旧 chunk 仍可能被检索。

状态：**FIXED**（新增 `delete_document` 与 Tauri Command）。

### P2-1 SearchHit 不携带 sourceRevision

当前 chunk 表已记录 `source_revision`/`chunk_hash`，但 `SearchHit` 只返回 chunk_id/owner/body，Native UI 无法直接展示来源修订。

状态：**OPEN**，P13.1 后续补 API。

### P2-2 正式章节变更不会自动重建索引

目前 `index_document` 只由显式知识导入/Tauri 命令调用；章节正式版本更新没有自动触发章节知识索引。

状态：**OPEN / PARTIAL**。

### P2-3 时间衰减可能降低长期伏笔可见性

RRF/decay 是排序函数，Context Assembly 主要按 source priority/budget；尚未实现“明确事实约束优先于衰减”的统一策略。

状态：**OPEN**。

## 安全

- promptText 只存储在本地 workflow artifact；未发现外部导出/上报路径。
- 生产 Bundle 未包含 WDIO、E2E Key、recovery_worker。
- 用户 untracked `integration_runs.rs` 未触碰。
