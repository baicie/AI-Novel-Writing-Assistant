# P15.8 Beta RC 系统验证与可靠性收敛

## 基线

- 分支：`migration/tauri-2-rust`
- 起始 HEAD：`1c5bfb17`
- 领先 origin（`3bc699dd`）：104 个提交
- 受保护文件：`crates/novel-http/tests/integration_runs.rs`（未改动、未暂存）

## G1 — Native Index Worker 真实两进程重启（本轮已交付）

### 交付

- 测试专用钩子（全部位于 `#[cfg(feature = "e2e")]`，生产构建不含这些字符串）：
  - `BIZ_NOVEL_E2E_INDEX_LEASE_MS`：索引领取租约时长。
  - `BIZ_NOVEL_E2E_INDEX_RECOVER_SECS`：过期租约回收间隔。
  - `BIZ_NOVEL_E2E_INDEX_HOLD`：领取结果落库后挂住，供控制器确认状态后 SIGKILL。
  - `BIZ_NOVEL_E2E_INDEX_FAIL_ONCE=0`：显式关闭首次失败注入。
- `scripts/run-tauri-index-crash-e2e.sh`：三阶段共享同一数据目录。
  1. 正常索引 v1 并检索 `剑谱`；
  2. 领取决胜 v2 后挂住，确认 `claim_epoch = 前一次 + 1`，SIGKILL 整个 App；
  3. 新进程回收过期租约，新 epoch 完成索引，检索只剩 v2 内容（`图谱` 命中、`剑谱` 不召回）。

### 结果

```text
SKIP_E2E_BUILD=1 bash scripts/run-tauri-index-crash-e2e.sh
  phase 1 index v1 -> succeeded, FTS hit
  phase 2 claim v2 persisted (epoch = v1 + 1), SIGKILL app   [PASS]
  phase 3 new process: epoch 继续增长, job succeeded, FTS 只返回 v2, 原生索引面板显示 succeeded   [PASS]
pnpm --filter @ai-novel/desktop-e2e test:index-retry           1 passing（405ms 回归）
```

### 断言覆盖

Job ID 不变、claim epoch 单调增长（两次递增）、旧租约被回收后才重新领取、索引完成后只召回最新 Source Revision、
原生索引面板显示 succeeded。

### 仍未覆盖

- 旧 Worker 迟到写入被拒绝（`complete_index_job` 的 owner+epoch CAS）由后端用例
  `crates/novel-storage-sqlite/tests/index_worker_crash.rs` 覆盖，本轮未在原生进程内重跑。
- Source Deleted、RetryExhausted、两 Worker 竞争的原生矩阵未做。
