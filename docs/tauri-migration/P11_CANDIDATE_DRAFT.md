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
