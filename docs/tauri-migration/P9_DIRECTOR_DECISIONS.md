# P9.3 Director Decisions

## 1. Backend

新增：

- `reject_run` / `DirectorService::reject`
- `regenerate_run` / `DirectorService::regenerate`
- `confirm_after_edit_run` / `DirectorService::confirm_after_edit`

共同保证：

- commandId 幂等（`workflow_command`）
- expectedVersion 可选 CAS
- Gate 目标校验
- Scope：run 的 novel/chapter 由 run contract 固定
- 拒绝/重生成把当前 gate artifact 标记 `stale` 并 `ResolveGate`
- 历史 artifact 不删除
- Confirm After Edit 插入 `user_edited` + protected artifact，并保留编辑后 JSON
- 失败返回 Conflict/NotFound/Integrity，不伪装成功

## 2. Legal Actions

`RunDetailView.legal_actions` 由 Rust 投影：

- `waiting_approval` + gate：`confirm`, `reject`, `regenerate`, `confirm_after_edit`
- 可恢复暂停：`resume`, `cancel`
- Safety 暂停：无 resume
- running/queued：`cancel`
- 终态：空

Native Director 按钮使用该投影，而不是 React 自行猜测状态。

## 3. UI

已接入：

- Confirm（沿用 approve）
- Reject
- Regenerate
- 质量债列表（Native Director）
- 操作失败错误提示
- 操作后刷新

未接入：

- Confirm After Edit 的编辑 UI（后端命令存在，尚无 artifact JSON 编辑器）
- 新旧候选对比
- 过期 Gate/双窗口/断线/审批取消竞争 E2E

## 4. 测试

`crates/novel-application/tests/p9_decisions.rs`：

- 第一 Gate 的 legalActions 包含四个决策
- Reject 后旧 artifact 保留为 stale，新生成 artifact 出现
- Confirm After Edit 生成 user_edited/protected artifact
- 之后 run 回到 running，不把任何决策映射成 approve 成功

未测试：

- Regenerate 的具体 UI 按钮 E2E
- 过期 Gate、双窗口、Channel 断线
- 审批与取消竞争
