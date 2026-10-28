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
## G2 — WAL 备份恢复崩溃窗口（本轮已交付，PARTIAL）

### 交付

`src-tauri/src/commands/backup.rs` 的单元测试矩阵（`cargo test -p novel-desktop --lib`，9 passed）新增 7 条：

| 用例 | 覆盖窗口与断言 |
| --- | --- |
| `no_intent_leaves_the_live_database_untouched` | 意图提交前（只剩 `restore-intent.tmp`）：不切换，正文不变 |
| `missing_staged_database_keeps_the_live_database` | stage 丢失：marker 转为 `.failed.json`，主库不动 |
| `corrupt_staged_database_is_refused` | stage 损坏：拒绝并保留 intent，主库既不移动也不被覆盖 |
| `switch_moves_wal_sidecars_and_keeps_both_databases_readable` | 切换窗口：旧 `-wal/-shm` 随原库搬走，恢复后的库只含备份内容，保护副本能独立打开 |
| `a_newer_intent_replaces_the_pending_source` | 重复 Restore Intent：以最后一份为准，旧 staged 文件不处理 |
| `safety_snapshot_includes_uncheckpointed_wal_rows` | 未 checkpoint 的已提交事务：`-wal` 非空时保护副本仍能读到该数据 |
| 既有两条 | 正常切换保留保护副本；原库已移动、staged 未切换时回滚原库 |

### 说明

崩溃窗口用"进程被杀后磁盘上留下的真实状态"复现，再由真实的 `recover_pending_restore` 做判定；
进程级 SIGKILL 目前只有 P15.3 的 Native 用例覆盖了"请求恢复后杀进程"这一处。

### 未覆盖

备份写入中 SIGKILL、VACUUM 中途失败、磁盘空间不足、只读目录、备份与恢复竞争、两个 App 实例、
"备份来自更新版本"在启动期的拒绝路径（`request_restore` 侧已拒绝）。

## G3 补充 — 100 / 500 / 1000 三档采样

- 三档都完成了真实采样：章节列表 P95 56 / 242 / 140 ms，打开章节 P95 46 / 13 / 21 ms，
  FTS P95 1343 / 220 / 207 ms，导出 P50 97 / 42 / 48 ms，备份 P50 501 / 253 / 294 ms，
  SQLite 5.4 / 13.5 / 23.3 MiB。
- 冷启动 228–913 ms，其中数据库迁移与运行时就绪只有 4–14 ms；RSS 54–92 MB，空闲 CPU ≈0。
- 100 章档的导出 / 备份 / FTS P95 尖峰与规模不成比例，判定为首次调用冷态；冷热分离统计留到下一轮。
- 详细表格与未覆盖项见 `docs/tauri-migration/P15_8_PERFORMANCE.md`。
## G4 — 共享 Fixture 作者旅程（进行中：准备与连续创作已通过）

### 已交付

- `scripts/run-tauri-author-journey-e2e.sh`：同一数据目录、同一 Novel ID 的多阶段旅程，
  阶段名可用 `BIZ_NOVEL_JOURNEY_PHASES` 选择，`BIZ_NOVEL_JOURNEY_KEEP=1` 保留 fixture 供后续阶段。
- 阶段 1 `journey-prepare`：通过产品命令创建一部小说、一个世界观、三名人物、两卷、10 章。
- 阶段 2 `journey-batch`（审校必报问题的场景）：连续创作 10 章，逐章经历"审校发现问题 → 有界修复 →
  质量上限停点 → 逐条确认质量提醒 → 保留问题继续 → 章节确认 → 正式提交"，最后断言：
  批次 `completed`、10 项全部 `succeeded`、每章 `officialVersion >= 1` 且正文非空、每个 run 恰好 1 份
  `chapter_commit` 产物（共 10 份，无重复提交）。

### 实测

```text
[journey] phase: prepare
[journey] phase: batch   → 1 passing（批次完成，10 章全部正式提交）
```

同一 fixture 上重复执行阶段 2 是幂等的（脚本复用已完成批次并只做校验，1.5s 通过）。

### 过程中确认的一件事

工作流命令按 `command_id` 去重：阶段脚本最初用固定 id 反复批准，第二次起被当成重放而静默不生效。
产品界面每次操作都会生成新的 `command_id`（`prefix:时间戳:uuid`），所以这不是产品缺陷；
旅程脚本已改为按控制版本生成唯一 id。

### 未完成

结构化记忆（别名/地点/生死/关系/冲突/As-Of）、编辑与历史（草稿、正式保存、Diff、恢复、双窗口）、
知识与导出、备份恢复重启这几段尚未接入同一 fixture。
## G4 续 — 结构化记忆阶段（已通过，同一 fixture）

阶段 3 `journey-memory` 在同一个 Novel ID、同一个 SQLite 上追加：

- 别名（林公子）、生死状态（alive）、有向关系（ally_of → 苏晚）、地点（第 2 章 山门 → 第 5 章 祖祠）；
- 断言 As-Of：第 2 章仍看到山门、看不到祖祠；第 5 章之后活动地点只剩祖祠（单值事实被新版本取代）；
- 关系终止：`{relation:"ally_of", active:false}` 生效后活动关系只剩终止记录；
- 同一章出现两个不同地点 → 产生冲突 → 用 `resolve_memory_conflict`（`keep_existing`）裁决后活动地点仍是祖祠；
- 断言最终地点事实带正确的 `sourceChapterId` 与 `truthfulness=confirmed`。

```text
[journey] phase: prepare
[journey] phase: batch    → 10 章正式提交
[journey] phase: memory   → PASS
[journey] batch <id> runs 10
```

fixture：`/tmp/biz-novel-journey.OxXXEB`（`BIZ_NOVEL_JOURNEY_KEEP=1` 保留，供后续阶段复用）。
## G4 续 — 知识与导出阶段（已通过，同一 fixture）

阶段 4 `journey-knowledge`：把第 1–5 章归入第一卷、第 6–10 章归入第二卷（`update_chapter_meta`，带官方版本 CAS），
然后新增一条知识来源并触发索引，最后：

- `search_knowledge("剑谱")` 命中该知识来源；
- 按卷导出只包含第 1–5 章，不含第 6 章；
- 全书导出按顺序包含第 1–10 章，长度 > 200。

```text
[journey] phase: knowledge → 1 passing（1.4s）
```

注意：该阶段默认关闭"首次索引失败注入"（`BIZ_NOVEL_E2E_INDEX_FAIL_ONCE=0`），
失败注入行为由 `index-retry` 专项用例覆盖。
## G4 续 — 编辑、历史与恢复阶段（已通过，同一 fixture）

阶段 5 `journey-editor`：第一位作者写完一章后，

- 两位作者（`journey-writer-a` / `journey-writer-b`）各自保存本地草稿，互不覆盖；
- 作者 A 用当前官方版本保存正式稿，官方版本 +1；
- 作者 B 用过期官方版本再次保存 → 被拒绝，B 的草稿仍在；
- 历史里至少有两条修订，选择最早一条恢复（`draftDecision=keep`）→ 官方版本再 +1，正文回到最早版本；
- 恢复后两份草稿都还在。

```text
[journey] phase: editor → 1 passing（1.1s）
```

说明：本阶段用命令级入口验证两位写作者的草稿隔离与官方版本 CAS；
真实双窗口交互由 `multiwindow-draft-*` 专项用例覆盖。
## G4 收尾 — 备份恢复重启阶段（已通过，同一 fixture）

阶段 6/7 `journey-backup` → `journey-backup-verify`，七个阶段全部在同一份 SQLite、同一个 Novel ID 上跑完：

1. 备份已完成的书稿并校验（`create_backup` + `verify_backup` = ok）；
2. 改动其中一章正文，然后提交恢复请求；
3. SIGKILL 整个应用进程；
4. 新进程启动时先应用恢复，再打开数据库；
5. 校验：该章正文回到备份版本、改动消失；备份目录里同时有本次备份与 `pre-restore-*` 保护副本；
   第十章的草稿仍在、批次仍是 `completed`（10 项全部成功）、结构化记忆的活动地点仍是祖祠、
   知识检索仍命中、全书导出仍包含第 10 章。

```text
[journey] phase: prepare → batch（10 章正式提交）→ memory → knowledge → editor →
                backup（请求恢复后 SIGKILL，退出码 1 属预期）→ backup-verify
[journey] batch e51faa02… runs 10
[journey] restore rolled back chapter to version 1 backup novel-1793193820517.sqlite
[journey] PASS
```

fixture：`/tmp/biz-novel-journey.ZUATT3`。

**G4 状态：VERIFIED**（共享 Fixture 完整旅程）。唯一说明：双写作者的真实双窗口交互仍由
`multiwindow-draft-*` 专项用例覆盖，本旅程用命令级入口验证同一组数据不变量。
## G2 补充 — 再补两类崩溃/失败窗口

新增两条单元用例（`cargo test -p novel-desktop --lib` 现为 11 passed）：

- `backup_into_a_read_only_directory_fails_without_losing_the_source`：备份目标目录只读时备份必须失败，
  不留下半成品文件，源库仍可打开且内容不变。
- `restore_switch_failure_keeps_the_live_database_and_the_intent`：数据目录只读导致切换重命名失败时，
  恢复必须显式报错而不是把库切坏；原库留在原位、staged 与 intent 都保留，便于修复后重试。
## G7 收尾 — 独立 worktree 全量回归

在 `HEAD` 上创建 detach worktree（不含未跟踪的本地文件），跑 `cargo test --workspace --exclude novel-http --all-targets`：

```text
cargo_exit=0
test result: ok  ×53
```

第一次跑时暴露了一个真实问题：`a_newer_intent_replaces_the_pending_source` 用 `std::fs::copy` 复制主库文件生成 staged 库，
而主库当时还有未 checkpoint 的 `-wal`，拷贝出来的库可能连 `schema_migration` 表都没有，用例随机失败。
已改为用 SQLite 备份接口生成 staged 库（提交 `26ce5478`），之后在独立 worktree 上稳定通过。EOF
tail -3 docs/tauri-migration/P15_8_PROGRESS.md
