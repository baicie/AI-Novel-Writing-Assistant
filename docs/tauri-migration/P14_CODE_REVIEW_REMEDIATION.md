# P14.0 Code Review Remediation

基线：`3bc699dd`  
状态：P14.0-A/B/C **PARTIAL–IMPLEMENTED**；P14.0-D 见 P14_ACCEPTANCE.md。

## P14.0-A Memory Conflict Atomicity

修复前问题：

```text
MemoryService::resolve_conflict()
→ confirm/reject/defer（独立事务）
→ Repository 记录裁决（第二个事务）
```

事实与裁决不是原子操作；`supersede` 只是确认别名；`keep_existing` 只处理一个候选；commandId 未绑定请求参数。

修复：

- `resolve_memory_conflict` 改为**单 SQLite 事务**：
  - conflict expectedVersion/status；
  - chosen candidate 属于 conflict；
  - candidate status/identity 校验；
  - 来源 chapterVersion + content hash 校验；
  - 写入/更新 fact；
  - 拒绝其余冲突候选；
  - supersede 旧 fact IDs；
  - 写 decision（含 `request_hash`）；
  - 更新 conflict version；
  - 全部成功才 commit。
- 新增 `0015_conflict_request_hash.sql`。
- `supersede_with_evidence` 要求 conflict 有 `fact_ids`，并真正标记旧事实 `superseded`。
- `keep_existing` / `reject_extraction` 拒绝**全部**候选。
- `mark_disputed` 将所有候选转为 deferred，不进入 confirmed Context。
- commandId + requestHash：相同请求 replay；相同 commandId 不同参数 Conflict。

测试：`p13_memory_extraction` 9 passed：

- stale conflict version / stale candidate revision 不写 fact/decision；
- keep existing 拒绝全部；
- supersede 旧事实不进入 Context；
- 两并发裁决只有一个成功；
- 相同请求 Replay，不同请求 Conflict；
- 裁决历史持久化。

未达到：

- 仍是候选冲突/过期来源两类；
- 未实现地点/死亡/时间线循环检测；
- 未做真实 process SIGKILL，只做事务并发/回滚。

## P14.0-B Knowledge Source Integrity

修复：

- 通用 Tauri `index_knowledge` 拒绝 `owner_type=chapter` 或 `chapter:` ID。
- 通用 `delete_document` 拒绝章节索引（chapter owner / `chapter:` id）。
- `index_document` 校验：
  - existing document novel_id 必须一致，不能跨小说转移；
  - source revision 带 `@vN`/`vN` 时，旧 revision 不能覆盖新 revision；
  - 相同正文但不同 revision 不再早返回，元数据会更新。
- 正式章节仍通过 `sync_chapter_knowledge` 受控原子路径更新。

测试：`p13_source_integrity` 2 passed：

- v1 不能覆盖 v2，v3 可以更新；
- 同 ID 跨小说被拒绝；
- 通用删除章节索引失败且内容保留。

未达到：

- 两个并发 index 请求的 building/last-writer 竞争未做真实并发测试；
- 失败事务留下 building 的恢复路径沿用现有 retry，未新增调度。

## P14.0-C Durable Index Job Correctness

修复：

- `0016_index_job_epoch.sql` 增加 `claim_epoch`。
- Claim 递增 epoch。
- Claim/Complete/Fail/Obsolete 全部校验：
  - status running；
  - leaseOwner；
  - claimEpoch。
- Complete 额外校验：
  - 来源 document 存在；
  - sourceRevision 与 job 一致；
  - indexVersion 不低于 job 要求。
- `retry_index_document() == false` 不再无条件 succeeded，改为 obsolete。
- 旧 revision / 已删除来源标记 obsolete 并保留错误。
- Tauri 启动增加 30s lease 过期恢复扫描。
- 手动任务使用 `tauri-manual-{timestamp}` owner，非固定长期 owner；
- failed/obsolete 任务可 reset 后手动重试；
- 空 sourceRevision 的用户文档不因 `None` 被误判 stale。
- Native 索引队列 UI 有刷新/重试。

测试：`p13_index_jobs` 3 passed：

- enqueue 幂等；
- lease + epoch 领取；
- 旧 owner/epoch Complete 拒绝；
- 只读 false/旧 revision 不能 Complete；
- source revision 变化后 Complete 拒绝；
- 过期 running 恢复 retry_wait；
- maxAttempts -> failed；
- obsolete 状态。

未达到：

- 章节保存尚未自动创建 pending Job（仍同步索引 + 无 job）；
- 真实 SIGKILL/新进程执行的 Native 测试未完成；
- Index Retry/Recover Native WDIO 未完成。

## 结论

P0 冲突原子性、知识来源授权/跨小说、Job fencing 已实际修复并有测试。  
P14.0 整体仍是 PARTIAL，剩余集中在自动 enqueue、真实 SIGKILL/Native 和完整产品旅程。
