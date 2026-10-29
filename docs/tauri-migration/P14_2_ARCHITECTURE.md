# P14.2 Architecture

## Scope

P14.2 把 P14.1 的章节历史/单章导出切片推进为可审计恢复与完整导出，并为后续批量创作预留边界。

## 模块关系

```text
React (NovelChapterEdit / History / Export)
  → desktopTransport invoke
  → Tauri command adapter
  → Application Director / Export
  → SQLite repository (transaction + CAS + provenance)
  → WAL SQLite
```

## Authority

| 数据 | 权威 | 写入 |
| --- | --- | --- |
| 正式正文 | `chapter_revision` + `chapter.official_revision_id` | save / candidate commit / restore 单事务 |
| 恢复来源 | `chapter_revision.origin_detail/restored_from/...` + `workflow_artifact(chapter_restore)` | restore 事务 |
| 本地草稿 | `chapter_local_draft` | 独立 writer/client CAS |
| 导出 | 正式 `content` | 只读 Application |
| 索引任务 | `knowledge_index_job` | claim_epoch + lease + source rank |
| 索引正文 | `knowledge_document`/chunk/FTS | revision rank CAS |

## S1 Revision Provenance

迁移 `0018_chapter_revision_provenance.sql`：

```text
origin_detail TEXT
restored_from_revision_id TEXT
command_id TEXT (partial unique)
restored_by TEXT
```

恢复事务：

1. `command_id` replay 检测；
2. Current official version CAS；
3. 获取写锁；
4. 检查 chapter_local_draft：
   - reject → 冲突；
   - keep → 保留；
   - discard_own → 必须 clientId + localRevision 匹配，且不能有别人的草稿；
5. 插入新正式 Revision（`origin_detail=restored_from_history`）；
6. 更新 official；
7. 失效记忆；
8. `chapter_restore` artifact；
9. 同步知识索引 + Job；
10. commit。

恢复来源不可删除；旧 Revision 永远保留。

## S2 Export Contract

```text
export_novel_selection(
  novelId,
  selection: chapter|range|entire_book|chapter_ids,
  format: markdown|txt,
  chapterId?, chapterIds?, startOrder?, endOrder?
)
```

- Application 只查询该 novel 的 chapter；
- 只读取正式 `content`；
- 按 `sort_order` 输出；
- Markdown/TXT 规范化；
- 当前 Native 以 Blob 下载保存，尚无 Tauri 文件保存对话框；这是 PARTIAL 项。

## Batch / Task Center 预留

Batch 不新建 AI 工作流，只编排现有单章 RunContract：

```text
Batch → item 1 → existing RunContract
      → item 2 → ...
```

Task Center 只做各领域状态投影，写操作代理到领域 Command。P14.2 未实现 S3–S8。

## Failure / Recovery

| 失败 | 结果 |
| --- | --- |
| 恢复 command 重放 | 返回当前章节，不新增 Revision |
| 恢复草稿竞态 | 写锁内读取，冲突或保留 |
| 导出跨 Novel | 查询集合仅限定 novelId |
| Index 旧 revision | rank CAS 拒绝 |
| Worker SIGKILL | lease 过期 + epoch 恢复 |
