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

Chunk 表已记录 source_revision/chunk_hash，但 SearchHit 原先只返回 chunk_id/owner/body。

状态：**FIXED**（SearchHit 现在返回 sourceRevision、chunkHash、indexVersion）。

### P2-2 正式章节变更不会自动重建索引

原实现只由显式知识导入调用，章节正式版本更新不触发索引。

状态：**FIXED**（save_user_chapter / promote_candidate / delete_chapter 同事务同步 chapter 索引）。

### P2-3 时间衰减可能降低长期伏笔可见性

RRF/decay 是排序函数，Context Assembly 主要按 source priority/budget；尚未实现“明确事实约束优先于衰减”的统一策略。

状态：**OPEN**。

## 安全

- promptText 只存储在本地 workflow artifact；未发现外部导出/上报路径。
- 生产 Bundle 未包含 WDIO、E2E Key、recovery_worker。
- 用户 untracked `integration_runs.rs` 未触碰。


### P2-4 长期事实写入链路未闭环

Character State / Timeline / Plot Thread 已有读取与 Context 集成，但没有 Native 写入 Command/UI 与 AI 抽取确认链路。

状态：**OPEN（P13.2 PARTIAL）**。

### P1-4 零/超大输出预算在 DNS 解析之后才校验

`OpenAiCompatibleProvider::generate` 原先先做 endpoint DNS 授权，再检查 max_output_tokens；0 预算测试可能先触发网络错误。

状态：**FIXED**（预算校验前移到任何 DNS/网络调用之前）。
