# Beta-A2 M1 复测执行手册

## 当前状态

| 项 | 值 |
| --- | --- |
| 复测对象 | `beta` HEAD（Beta-A2-R2 合并后） |
| Planner 契约 | `story-macro-v2` + `chapter-list-v3`（成对固定，执行器内置） |
| M1 状态 | `RETEST_PENDING`（Beta-A1 HEAD 上的 `M1_PASS` 不延续，A2 改了 Planner 契约与 Run 冻结） |
| 执行器 | `src-tauri/src/bin/m1_run.rs`（`cargo run -p novel-desktop --bin m1_run`） |
| 离线演练 | 已通过，见第 2 节（8 次 mock 调用，`waiting_approval`，无网络、无费用） |

本手册只解决“怎样安全地跑一次真实 M1”。判定标准仍以
[P17.4.10-M1-RETEST-PACKAGE.md](../design/P17.4.10-M1-RETEST-PACKAGE.md) 第 4 节为准，
发布裁决仍以 [BETA-A2-R2-RELEASE-BLOCKERS.md](./BETA-A2-R2-RELEASE-BLOCKERS.md) 为准。

## 1. 授权清单（缺一项不得运行）

真实模式会调用付费模型。以下各项必须由作者确认后才继续：

| 项 | 说明 |
| --- | --- |
| 新 API Key | 已轮换、未出现在聊天/日志/代码中的 Key |
| 模型名称 | 复测固定使用同一个模型，便于与 Beta-A1 对比 |
| endpoint | OpenAI 兼容地址；本机地址允许无 Key 的规则不适用于本复测 |
| 供应商侧硬额度 | 必须先在供应商后台设置；本地只记录 `M1_MAX_COST_CNY`，无法强制 |
| 调用次数上限 | `M1_MAX_CALLS`，建议 10（正常链路 6–8 次，含一次结构化修复的余量） |
| 是否执行失败路径探针 | 额外 3 次调用（review → repair → re-review），可选 |

执行器在真实模式下强制要求 `M1_COST_GUARD=provider_side`；未设置时直接 `M1_REFUSED`，
请求不会发出。

## 2. 离线演练（无费用，可随时重复）

用途：确认执行器、新 Planner 契约、义务链路与产物导出在当前 HEAD 上仍然正常。
**dry-run 不是 M1 证据，不能替代真实样本。**

终端 A 启动离线 mock：

```bash
node scripts/acceptance/m1-mock-provider.mjs 18766
```

终端 B 构建并演练：

```bash
cargo build -p novel-desktop --bin m1_run
```

```bash
rm -rf /tmp/beta-a2-m1/dryrun && mkdir -p /tmp/beta-a2-m1/dryrun && BIZ_NOVEL_DATA_DIR=/tmp/beta-a2-m1/dryrun M1_AUTHORIZED=1 M1_DRY_RUN=1 M1_ENDPOINT=http://127.0.0.1:18766/v1 M1_MAX_CALLS=8 M1_MAX_COST_CNY=5 ./target/debug/m1_run
```

通过标准（本仓库最近一次演练结果）：

```text
M1_PROMPTS chapter_draft=chapter-draft-v3 chapter_list=chapter-list-v3 chapter_repair=chapter-repair-v2 story_macro=story-macro-v2
M1_FINAL_GATE run_id=...
M1_RESULT status=waiting_approval pause=None/None approvals=2 calls=8 requests_started=8 unknown_usage=0
M1_NOTE=候选已生成，停在最终确认门；未自动提升为正式正文。
```

同时检查产物 `m1-<runId>.json`：

- `chapter_list` 第 1 章的三条义务带 `sourceEventId`（`ch1-event-1`、`ch1-event-2`），
  一条 `advisory` 不带；
- `context.obligationIds` 与章节计划一致，`chapterPlanVersion` 等于本章计划版本；
- `consistency_review` v1 引用 `ch1-req-1` 且 `obligationEnforcement=hard`，v2 为 clean；
- `qualityDebt` 中该问题状态为 `resolved`。

## 3. 配置真实凭据

Key 只进入应用本地加密凭据存储，不接受命令行参数与环境变量，也不会被打印。

**方式 A（与应用一致，推荐）**：用独立数据目录启动桌面应用，在设置里保存 endpoint、
模型与 Key，然后关闭应用。

```bash
BIZ_NOVEL_DATA_DIR=/tmp/beta-a2-m1/real "/path/to/Biz Novel Studio.app/Contents/MacOS/novel-desktop"
```

**方式 B（终端录入）**：从 stdin 读取，不进入 shell 历史。

```bash
mkdir -p /tmp/beta-a2-m1/real && read -rs M1_KEY && printf '%s' "$M1_KEY" | BIZ_NOVEL_DATA_DIR=/tmp/beta-a2-m1/real M1_SETUP=1 M1_ENDPOINT=<endpoint> M1_MODEL=<model> ./target/debug/m1_run && unset M1_KEY
```

## 4. 预检（不发送模型请求）

```bash
BIZ_NOVEL_DATA_DIR=/tmp/beta-a2-m1/real M1_AUTHORIZED=1 M1_MAX_CALLS=10 M1_MAX_COST_CNY=<上限> M1_COST_GUARD=provider_side M1_PREFLIGHT=1 ./target/debug/m1_run
```

预期输出 `M1_PREFLIGHT_OK model=... endpoint=... key_present=true`。
出现 `M1_REFUSED` 时按提示修复配置，不要绕过。

## 5. 真实 M1 主样本（产生费用）

```bash
BIZ_NOVEL_DATA_DIR=/tmp/beta-a2-m1/real M1_AUTHORIZED=1 M1_MAX_CALLS=10 M1_MAX_COST_CNY=<上限> M1_COST_GUARD=provider_side M1_MODEL=<model> ./target/debug/m1_run
```

观察点：

- `M1_STARTED` 打印 `dry_run=false`；
- `M1_TICK` 依次经过规划、正文、审校；
- `M1_RESULT` 为 `status=waiting_approval`，`pause=None/None`；
- `requests_started` 不超过 `M1_MAX_CALLS`；`unknown_usage` 大于 0 时说明有调用没能拿到
  usage，按“可能已计费”记录。

执行器停在最终确认门，不会把候选提升为正式正文。

## 6. 失败路径探针（可选，3 次调用）

用主样本的 `run_id` 与 `novelId`（都在 `m1-<runId>.json` 里）验证 Reviewer 能识别
“事后到达”并触发一次有界修复：

```bash
BIZ_NOVEL_DATA_DIR=/tmp/beta-a2-m1/real M1_AUTHORIZED=1 M1_PROBE=1 M1_RUN_ID=<runId> M1_NOVEL_ID=<novelId> M1_MAX_CALLS=3 M1_MAX_COST_CNY=<上限> M1_COST_GUARD=provider_side ./target/debug/m1_run
```

预期：`M1_PROBE_REVIEW reviewer_cites_req1=true` → `M1_PROBE_REPAIR nonempty=true` →
`M1_PROBE_REREVIEW issues=0` → `M1_READY_FOR_AUTHOR_REVIEW`。

## 7. 产物与记录

- 产物：`<数据目录>/m1-<runId>.json`，保留 Run 状态、全部产物、ContextTrace、usage、
  pauseSignal 与最终候选；`M1_ARTIFACTS` 行给出路径。
- 不要把 API Key、完整凭据或未脱敏的个人作品写入仓库或聊天。
- 复测完成后，在 `docs/acceptance/` 新增结果记录，并同步更新
  `BETA-A2-R2-RELEASE-BLOCKERS.md` 的 `M1` 一项；不要把结果只留在终端输出里。

## 8. 判定标准

确定性检查（必须全部满足）：

- 第 1 章计划含 `obligations`，至少一条 `hard`，ID 唯一；
- `sourceEventId` 全部指向本章 `keyEvents`，description/participants/type 未被改写；
- Context 的 `obligationIds` 与本章计划一致，无未来章节义务；
- Reviewer 的 hard 违反项带合法 `obligationId`，evidence 能在当前候选正文中逐字定位；
- 修复后复验保留原义务 ID，`issues=0` 或残留项属于 advisory；
- `M1_RESULT` 不是 `paused`/`failed`，`pause=None/None`。

人工/授权评审（见复测包 4.2）：林动是否在灭口发生时在场、老铁匠是否当场被杀、
目击是否成为因果起点、事件顺序是否与计划一致。

## 9. 停止条件

- `M1_BUDGET_STOP`：预算用尽，立即停止，不追加调用。
- `paused / data_integrity`：记录 `pauseReason` 与 `structured_failure` 原始输出，
  不自动重试，下一次重试需要新的授权。
- 出现异常计费、Key 泄露迹象或供应商侧额度告警：立即停止并轮换 Key。
- 本轮不执行 M2–M10，不做人工 GUI 验收，不把 `beta` 提升到 `main`。
