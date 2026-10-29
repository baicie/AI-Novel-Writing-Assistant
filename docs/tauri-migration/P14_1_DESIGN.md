# P14.1 Design

## 当前架构与权威数据

```text
React Native 页面
  → desktopTransport invoke
  → src-tauri commands
  → Application Runtime / Director
  → SQLite repositories (WAL)
```

| 数据 | 权威来源 | 写入边界 |
| --- | --- | --- |
| 正式章节正文 | `chapter_revision` + `chapter.official_revision_id` | `save_user_chapter` / candidate commit / restore 单事务 |
| 本地草稿 | `chapter_draft` | 独立 CAS，不修改正式 Revision |
| Candidate | `workflow_*` + journal | Approval 前不写正式正文 |
| 长期记忆 | `story_fact` / candidate | conflict 裁决单事务 |
| 知识索引 | `knowledge_document` / chunk / FTS | index 写锁 + revision CAS |
| 索引任务 | `knowledge_index_job` | claim_epoch + lease + source CAS |
| 历史恢复 | 追加新 `chapter_revision` | 不删除旧版本 |

## 阶段依赖

```text
S0 审计
  → S1 Crash
  → S2 Index Worker
  → S3 Integrity
  → S4 History
  → S5 Export
  → S6 Multi-Chapter
  → S7 Task Center
  → S8 Memory
  → S9 Acceptance
```

S1/S2 依赖 SQLite 持久化与 E2E 编排；S4 依赖 S3 的正文保护；S5 只读正式 Revision；S6 依赖单章 RunContract；S7 只投影状态，不直接写状态。

## S4 章节历史恢复

```text
list_chapter_revisions
  → 历史版本列表

restore_chapter_revision
  → 校验 novel/chapter/revision 归属
  → 校验 expected officialVersion
  → 插入新的 chapter_revision（origin user, 新 version）
  → 更新 official_revision_id
  → 失效旧记忆
  → workflow_artifact(chapter_restore) 记录 restoredFrom
  → 同步章节知识索引 + Job
  → 单事务 commit
```

约束：

- 不覆盖、不删除历史；
- 恢复正文先生成新正式 Revision；
- 旧 version 的 CAS 失败即拒绝；
- 未保存 Local Draft 由 UI 明确确认后再载入/恢复；
- 历史 UI 只调用 Tauri Command，不直接写 SQLite。

## 失败恢复

| 失败 | 行为 |
| --- | --- |
| 进程 SIGKILL | SQLite/草稿/Job 持久化；新进程恢复 |
| Lease 过期 | recovery 置 retry_wait，新 Worker 重新 claim |
| 旧 epoch 完成 | Fencing 拒绝 |
| 来源 revision 变化 | obsolete，不覆盖新索引 |
| 历史恢复 CAS 失败 | Conflict，不写新 Revision |
