# P13.4 Native Acceptance

## 状态：PARTIAL / NOT_STARTED

本轮未修改 Native UI，仅新增索引 Tauri Command 与文档。

需要重新验证：

- 创建小说/章节
- 知识条目新增/修改/删除
- Director 运行
- Context 来源
- Review/Repair
- Quality Debt 接受
- Reject/Regenerate
- Confirm After Edit
- Commit
- 重启恢复
- Editor SIGKILL / Candidate Journal SIGKILL

知识库管理 UI 未在本轮做 Native 验收。


本轮新增 Tauri `delete_knowledge_document` 与 index source fields，但未做 Native UI 验收；不能以 Rust 测试替代。
