# P11.2 Candidate Edit Draft

## 当前状态

**IMPLEMENTED / PARTIAL（核心 SIGKILL 已通过）**

P11.2 已新增：

- Candidate CAS（baseCandidateId + baseContentSha256）
- `user_edited` protected Artifact
- Native 编辑 UI
- 重启后 Author Candidate/Revision 持久化

已实现：

- SQLite `candidate_edit_draft`（migration 0007）
- 按 run/gate/client 隔离
- baseCandidateId + baseContentSha256 绑定
- localRevision CAS
- Tauri get/save/clear Commands
- Native UI 加载/自动保存/恢复/状态显示
- Confirm After Edit 成功后按 content+revision 清理
- WDIO SIGKILL 后新进程恢复并 Commit 通过

仍未完成：

- 两窗口分别编辑仍共享同一 localStorage client id（需要窗口级稳定 id）
- Gate 变化 UI 冲突提示
- 保存失败注入 E2E
- 编辑过程中 Cancel 专项 E2E
- 正式章节被其他窗口更新的专项 E2E

## 设计约束（后续实现）

不得写入：

- `chapter_local_draft`
- `chapter_revision`

应新增 additive 表：

- novel_id
- chapter_id
- run_id
- gate_id
- base_candidate_id
- base_content_sha256
- client_id
- local_revision
- content
- updated_at

规则：

- 事务成功后才显示安全保存
- 旧 local_revision 写入拒绝
- 确认后只清除匹配 base candidate/hash 的 journal
- Reject/Regenerate/Cancel 不删除已持久化作者编辑
- Gate 变化标记冲突，不自动覆盖

## Native SIGKILL 证据

```text
test:candidate-journal-crash  SIGKILL 预期非零
test:candidate-journal-restart 1 passing
```

- 作者编辑候选后等待 `director-candidate-draft-status` 显示“已本地保存”
- SIGKILL App
- 新进程自动发现 active Run，打开 Chapter Approval
- 恢复候选编辑内容
- Confirm After Edit
- Commit
- 正式正文等于作者编辑
- `chapter_approval` artifact 为 `user_edited`

## Gate

**PARTIAL**：核心跨进程恢复已验证；两窗口/Gate 变化/失败注入未完成。


## P11.2-W1 窗口隔离与发现

### Writer Identity

- 旧实现：localStorage 共享 writer id，多窗口可能共用同一草稿记录。
- 新实现：`sessionStorage` 窗口级 writer id。
  - 同一窗口刷新保持。
  - 新 App 进程生成新 writer id。
  - 不依赖 writer id 作为授权凭据。

### 恢复规则

- 先按 run/gate/当前 writer 读取自己的草稿。
- 未找到时，按 `novel_id + run_id` 列出所有可恢复草稿。
- 当前 Gate/Candidate/Hash 匹配的草稿可由用户选择恢复。
- 恢复后作为当前窗口的新草稿记录保存，不修改其他 writer 记录。
- Gate/Candidate 不匹配的旧草稿保留，不允许 Confirm，不做自动覆盖。
- 多个草稿全部展示，不按 updatedAt 自动选择，不自动合并。
- Confirm 成功后只清理当前 writer/gate/content/revision 匹配记录。
- Reject/Regenerate/Cancel 不清理其他窗口草稿。

### Discovery API

`list_candidate_edit_drafts(novel_id, run_id)`：

- 查询受 novel + run 双重范围限制。
- 返回 writer、gate、candidate/hash、revision、updatedAt。
- 不返回跨小说草稿。

### SQLite 验证

`cargo test -p novel-storage-sqlite --test chapter_draft`

新增覆盖：

1. Window A/B 独立保存。
2. 各自 revision 独立（A 更新不影响 B）。
3. 旧 revision A 写入被拒绝。
4. A 清理后 B 草稿仍在。
5. Gate 变化后旧草稿仍可列出。
6. 旧共享 localStorage 身份记录仍可发现。
7. Discovery 按 novel/run 隔离。

### Native E2E（W1）

```text
test:candidate-journal-crash    SIGKILL 预期非零
test:candidate-journal-restart  1 passing
```

新进程使用不同 writer identity，仍能发现旧草稿；用户选择恢复后 Confirm Edit 并 Commit。

### 未完成

- 嵌入式 WebDriver 当前只提供单窗口控制，未建立真实双 Window 同时操作 E2E。
- 双窗口数据合同由 SQLite 集成测试验证；Native 双窗口标记 PARTIAL。
- Gate 变化 UI 冲突 banner 仅通过草稿列表文案/禁用确认表达，未做独立视觉验收。
