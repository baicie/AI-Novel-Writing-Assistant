# Editor Data Safety

基线：`6e230499`。本轮增加独立 SQLite 本地草稿 Journal，不再仅依赖 `sessionStorage`。

## 1. 数据合同

表：`chapter_local_draft`

| 字段 | 含义 |
| --- | --- |
| `novel_id` | 小说隔离 |
| `chapter_id` | 章节隔离 |
| `client_id` | 编辑器实例/窗口隔离 |
| `content` | 未正式提交的作者正文 |
| `base_official_version` | 草稿基于的 official version |
| `base_updated_at` | 草稿基于的章节更新时间 |
| `local_revision` | 本地草稿单调序号 |
| `updated_at` | 本地草稿更新时间 |

规则：

- 本地草稿写入不创建 `chapter_revision`。
- 正式保存仍走 `save_chapter` 的 `expectedVersion` CAS。
- 正式保存成功后只清除：
  - 同一 `novel_id/chapter_id/client_id`
  - `content` 与本次提交正文完全匹配
  - `local_revision <=` 提交时已知草稿版本
- 用户在保存中继续输入时，较新草稿不会被删除。
- 正式保存不会覆盖/删除其他窗口或其他 client 的草稿。
- 启动时先按当前 client 取草稿；没有则读取该章节其他草稿，保留冲突标记。
- 草稿内容不写日志、不写错误消息、不写密钥存储。

## 2. 失效场景矩阵

| 场景 | 状态 | 依据 |
| --- | --- | --- |
| 输入后尚未正式保存 | VERIFIED | autosave 写 SQLite；E2E local draft saved |
| 保存请求在途继续输入 | VERIFIED | `acknowledgeChapterSave` 保留较新本地输入；单测 |
| 旧保存响应迟到 | VERIFIED | `acknowledgeChapterSave` 只在 content===submitted 时更新；单测 |
| 保存请求失败/超时 | PARTIAL | UI 显示“保存失败”，不伪装成功；无网络故障 E2E |
| 切换章节 | VERIFIED | E2E 保存→切章→返回正文一致；草稿按章隔离 |
| WebView 刷新 | VERIFIED | WDIO refresh 后恢复 |
| 普通窗口关闭 | PARTIAL | beforeunload/pagehide flush + durable draft；无 GUI 真实关闭 E2E |
| 应用进程 SIGKILL | VERIFIED | WDIO 强制 SIGKILL 后新进程恢复未保存草稿 |
| 操作系统异常退出 | PARTIAL | 与 SIGKILL 共用 SQLite Journal；未做真实 OS crash |
| AI 在用户编辑期间产生新版本 | VERIFIED（数据保留）/ PARTIAL（UI 冲突流程） | 正式保存不删除本地草稿；其他 client 草稿保留；AI commit 的实际 UI 冲突未 E2E |
| 两个窗口编辑同一章 | PARTIAL | `client_id` 隔离和 storage 测试；无真实双窗口 E2E |
| 中文 composition 期间保存 | BLOCKED | 系统输入法 composition 不可可靠自动驱动；列入人工验收 |

## 3. Autosave

- 本地草稿：600ms debounce，仅写 `chapter_local_draft`。
- 页面隐藏/`pagehide`/`beforeunload` 尽力 flush。
- 正式章节保存：仅显式保存/Ctrl+Cmd+S；不会因每次键入创建 Revision。
- 同一草稿写入使用 `local_revision` 单调递增。
- 明确保证级别：
  - 已成功显示“本地草稿已安全保存”的正文可跨进程恢复。
  - SIGKILL 前最后 600ms 内尚未落库的输入仍可能丢失；这是明确的输入窗口，不是完整 WAL。
  - `sessionStorage` 仅作兼容/离线 fallback，不作为唯一恢复机制。

## 4. 关键实现

- Migration：`migrations/0005_chapter_local_draft.sql`
- Storage：`crates/novel-storage-sqlite/src/novels.rs`
- Ports/DTO：`crates/novel-application/src/{ports,dto}.rs`
- Application：`crates/novel-application/src/director.rs`
- Tauri Commands：`src-tauri/src/commands/editor.rs`
  - `get_chapter_draft`
  - `list_chapter_drafts`
  - `save_chapter_draft`
  - `clear_chapter_draft`
- Frontend：
  - `client/src/platform/desktopTransport.ts`
  - `client/src/pages/novels/components/chapterEditor/document/useChapterDraft.ts`
  - `ChapterEditorShell` 显示“本地草稿已安全保存 / 保存失败 / 版本冲突”。

## 5. 自动化证据

Rust：

```text
cargo test -p novel-storage-sqlite --test chapter_draft
1 passed
```

覆盖：

- 章节/窗口隔离
- 旧 revision 草稿不能被错误清除
- 匹配 content/revision 才清除
- 正式保存不删除其他窗口草稿
- 本地草稿不创建 chapter_revision

Native UI：

```text
test:editor
2 passing（autosave + refresh；official save + switch）
test:editor-crash
expected exit 1（测试进程主动 SIGKILL App）
test:editor-crash-restart
1 passing（新 App 进程恢复 SIGKILL 前草稿并正式保存）
```

## 6. 已知风险

- 600ms debounce 窗口内强制 SIGKILL 仍可能丢最后输入。
- 系统拼音 composition 期间保存未自动验证。
- AI Commit 与用户保存并发的 UI 冲突提示仍不完整。
- 两个真实窗口的 UI 级冲突流程未自动化。
- 草稿无限保留，目前没有容量清理策略。
