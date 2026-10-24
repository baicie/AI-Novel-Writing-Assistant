# P14.0 Acceptance

基线：`3bc699dd` + P14 remediation commits。  
状态：P0 安全问题优先修复；P14.0 整体 **PARTIAL**，不得全量 VERIFIED。

## P14.0-A 冲突原子性

| 验收项 | 状态 | 证据 |
| --- | --- | --- |
| Confirm 成功/裁决失败 rollback | **PARTIAL** | 单事务实现；测试覆盖 stale conflict/candidate 不写 fact/decision、并发只写一次；无专项决策插入失败注入 |
| 冲突 Version 并发 | VERIFIED | expectedVersion CAS + 并发测试 |
| Candidate Revision 过期 | VERIFIED | chapterVersion/hash 校验 |
| 两窗口并发裁决 | VERIFIED | 同一 conflict 两请求仅一成功 |
| commandId 相同参数 Replay | VERIFIED | request_hash replay |
| commandId 不同 Decision Conflict | VERIFIED | request_hash 拒绝 |
| supersede 旧事实不进入 Context | VERIFIED | fact_ids 标记 superseded，list_story_facts 不返回 |
| Restart 后历史 | PARTIAL | decision 持久化；未做独立进程重启测试 |
| Native Conflict WDIO | 待最终 HEAD 重跑 | 见 Native 段 |

## P14.0-B Knowledge Source Integrity

| 验收项 | 状态 | 证据 |
| --- | --- | --- |
| 通用 Command 不能覆盖章节索引 | VERIFIED（命令守卫） | Tauri `index_knowledge` 拒绝 chapter owner/ID |
| 通用 Delete 不能删章节索引 | VERIFIED（存储+命令路径） | `delete_document` 拒绝；测试 |
| 旧 Revision 不覆盖新 Revision | VERIFIED | `@vN`/`vN` 比较；测试 |
| SourceRevision 改变更新元数据 | VERIFIED | 同正文不同 revision 不再早返回 |
| 旧索引不进入 Context | PARTIAL | building/failed/ready 过滤沿用；无新并发验证 |
| 跨 Novel ID 冲突 | VERIFIED | novel_id 不一致拒绝；测试 |
| SQLite 并发/失败回滚 | PARTIAL | 单请求事务/测试；无两并发 index 压测 |

## P14.0-C Durable Job Correctness

| 验收项 | 状态 | 证据 |
| --- | --- | --- |
| Lease 过期旧 Worker Complete 拒绝 | VERIFIED | claim_epoch + epoch test |
| 同 Owner 新 Claim 不混淆旧执行权 | VERIFIED | epoch 递增 |
| 旧 Revision Job 不覆盖新索引 | VERIFIED | complete 校验 sourceRevision |
| 目标已删除不能成功 | VERIFIED | complete 检查 document |
| SIGKILL / 新进程恢复 | PARTIAL | lease 过期恢复测试；无 Native SIGKILL |
| 重复投递幂等 | VERIFIED | command_id unique |
| Retry Exhausted | VERIFIED | maxAttempts test |
| succeeded 对应实际 Revision | VERIFIED | complete 校验 |
| 自动 Enqueue | **NOT_IMPLEMENTED** | 仍同步章节索引 |
| 启动恢复执行 | PARTIAL | 30s 扫描过期 lease；无自动执行 worker |
| Native Retry/Recover WDIO | 未完成 | — |

## P14.0-D Native 与检索

### FTS 排序

- 增加 `ORDER BY bm25(knowledge_fts), c.id`；
- Vector 同分按 chunk_id tie-break；
- 50 问题基准 MRR 从 0.76 提升到 1.00（FTS 冻结集）；
- FTS/Vector/Hybrid 对照结果见 P13_RETRIEVAL_COMPARISON.md；
- 不将 mock embedding 结果当作真实语义能力。

### Native 完整旅程

已具备/已验证：

- 创建小说、章节、Director 运行、Candidate/Approval/Commit；
- 记忆抽取/确认/冲突裁决；
- 知识 CRUD；
- Editor 草稿、Candidate Journal、SIGKILL/Recovery 既有测试。

未完成：

- 章节历史查看/恢复 UI；
- 连续多章节创作 UI；
- 分章/范围导出；
- 任务中心完整状态操作；
- Index Retry/Recover Native WDIO。

### Native 回归

最终 HEAD 结果：

```text
Director              4 passing
conflict              1 passing
memory-extraction     1 passing
knowledge CRUD        1 passing

editor-crash          未通过（WDIO 在主动 SIGKILL 后连接断开）
candidate-journal-crash 未通过（同类 SIGKILL 连接问题）
editor-crash-restart / candidate-journal-restart
restart / confirm-edit-restart
  失败原因：这些两进程 spec 需要同一临时数据目录的 crash 前状态文件；
  单次独立运行没有状态文件，不能据此判定产品数据安全回归。
Index Retry          未完成 WDIO
```

因此 Native 数据安全 G7/D 仍为 PARTIAL；不把 crash spec 失败写成产品通过。

## 最终验证

```text
cargo test --workspace --exclude novel-http
275 passed, 0 failed, 688.70s

clean worktree cargo test --workspace --all-targets
334 passed, 0 failed（实际执行）

pnpm test
186 passed, 0 failed

pnpm typecheck / build
PASS

cargo fmt --check
PASS

cargo clippy --workspace --exclude novel-http --all-targets
PASS（既有 warning）

Production .app
21.40 MiB

DMG
9.5 MiB，hdiutil verify VALID

Commands / Handlers / Capabilities
89 / 89 / 89

Tauri release audit
PASS
```

## 未完成 / P14 Backlog

1. 完整冲突类型（地点、死亡、时间线循环、伏笔重复回收）。
2. 自动索引 enqueue + 启动执行 worker + SIGKILL。
3. 两并发 index 写与 building 恢复。
4. 真实 Embedding/Provider 评测（需预算授权）。
5. Native 历史恢复、多章、范围导出、任务中心。
