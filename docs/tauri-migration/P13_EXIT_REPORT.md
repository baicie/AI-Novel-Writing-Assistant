# P13 Exit Report

日期：2026-10-24  
HEAD：`8b79a518` + 本文档提交（C4 验收后）  
结论：**P13 主阶段关闭，整体 PARTIAL；不得宣称完全 VERIFIED。**

## Gate 状态

| Gate | 状态 | 证据/缺口 |
| --- | --- | --- |
| G1 正式事实可安全管理 | **PARTIAL** | 人物状态/时间线/伏笔可写、按章节 As-Of 读取、章节修改 superseded；仍缺身份/关系/地点/能力物品结构化事实 |
| G2 AI 候选来源/确认/版本 | **IMPLEMENTED / PARTIAL** | 候选 evidence/revision/hash/CAS/确认/拒绝/修改；事实类型仅 3 类，未接真实模型 |
| G3 冲突可操作裁决 | **PARTIAL** | 候选不一致 + 过期来源检测；5 种裁决、CAS、幂等、历史表、Native 冲突 WDIO；无地点/死亡/时间线循环检测，supersede 语义未覆盖旧事实链 |
| G4 内容变化后旧索引失效 | **VERIFIED** | 章节保存同步索引；building 不可查；旧 revision 删除 FTS/vector/chunk；失败重试测试 |
| G5 索引持久化重试/重启恢复 | **PARTIAL** | 持久化 job、lease、attempt、退避、恢复命令、Native 队列；章节保存未自动 enqueue、旧 job revision 强校验/启动循环未完整 |
| G6 FTS/Vector/Hybrid 冻结基准 | **PARTIAL** | 50 问题、FTS/Vector/Hybrid/Policy 对照；Embedding 为 deterministic mock-64，无真实语义结论 |
| G7 Native 核心回归 | **PARTIAL** | Director 4、memory/extraction、knowledge CRUD、conflict WDIO；Index Retry/失败恢复 WDIO 未完成 |
| G8 Editor/Journal/Repair 数据安全无倒退 | **PARTIAL** | 既有 SIGKILL/Recovery 证据沿用最近同路径构建；本轮未重跑全部 Native SIGKILL，代码未改相关路径 |

**P13 主阶段关闭，G3/G5/G6/G7/G8 作为 P14 Backlog。**

## C1 冲突合同

- `memory_conflict` + `memory_conflict_decision` additive migration；
- 检测：同实体/同章节/同 factType 多候选值；候选来源 revision 过期；
- 裁决：confirm_new / keep_existing / mark_disputed / supersede_with_evidence / reject_extraction；
- commandId 幂等、expectedVersion CAS、operator、reason、历史保留；
- 原始候选不物理删除；
- 修复 plot_thread 章节修订未 supersede 的旧缺陷。

## C2 Index Job

- `knowledge_index_job` additive migration；
- commandId 幂等、sourceRevision/indexVersion、pending/running/retry_wait/succeeded/failed、attempt/maxAttempts/nextRunAt、lease、lastError；
- 手动重试与过期 running 恢复；
- Native 队列与重试按钮。

## C3 检索对照

| 策略 | Recall@10 | MRR | nDCG@10 |
| --- | ---: | ---: | ---: |
| FTS | 1.00 | 0.86 | 0.86 |
| Vector mock-64 | 0.70 | 0.48 | 0.50 |
| Hybrid RRF | 1.00 | 0.83 | 0.85 |
| Hybrid + Policy | 1.00 | 0.83 | 0.85 |

结论：保留 FTS 默认；Hybrid 在 mock 条件下未改善排序；真实 Embedding 前不切换默认。

## C4 验证

```text
cargo test --workspace --exclude novel-http
268 passed, 0 failed, 1248.39s

clean worktree cargo test --workspace --all-targets
327 passed, 0 failed

pnpm test
186 passed, 0 failed

pnpm typecheck / build
PASS

cargo fmt --check
PASS

cargo clippy --workspace --exclude novel-http --all-targets
PASS（既有 warning）

Tauri Commands / Handlers / Capabilities
89 / 89 / 89

Production .app
21.35 MiB

DMG
9.5 MiB，hdiutil verify VALID

Tauri release audit
PASS（无 Node/Electron/sidecar/WDIO marker）
```

当前工作树直接 `--all-targets` 会编译用户保留的未跟踪 `crates/novel-http/tests/integration_runs.rs` 并失败（旧测试辅助 API）；按约束未修改。干净 worktree 实际执行 327 passed。

## 未完成的人工/真实模型验收

- 真实 Provider 抽取/生成质量；
- 真实 Embedding 语义检索；
- 系统输入法与真实双窗口；
- 索引任务 SIGKILL/重启 Native；
- 完整冲突地点/死亡/时间线检测；
- 旧 Electron 图像/短剧/漫画/移动端/云端协作；
- Windows/Linux 打包。

## P14 Product Readiness Backlog

1. 结构化长期记忆（身份别名、关系、地点、死亡、能力物品）。
2. 冲突检测扩展到地点/死亡/时间线/伏笔重复回收，并完成 supersede 链。
3. 章节保存自动 enqueue 索引任务 + revision 强校验 + 启动恢复循环。
4. 真实本地 Embedding 与冻结对照，决定默认检索策略。
5. 真实模型记忆抽取/生成质量与人工评分。
6. Native 双窗口、系统输入法、完整索引重试 WDIO。
