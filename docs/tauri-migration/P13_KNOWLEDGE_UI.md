# P13.7 Knowledge Native Management & Index Lifecycle

## 状态

**PARTIAL**

已有：

- Tauri `list_knowledge_documents`
- `index_knowledge`
- `delete_knowledge_document`
- `retry_knowledge_document`
- Director “知识索引”健康面板：非 ready 文档 + 重建按钮
- 章节正式修订自动同步索引
- sourceRevision / chunkHash / indexVersion 返回

未完成：

- 完整知识条目列表/搜索/新增/编辑 Native 页面
- 归档流程
- 持久化索引任务、重试计数、下一次执行时间、最大重试
- 应用重启后的任务恢复
- 编辑 CAS command 与 Native WDIO

## 索引任务

当前重试是同步命令，不是持久化后台队列；失败原因仍在 `memory_extraction` 之外的 knowledge document error 字段未单独持久化。
