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

## G5 — Quality Debt 安全边界（本轮已交付）

### 负向用例（`crates/novel-application/tests/p9_repair_flow.rs`）

- `data_integrity_hold_is_never_resolved_through_quality_debt`：修复返回空正文导致的数据完整性停点，
  即使作者把全部质量提醒都确认完，也不下发 `resume_quality_debt`，两条恢复入口都返回错误。
- `cancelled_quality_stop_cannot_be_resumed`：先取消再恢复被拒绝，运行保持 `cancelled`。
- `quality_debt_resume_is_idempotent_and_closed_afterwards`：同一命令号重复提交只推进一次控制版本并返回
  `replayed`；换新命令号再恢复被拒绝；质量提醒用过期 `decision_version` 重复确认会被拒绝。

### 界面

- “保留这些问题并继续”改为两步：先展开本次要保留的质量问题清单（类型、严重度、描述）与后果说明，
  点“确认保留并继续”才会发起恢复；可“先不继续”。

### 结果

```text
cargo test -p novel-application --test p9_repair_flow   9 passed / 0 failed
```
## G3 — 1000 章桌面性能（本轮已交付，单档）

- 真实数据：1000 章 / 478,984 字 / 1,436,952 字节正文 / SQLite 22.9 MiB / 200 篇知识文档。
- 关键数字（P50 / P95）：章节列表 11 / 56 ms，打开章节 1 / 2 ms，FTS 11 / 61 ms，全书导出 14 / 15 ms，
  备份 89 ms；冷启动 736 / 1436 / 353 ms（初始化 8–29 ms），RSS 54.8–58.4 MB。
- 详细方法与未覆盖项见 `docs/tauri-migration/P15_8_PERFORMANCE.md`。
- 复现：`bash scripts/run-tauri-perf-1000-e2e.sh`（`BIZ_NOVEL_PERF_KEEP=1` 保留报告）。
## G7/G8 — 回归、生产审计与门禁状态

### 回归（最终 HEAD 上执行）

```text
cargo test -p novel-application -p novel-storage-sqlite     43 个测试二进制全 ok / 0 failed
cargo check -p novel-desktop                                 PASS
pnpm --filter @ai-novel/client typecheck                     PASS
pnpm --filter @ai-novel/client test                          186 passed / 0 failed
pnpm --filter @ai-novel/client build                         PASS
pnpm --filter @ai-novel/desktop-e2e test:batch-repair        1 passing（两步确认新界面）
pnpm --filter @ai-novel/desktop-e2e test:index-retry         1 passing
bash scripts/run-tauri-index-crash-e2e.sh                    三阶段 PASS
```

未执行：`cargo test --workspace`、clean worktree `--all-targets`（受保护文件与既有 fmt 差异原因），
以及其余 Native 用例（Batch 10 章、Backup Restore、Dual Writer、Task Center、Export、Structured Memory）。
`cargo fmt --check` 全仓仍会因 `crates/novel-http` 的既有格式差异失败，与本轮改动无关。

### 生产审计

```text
cargo tauri build --target aarch64-apple-darwin --bundles app   PASS，23.27 MiB
bash scripts/audit-tauri-release.sh                             PASS
命令数 / capability 允许项                                       111 / 111
生产二进制中的 e2e 钩子字符串                                    全部 0
（BIZ_NOVEL_E2E_INDEX_HOLD / _LEASE_MS / _RECOVER_SECS / _STARTUP_MARKER / _INDEX_FAIL_ONCE / tauri_plugin_wdio）
```

### 门禁状态

| Gate | 状态 | 依据 |
| --- | --- | --- |
| Native Index Restart | VERIFIED | 三阶段真实两进程 + epoch 递增 + FTS 只返回最新修订 |
| Quality Debt Security | VERIFIED | 三条负向用例 + 两步确认界面 |
| Desktop Performance | PARTIAL | 1000 章单档真实采样；100/500、CPU 长时间、30 分钟内存漂移未做 |
| WAL Crash Restore | NOT_STARTED | 本轮未执行 |
| Shared Author Journey | NOT_STARTED | 本轮未执行 |
| Native Save | PARTIAL | 导出链路已有自动化；系统保存对话框 MANUAL_NOT_RUN |
| Full Regression | PARTIAL | 见上：作用域内全绿，workspace/all-targets 未跑 |
| Production Audit | VERIFIED | 构建、审计、钩子隔离、111/111 |

结论：`BETA_RC_BLOCKED`（缺 WAL 崩溃矩阵、共享 Fixture 旅程、Native Save 人工验收、完整回归与 100/500 档性能）。
