# 桌面端恢复契约：备份恢复、索引接管与质量债停点

## 背景

桌面端（Tauri 2 + 本地 SQLite）必须保证三件事：
用户的书不会因为进程被杀而丢；索引任务不会被两个进程重复执行；质量问题不能绕过数据完整性停点。

这三条都不是"界面提示"，而是数据层契约。下面记录当前规则与崩溃窗口，改代码前先读这一页。

## 一、备份恢复协议

一次恢复由三步组成，顺序不能变：

1. `request_restore` 把请求写成 `restore-intent.json`（源备份、staged 路径、原库路径、时间）；
2. 启动阶段 `recover_pending_restore` 在**任何运行时打开数据库之前**执行；
3. 切换：校验 staged → 把活动库重命名为 `pre-restore-*` 保护副本（连同 `-wal/-shm` 一起搬走）→ staged 改名为主库。

崩溃窗口与当前行为：

| 窗口 | 磁盘上的真实状态 | 行为 |
| --- | --- | --- |
| 意图提交前 | 只有 `restore-intent.tmp` | 不切换，主库不动 |
| staged 丢失 | intent 指向不存在的文件 | 标记 `restore-intent.failed.json`，主库不动 |
| staged 损坏 | 校验失败 | 拒绝，保留 intent，不移动主库 |
| 切换中 | 原库已移走、staged 未改名 | 下次启动回滚原库 |
| 重复请求 | 多份 intent | 以最后一份为准 |
| 备份写到一半 | 只有 `.<name>.tmp`，无正式文件 | 半成品永不暴露给用户，主库不受影响 |

关键点：

- 备份走"临时文件 + 改名"，因此中途被杀不会留下看起来可用的半个备份；`list_backups` 也只列 `.sqlite`。
- 保护副本必须包含未 checkpoint 的已提交事务。用文件拷贝代替 SQLite 备份接口会丢掉 `-wal` 里的内容，
  这是测试和工具脚本里最容易犯的错误。
- 切换失败（例如数据目录只读）必须显式报错，并保留原库、staged 与 intent，方便修复后重试。

## 二、索引任务的租约与接管

- 任务在 `knowledge_index_job` 里，状态 `pending/retry_wait/running`，带 `lease_owner`、`lease_until`、`claim_epoch`。
- 只有 `pending/retry_wait` 能被领取；过期的 `running` 由 `recover_expired_index_jobs` 转回 `retry_wait`。
- 完成时按 owner + epoch 做 CAS：旧进程的迟到写入会被拒绝，因此"两个进程都以为自己在写"不会污染数据。
- 进程被杀后重新接管会拿到更高的 epoch；检索必须只返回最新修订，旧修订内容不能通过 FTS 召回。

## 三、质量债停点的两条恢复入口

- `pause_kind = Safety` 有两种来源：质量问题（`stop_signal = Safety`）与数据完整性（`stop_signal = DataIntegrity`）。
- `resume_accepting_quality_debt` 只允许前者：必须同时满足 PauseKind::Safety、StopSignal::Safety、
  最新审校产物 issues > 0、且该 run 没有未决质量债。
- 数据完整性停点永久关闭这条入口；作者只能取消或走普通恢复路径。这是安全边界，不要放宽。

## 四、验证方式

- 单元层：`cargo test -p novel-desktop --lib`（备份恢复窗口）、
  `cargo test -p novel-application --test p9_repair_flow`（质量债停点）。
- 进程层：`scripts/run-tauri-backup-crash-e2e.sh`（备份写到一半被杀）、
  `scripts/run-tauri-backup-restore-e2e.sh`（请求恢复后被杀）、
  `scripts/run-tauri-index-crash-e2e.sh`（领养后被杀、新进程接管）。
- 端到端：`scripts/run-tauri-author-journey-e2e.sh` 在同一份 SQLite、同一个 Novel ID 上跑七个阶段。

## 五、失败模式与诊断

- 用例报 `no such table: schema_migration` → 多半是自己造了一条不完整的 staged 库（拷贝主库文件而漏了 `-wal`）。
- 原生用例报 `Unable to connect to 127.0.0.1:4445` → 应用在驱动就绪与建立会话之间退出了，先确认二进制是 e2e 构建，
  再看是否有上一轮的实例占着端口。
- 恢复后 FTS 命中旧内容 → 检查 `complete_index_job` 的 owner+epoch 是否被绕过。

## 相关模块

- `src-tauri/src/commands/backup.rs`（恢复协议与窗口）
- `src-tauri/src/commands/knowledge.rs`、`src-tauri/src/lib.rs`（租约、接管、回收）
- `crates/novel-storage-sqlite/src/workflow.rs`（质量债停点与恢复）
- `crates/novel-application/src/director.rs`（可执行动作与投影）
