# P14 M1/M2 Final Evidence

## M1 Native Crash/Restart

Runner: `scripts/run-tauri-crash-e2e.sh`  
Evidence: `tests/desktop-e2e/artifacts/20261025-112829/`

| Scenario | Data dir | PID | Crash exit | Restart |
|---|---|---:|---:|---|
| Editor local draft | `/tmp/biz-novel-crash-editor.498aDe` | 20805 | 1（预期 SIGKILL 断连） | PASS |
| Candidate journal | `/tmp/biz-novel-crash-candidate.qTNii0` | 20967 | 1（预期 SIGKILL 断连） | PASS |
| Normal restart | `/tmp/biz-novel-restart.*` | — | — | PASS |
| Confirm After Edit restart | 同上 | — | — | PASS |

State JSON 已复制到 evidence：

- `editor-state.json`（PID 20805）
- `candidate-state.json`（PID 20967）

Crash 非零退出只有在以下同时满足时才接受：

- state JSON 存在且非空；
- 记录的 App PID 已不再存在；
- Restart 使用同一 data dir 并通过断言。

`pgrep` 使用 E2E 专用二进制绝对路径，不透杀其他进程。

结果：

```text
editor-crash + editor-crash-restart        PASS
candidate-journal-crash + restart          PASS
director keep + restart                    PASS
confirm-edit-restart                       PASS
```

## M2 Index Worker

新增独立 `index_worker` test binary：

- 打开同一 SQLite；
- claim Job（epoch/lease）；
- 写 PID marker；
- 真实 SIGKILL。

测试 `index_worker_crash`：

```text
killed_index_worker_is_recovered_by_fencing_and_reindexed ... ok
```

验证：

- 进程被 SIGKILL；
- lease 过期后 recover；
- 新 owner claim，epoch 递增；
- 重新索引成功；
- Job succeeded；
- 只有 1 个 chunk，source_revision 保持 rev-1。

Tauri 自动 Worker：

- 每 2s claim 一个 due pending/retry_wait Job；
- 独立 owner `index-worker-{pid}-{seq}`；
- execute 校验 source revision，完成/失败/obsolete。
- 启动 30s 扫描过期 lease。

仍未完成：

- 失败章节索引的 pending Job 自动创建（章节保存仍同步索引，记录 succeeded job）；
- Native Index Retry/Recover WDIO；
- 多进程并发索引（当前进程内写锁）。

## M3 复审

- G1：事务回滚 trigger、CAS、并发、supersede 测试通过；
- G2：跨小说/章节守卫、revision rank、进程内写锁 + ready CAS、并发 v2/v3 测试通过；
- 未新增多进程索引测试；
- 未新增冲突重启历史测试。

状态：

| Goal | Status |
| --- | --- |
| M1 | VERIFIED |
| M2 | IMPLEMENTED / PARTIAL |
| M3 | PARTIAL |
