# P13.4 Native Acceptance

## 状态：PARTIAL

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


本轮实际执行：

```text
Director 4 passing
Editor 2 passing
Candidate Journal SIGKILL + restart 1 passing
```

未执行：

- 原生知识库管理 UI 新增/编辑/删除验收
- 真双窗口
- 知识删除命令 UI


## 最终 Native 回归（本轮）

```text
Director 4 passing
Editor 2 passing
Candidate Journal SIGKILL crash + restart 1 passing
```

知识库管理 UI 未做 Native 新增/编辑/删除验收，保持 PARTIAL。


新增 Tauri 命令但未做 UI：

- upsert_character_state
- upsert_timeline_event
- upsert_plot_thread
- list_story_facts


第 2 轮最新构建实测：

```text
Director 4 passing
```

Editor 2 passing / Candidate Journal SIGKILL + restart 1 passing 来自同一 UI 面的上一构建；故事事实写入只新增 Tauri Command，未改 UI。


第 3 轮新增：

- Director 只读“长篇记忆”面板；
- 索引重试 Tauri Command；
- 尚未做知识库索引状态/重试 UI 验收。


第 3 轮最新构建：

```text
Director 4 passing（含长篇记忆只读面板）
```

知识库索引状态/重试 UI 与记忆编辑 UI 未验收。
