# P11.2 Candidate Edit Draft

## 当前状态

**NOT_STARTED / PARTIAL（仅审计）**

P10 的 Confirm After Edit 已具备：

- Candidate CAS（baseCandidateId + baseContentSha256）
- `user_edited` protected Artifact
- Native 编辑 UI
- 重启后 Author Candidate/Revision 持久化

但未确认编辑内容目前仍只存在：

- React state
- `sessionStorage`（P10 新增）

还没有：

- 独立 SQLite Candidate Edit Journal
- localRevision CAS
- 跨进程 SIGKILL 恢复未确认候选编辑
- Gate 变化时保留/冲突提示
- 两窗口候选编辑隔离
- 确认成功后按匹配版本清理 Journal

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

## 本轮结论

P11.2 Gate **未开始**，不得标记 VERIFIED。
