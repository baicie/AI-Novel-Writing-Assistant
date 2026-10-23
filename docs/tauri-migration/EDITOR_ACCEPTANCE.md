# Editor Acceptance

## 自动验收

| 项目 | 结果 |
| --- | --- |
| Rust draft storage | 2 PASS |
| Native UI autosave + refresh | PASS |
| Native UI official save + switch chapter | PASS |
| Native UI SIGKILL recovery | PASS（crash harness 主动退出 1，新进程恢复 PASS） |
| Frontend draft race tests | 6 PASS |
| Full frontend tests | 186 PASS |
| Rust workspace | 223 PASS |
| Clean worktree all-targets | PASS（HEAD `7f7b7e60`） |

## 人工验收

### 系统拼音输入法

1. 打开测试章节，切换到 macOS 拼音输入法。
2. 连续输入组合，观察候选、光标、标点。
3. 在 composition 尚未确认时按 Cmd+S。
4. 预期：不提交半成品；composition 结束后正文完整。
5. 输入后立即切换章节。
6. 预期：返回时草稿/正文完整，不静默丢失。
7. 长文滚动、撤销/重做、粘贴长文本。
8. 关闭窗口后重开。
9. 失败时保存：数据库文件、日志、操作步骤、输入法版本、截图。

### 双窗口

1. 打开同一本书同一章的两个窗口。
2. 在两个窗口输入不同正文。
3. 各自看到自己的本地草稿；不得互相覆盖。
4. 一个窗口正式保存，另一个窗口保存时必须出现版本冲突。
5. 保存失败时保留双方草稿。

### 崩溃恢复

1. 输入正文，等待“本地草稿已安全保存”。
2. 强制结束 App 进程。
3. 重新启动相同测试数据目录。
4. 打开章节，预期出现草稿恢复提示和正文。
5. 正式保存后检查 Revision 只增加一次。

## 已知未自动验收

- 中文 composition 候选交互
- 真实窗口关闭按钮
- 真实双窗口
- AI Commit 与人工保存同一秒的 UI 冲突提示
