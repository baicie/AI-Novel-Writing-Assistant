# P9 Context Assembly

## 1. Sources

已接真实 SQLite / artifacts：

| Source | 读取 | 状态 |
| --- | --- | --- |
| Novel seed / setup | `novel` 表 | VERIFIED |
| Story Macro | `story_macro` artifact | VERIFIED（必要来源，缺失则失败） |
| Character Cast | `character_cast` artifact | VERIFIED |
| Volume Strategy | `volume_strategy` artifact | VERIFIED |
| Chapter Plan | `chapter_list` artifact，只取目标章节 | VERIFIED |
| World | `world` 表（owner_novel_id 或 novel.world_id） | VERIFIED |
| Character facts | `character` 表 + profile_json | VERIFIED |
| Confirmed previous chapters | `list_chapters` 的 official revision，仅目标章节之前 | VERIFIED |
| Recent summaries | 未迁移 | TRACE `not_migrated`，不伪造 |
| User supplement | 未迁移独立入口 | TRACE `not_migrated`，不伪造 |
| Knowledge/RAG | 未迁移 | 扩展点，不返回占位结果 |

读取失败直接向上传播；不会把数据库错误当成空上下文。

## 2. Selection Rules

- 按 `novelId` 查询；artifact scope 严格限定 `book` 或 `chapter:<id>`。
- 只取 `chapter.sort_order < target.sort_order` 的已确认章节。
- `list_chapters` 来自 official revision，不使用 `chapter_candidate`。
- 未来章节不进入上下文。
- 目标章节计划缺失时失败，不生成空计划。
- 用户修改/正式保护通过 `protection` 与 artifact status 记录。
- 旧 artifact stale 时不选中并记录 `stale_artifact`。

## 3. Budget

- 新增 `RunContract.context_token_budget`（默认 24000，旧合同反序列化使用默认）。
- 输出预留单独记录；`token_budget` 仍是 Provider output 上限。
- 估算器：CJK 每字 1，其他字符每 3 个 1 token；这是保守估算，不是 tokenizer。
- 必要来源超过输入预算：结构化错误，停止生成，不静默截断。
- 可选来源按固定优先级尝试，超出预算标记 `over_budget`。
- 固定优先级：seed → story → cast → volume → chapter plan → characters → world → recent chapters → optional notes.

## 4. Trace

`context` artifact 保存：

- `assemblyVersion`
- `targetChapterId`
- `inputBudget` / `outputReserve`
- `estimatedTokens`
- `contentHash`
- `promptText`（受预算控制）
- `sources[]`：type/id/version/status/selected/exclusionReason/estimatedTokens/contentHash

相同事实快照 + 相同配置产生相同 `promptText`/hash。完整正文只在 artifact，不写普通日志。

## 5. Prompt / Draft

- `chapter-draft-v1` 保留旧语义。
- `chapter-draft-v2` 新 run 使用，包含真实 `context` artifact。
- Native Director 默认请求 v2；旧 Run 的 v1 仍按旧 Prompt 执行。
- P9.1 测试真实 Mock Provider + SQLite，验证世界、角色、前三章进入第四章 draft prompt。

## 6. 测试结果

```text
cargo test -p novel-application --test p9_context
3 passed

cargo test -p novel-application --test p9_context_flow
1 passed
```

完整 workspace：231 passed；干净 Worktree all-targets PASS。

## 7. 未完成

- 旧版 Context Broker 的复杂多块 policy 与知识库/RAG。
- 真实模型 tokenizer 精确计数。
- 用户补充设定独立入口。
- 完整旧版 recent summary 表。
