# P13.7 Index Retry Lifecycle

## 已有

- building 状态不可查询；
- `retry_index_document` 重建 building 文档；
- ready 文档重复重试返回 false；
- 删除来源清理 FTS/embedding/chunk；
- 章节修改自动替换索引。

## 测试

```text
knowledge_lifecycle
3 passed
```

## 缺口

- 无持久化 retry 计数/失败原因/next_run_at；
- 无后台调度；
- 无应用重启恢复任务；
- 无 Native 编辑知识条目 CAS。
