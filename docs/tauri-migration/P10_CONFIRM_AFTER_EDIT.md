# P10.3 Confirm After Edit

## 1. Backend 合同

新增命令：

`confirm_candidate_after_edit_run`

输入：

- `runId`
- `expectedVersion`（control CAS）
- `baseCandidateId`
- `baseContentSha256`
- `content`

Rust/SQLite 校验：

- Run 必须处于 waiting_approval
- 当前 Gate 必须包含 `chapter_approval`
- 最新 candidate 的 id + content_sha256 必须与 base 完全匹配
- 编辑正文非空
- 新建 `author_edit` candidate，不覆盖原 candidate
- 插入 `chapter_approval` / `user_edited` / `protected_user_content=1` artifact
- `ResolveGate` 后才能进入 Commit
- 不做 official chapter_revision 写入
- 不写章节 Local Draft

冲突时不改任何正文，返回 Conflict。

## 2. Native UI

- Chapter Approval Gate 显示“编辑候选正文”
- 原始候选可查看
- textarea 编辑
- 简单行级差异（删除线/新增）
- “恢复原候选”
- “确认修改后候选”
- 编辑草稿存 `sessionStorage`，仅用于未确认候选，不写正式章节 Local Draft
- 成功后刷新 Run，进入 Commit
- 失败显示 Conflict 文案，旧 Gate 版本不能提交

## 3. 保护

- Artifact `user_edited` + protected
- 原始 candidate 保留
- 作者编辑 candidate 保留
- Commit 仍走正式章节保护/CAS
- Cancel Run 不删除已保存作者编辑候选（由 candidate/artifact 持久化）

## 4. E2E

WDIO：

```text
test:director
4 passing
- 页面/启动
- 完整 Director 流程
- Reject / Regenerate
- 编辑候选 → Confirm After Edit → Commit
```

重启：

```text
test:confirm-edit-restart
1 passing
- 新 App 进程正文、Revision、user_edited artifact、最新候选一致
```

Rust：

```text
cargo test -p novel-application --test p10_confirm_edit
1 passed
- 错误 base hash 被拒绝
- 确认前 official 正文为空
- Commit 后正文等于作者编辑
- 原/编辑 candidate 均保留
```

## 5. 未完成

- 真实双窗口同时编辑
- 编辑中 Cancel
- 保存候选失败注入
- 正式章节被另一窗口更新
