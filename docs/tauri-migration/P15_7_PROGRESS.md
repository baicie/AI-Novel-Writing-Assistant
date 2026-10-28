# P15.7 进度（原生可靠性 → 可交付 Beta）

## 基线

- 分支：`migration/tauri-2-rust`
- 起始 HEAD：`aa314218`
- 领先 origin：99 个提交
- 受保护文件：`crates/novel-http/tests/integration_runs.rs`（未改动、未暂存）

## 切片 1：质量上限停点必须可继续（本轮已交付）

### 审查结论（代码事实）

- `crates/novel-application/src/worker.rs`：审校发现问题且修复预算为 0、或有界修复后仍存在问题，都会 `pause_safety`，不提交正文，质量提醒保持 `open`。
- `crates/novel-storage-sqlite/src/workflow.rs`：`resume` 对 `PauseKind::Safety` 直接拒绝。
- `crates/novel-application/src/director.rs`：`RunDetailView.legal_actions` 对 `Paused + Safety` 返回空数组。
- `client/src/features/directorNext/TauriDirectorPage.tsx`：所有操作按钮都由 `legalActions` 驱动。

结论：正文质量问题停下后，作者既不能恢复、也不能取消，连续创作永久卡在该章节。属于 P1 产品缺陷（本地质量问题阻断整条创作链，且没有合法操作入口）。

另一处产品缺口：`client/src/features/directorNext/BatchWorkspace.tsx` 把 `repairBudget` 固定为 0，界面上无法启用工作流已具备的有界修复。

### 改动

- 新增端口 `WorkflowRepository::resume_accepting_quality_debt`：仅当暂停是质量问题停点（`Safety` 种类 + `Safety` 信号）、审校产物确有 `issues > 0`、且该 Run 的质量提醒全部被作者处理后，才允许继续；同一事务内记录“作者跳过自动修复”的 `repair_notes`（仅当从未发生修复）并应用 `Resume`。
- `DirectorService::resume_accepting_quality_debt`：命令行要求显式理由，幂等哈希包含理由。
- `legal_actions`：`Paused + Safety` 始终提供 `cancel`；当质量提醒全部处理且确有审校问题时追加 `resume_quality_debt`。普通 `resume` 对安全停点仍然拒绝。
- `RunDetailView` 增加 `pause_signal`，界面据此区分质量问题停点与数据完整性停点。
- Tauri 命令 `resume_run_with_quality_debt` + capability `allow-resume-run-with-quality-debt`。
- 界面：质量提醒未处理时给出下一步提示；处理后出现“保留这些问题并继续”。连续创作新增“发现问题时”选择，默认自动修复一次。
- 测试：`crates/novel-application/tests/p9_repair_flow.rs` 新增 2 个用例——质量停点必须逐条确认后才能继续并最终提交；修复预算为 0 时记录 `skipped_by_author` 后进入确认点，而不是卡死。

### 测试

```text
cargo test -p novel-application --test p9_repair_flow   → 6 passed, 0 failed
cargo fmt --check                                        → PASS
pnpm --filter @ai-novel/client typecheck                 → PASS
pnpm --filter @ai-novel/client test                      → 186 passed
```

### 未完成

- 原生 E2E：质量停点逐条确认 → 继续 → 确认正文章节 → 批次推进（需要重新构建应用后运行）。
- 修复预算为 1 时的原生“自动修复一次”往返仍未在真机验收。

## 后续优先级

1. 原生 E2E：`batch-repair`（审校发现问题 → 有界修复 → 逐条确认 → 继续 → 提交 → 下一章）。
2. Native Index 两进程重启恢复重跑。
3. 备份恢复 WAL 崩溃矩阵。
4. 单一 Fixture 作者旅程。
5. 1000 章桌面性能（冷启动 / RSS / CPU / 导出）。
6. 历史 Schema 异常升级矩阵；Native Save 人工 Gate（`MANUAL_NOT_RUN`）。
