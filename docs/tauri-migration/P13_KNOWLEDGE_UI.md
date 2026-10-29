# P13.7 Knowledge Native Management & Index Lifecycle

## 状态

**PARTIAL**

已实现（本轮）：

- Tauri `list_knowledge_documents`
- `index_knowledge`
- `delete_knowledge_document`
- `retry_knowledge_document`
- Director “知识索引”健康面板：非 ready 文档 + 重建按钮
- 章节正式修订自动同步索引
- sourceRevision / chunkHash / indexVersion 返回
- Director“知识索引”面板：本小说知识列表、标题筛选、新增、编辑、删除
- 编辑前读取 `get_knowledge_document` 正文
- 删除清理 FTS/embedding/chunk
- Native WDIO `test:knowledge` 通过

未完成：

- 独立知识库页面与全文搜索；当前在 Director 面板内
- 归档流程
- 持久化索引任务、重试计数、下一次执行时间、最大重试
- 应用重启后的任务恢复
- 编辑 CAS command 与 Native WDIO

## 索引任务

当前重试是同步命令，不是持久化后台队列；失败原因仍在 `memory_extraction` 之外的 knowledge document error 字段未单独持久化。
