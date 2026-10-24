# P13.1 RAG Source Consistency & Index Lifecycle

## 数据模型增量

migration 0009：

- `knowledge_document.source_revision`
- `knowledge_document.source_content_hash`
- `knowledge_document.index_version`
- `knowledge_document.indexed_at`
- `knowledge_chunk.source_revision`
- `knowledge_chunk.chunk_hash`

## 重建流程

1. 对已有 document 标记 `index_status='building'`。
2. 提交“building”状态，使旧索引立即不可查询。
3. 单事务删除旧 FTS / embedding / chunk，写入新 chunk。
4. 更新 document 为 `ready`，递增 index_version，记录 source_revision/source_content_hash/indexed_at。

失败时保持 building；查询层过滤 ready，因此旧正文不会继续冒充有效事实。

## 查询约束

- FTS：`d.novel_id = ? AND d.index_status='ready' AND MATCH`
- Vector：novel + ready + model + dimensions + normalized
- 删除 document 同时删除 FTS/embedding/chunk

## 测试

`crates/novel-storage-sqlite/tests/knowledge_lifecycle.rs`：

- 旧正文重建后不再命中；
- 新正文可命中；
- building 状态不可查询；
- 重建失败模拟后重新索引恢复；
- 错误 Embedding model 返回空；
- 删除后不可检索；
- source_revision / source_content_hash / index_version 持久化。

```text
cargo test -p novel-storage-sqlite --test knowledge_lifecycle
1 passed
```

## 未完成

- SearchHit 已返回 sourceRevision/chunkHash/indexVersion。
- 正式章节保存/删除/晋升已自动同步章节知识索引。
- 正式章节保存未自动触发章节索引。
- 索引失败重试队列与后台重建未实现。
- 跨小说/未来章节的章节索引 E2E 尚未完成。


## 章节索引自动同步

`save_user_chapter`、`promote_candidate`、`delete_chapter` 在同一 SQLite 事务内同步 `chapter:{id}` 文档：

- 正文 revision 与 source_revision/hash 一致；
- 旧 chunk/FTS/embedding 在保存新修订时替换；
- 删除章节同时删除索引；
- 章节索引 source_revision 记录 revision id + version。

测试：`confirmed_chapter_revision_updates_and_deletes_chapter_index`。
