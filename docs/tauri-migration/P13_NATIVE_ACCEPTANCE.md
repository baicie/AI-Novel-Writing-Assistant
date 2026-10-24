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


第 4 轮新增：Director 长篇记忆区域支持作者记录时间线事实（真实 Tauri Command）。


第 4 轮最新构建：

```text
Director 4 passing（含长篇记忆面板与时间线记录表单）
```


第 5 轮新增：

- 长篇记忆三种编辑表单；
- 知识索引健康与重建按钮；
- 仍缺真实双窗口与知识库完整管理页 Native 验收。


第 5 轮最新构建：

```text
Director 4 passing
```

长篇记忆三种编辑表单、知识索引健康与重建按钮已进入页面；尚未对表单提交做 WDIO 自动化。


第 6 轮新增：

```text
test:memory 1 passing（时间线 + 伏笔 + 人物状态）
```


第 6 轮最新构建：

```text
Director 4 passing
test:memory 1 passing（时间线 + 伏笔 + 人物状态）
```


第 8 轮最终构建：

```text
test:memory 1 passing（时间线 + 伏笔 + 人物状态）
Director 4 passing（上一构建同一 UI 面）
```

`.app` 20.74 MiB；DMG 9.3 MiB；audit PASS；Tauri Commands 75。


第 9 轮新增：

```text
test:memory-extraction 1 passing
```


第 10 轮新增：

```text
test:knowledge 1 passing（新增 / 编辑 / 删除）
```


## 最终 Native 回归

```text
Director 4 passing
test:knowledge 1 passing
test:memory 1 passing
test:memory-extraction 1 passing
```

生产包 `.app` 21.04 MiB，DMG 9.4 MiB，audit PASS。
