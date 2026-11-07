# Beta-A1 创作一致性验收与真实作者工作台验收

## 结论摘要

| 裁决项 | 结果 | 证据 |
| --- | --- | --- |
| `ENGINEERING_READY` | **YES** | PR #24 已合并到 beta（`1d21ba6b`）；CI 全部通过；应用在 Planner 失败时正确 fail-closed |
| `M1_ACCEPTANCE` | **M1_FAIL** | beta `1d21ba6b` 最终 M1 在 `story_macro` 处失败：非法 JSON，结构化修复仍缺 `protagonist_core`；无候选、无正式正文 |
| `GUI_AUTOMATED_ACCEPTANCE` | **PASS** | 11/11 桌面 E2E spec 通过；创建自然导航 + 重启 2/2 通过 |
| `MANUAL_GUI_ACCEPTANCE` | **MANUAL_GUI_NOT_RUN** | 没有真实作者执行第 7 节流程 |
| `PUBLIC_BETA_READINESS` | **NOT_READY** | M1_FAIL（Planner 结构化输出失败）、人工 GUI 未执行，不能用 Mock/E2E 代替 |

## 1. 基线

| 项 | 值 |
| --- | --- |
| `beta` 验收基线 | `90925ddd93ca041dae44778feceebf49eafad278` |
| `beta` 当前 HEAD | `1d21ba6be10a64983889e23acb8a663ba971c64b`（PR #24 合并后） |
| `main` | `eff69f2`，未修改 |
| PR #23 | 已合并；合并提交 `90925ddd` |
| PR #24 | 已合并；合并提交 `1d21ba6b`（修复截断 P1） |
| PR #25 | Draft（结构化失败诊断；未合并） |
| 验收分支 | `acceptance/beta-a1`（从 `origin/beta` 创建） |
| 桌面构建树 | 与 `6ddf96cc` 完全一致（`git diff 6ddf96cc 90925ddd` 为空），E2E 应用由此树重新构建 |
| 数据目录 | 每个 E2E spec 独立 `mktemp -d`，不接触正式作品 |
| 真实模型 | 已执行 1 次 M1 主样本（`deepseek-v4.1-flash`，6 次调用） |

## 2. M1 调用授权状态

| 门禁 | 状态 |
| --- | --- |
| 已暴露 Key 已撤销 | 用户确认已轮换；本轮未使用任何旧 Key |
| 新 Key 通过安全存储配置 | 是：`/tmp/beta-a1-m1/real/secrets/openai-compatible.secret`，权限 0600 |
| 模型名称已明确 | `deepseek-v4.1-flash` |
| 用户明确授权本次调用 | 是 |
| 最大费用 / 调用次数 / 失败重试策略 | 供应商侧额度上限；本地 10 次调用硬停止；0 自动重试 |

结论：门禁满足，M1 主样本已执行。Key 只写入应用本地加密凭据存储，未出现在聊天、日志、
代码或截图中。

## 3. A1 实际运行链路离线验证

### 3.1 运行时 Prompt 版本探针

在 beta 验收分支运行真实 Worker（ScriptedLlm，无网络、无付费），从本轮实际发送的 Prompt
文本与产物读取版本证据，而不是检查源码默认值。新增测试：

`crates/novel-application/tests/p17_4_10_obligations.rs::beta_a1_runtime_prompt_versions_are_observable`

运行命令：

```bash
cargo test -p novel-application --test p17_4_10_obligations beta_a1_runtime_prompt_versions_are_observable -- --nocapture
```

运行输出（`--nocapture`）：

```text
BETA_A1_PROMPT_PROBE chapter_list=chapter-list-v2 chapter_draft=chapter-draft-v3 consistency_review=consistency-review-v2 chapter_repair=chapter-repair-v2
BETA_A1_RUN status=succeeded signal=None
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

版本判定依据是各版本独有的运行时模板文本，而非源码默认值：

| 步骤 | 运行时证据 |
| --- | --- |
| Planner | Prompt 含 chapter-list-v2 独有要求“obligations 是本章必须遵守的结构化义务数组”“每章至少一条 hard 义务”，且含 story_macro 内容 |
| Writer | Prompt 含 chapter-draft-v3 模板槽位“章节义务：\n【硬约束｜必须遵守”，且不含 v2 渲染后追加的“如果正文与硬约束冲突，必须调整正文” |
| Reviewer | Prompt 含 consistency-review-v2 独有措辞“没有采用 advisory 建议不构成问题”及 obligationId 要求 |
| Repair | Prompt 含 chapter-repair-v2 的“本章章节义务”“硬约束必须保留”“创作建议可以合理取舍” |

### 3.2 义务与产物

同一次运行生成的 `context` 产物被断言包含：

- `runId` 等于本次 Run。
- `chapterPlanVersion = 1`。
- `obligationIds` 含 hard 义务 `ch1-req-1`、`ch1-order-1` 与 advisory 义务 `ch1-adv-1`。
- `obligationsText` 同时含 `【硬约束｜必须遵守】` 与 `【创作建议｜可合理取舍】` 两个分区。
- `trace.chapterPlanVersion = 1`，`trace.sources` 含 `chapter_obligations` 来源、`selected=true`
  与 `contentHash`。

Reviewer 产物被断言包含：

- hard 违反项的 `obligationEnforcement = "hard"`。
- issue 顶层 `issues = 1`；advisory 只计入 `advisoryIssueCount`，不进入修复计数。
- `obligationId`、`evidence`、`needsAuthorConfirmation` 保留。

以上证明章节义务契约在运行时链路中真实生效；**不等于真实模型会遵守义务**。模型是否按
hard 义务写作、Reviewer 是否能正确判断参与者/因果变化，仍由 M1 的语义检查回答。

### 3.3 取消、失败与恢复

在 beta 树上运行：

| 套件 | 结果 |
| --- | --- |
| `p17_4_10_obligations`（含 Beta-A1 探针） | 15 passed |
| `p11_repair_faults` | 3 passed |
| `recovery` | 6 passed |
| `p9_repair_flow` | 9 passed |

覆盖：修复期间取消 → cancelled；修复失败/Provider 故障 → 可恢复暂停；未知义务 ID 与伪造证据
→ DataIntegrity 暂停；租约/并发恢复；审校后残留问题的安全停点。

## 4. A2 固定 M1 样本

与历史 M1 失败样本保持一致，便于比较：

```text
书名：P17.4.7 模型评测样本
简介：边陲小镇的少年在雪夜发现一枚会发光的旧玉佩，由此被卷入山门与一桩旧案。
预计章节数：30

第1章：林动在雪夜发现玉佩，随后亲眼撞见老铁匠被灭口。
```

固定参数：

| 项 | 值 |
| --- | --- |
| Planner | `chapter-list-v2` |
| Writer | `chapter-draft-v3` |
| Reviewer | `consistency-review-v2` |
| Repair | `chapter-repair-v2` |
| 上下文预算 | 24k |
| 输出预算 | ≥8192 |
| max automatic retries | 0 |
| repair budget | 1 |
| 模型 | `deepseek-v4.1-flash`（实际执行） |

本轮 Planner 实际生成的 hard 义务（允许等价措辞）：

| ID | 类型 | 内容 |
| --- | --- | --- |
| `ch1-req-1` | required_event | 林动先拾得发光玉佩，随后在铁匠铺外亲眼撞见老铁匠被灰斗篷人灭口；不得改为事后到达 |
| `ch1-req-2` | required_event | 灰斗篷人察觉林动动静，林动携玉佩逃亡 |
| `ch1-par-1` | participant | 本章出场剧情参与者只能是林动、老铁匠、灰斗篷人；沈砚不得登场 |
| `ch1-cau-1` | causal_constraint | 拾玉 + 目击灭口 → 同时成为持玉人与活口，直接卷入旧案 |
| `ch1-ord-1` | ordering_constraint | 拾玉 → 回铺外 → 目睹灭口 → 被发现 → 逃亡 |
| `ch1-for-1` | forbidden_event | 不得提前揭示旧案全貌、玉佩真正来历或灰斗篷人受谁指使 |
| `ch1-con-1` | continuity_constraint | 林动保持边陲少年、铁匠铺帮工、受老铁匠收留的身份起点 |

`advisory` 只能用于氛围、文风与场景细节，不能把关键事件降级。

附加约束：第 1 章不得提前揭示后续旧案真相；未来章节才揭示的事实不得进入当前章节的
Planner/Writer 上下文。M1 评审按 `docs/design/P17.4.10-M1-RETEST-PACKAGE.md` 的语义检查表
逐项判断，不把“关键词出现”当作通过。

## 5. A3 / A4 M1 执行情况

**已执行一次真实 M1 主样本**（run `8ee2c031-1cfa-4001-8dcd-272a543950d8`，2026-11-07）。
执行门禁：

1. 已暴露 Key 未使用；用户确认已轮换。
2. 新 Key 通过隔离数据目录的应用加密凭据存储配置。
3. 模型 `deepseek-v4.1-flash`；供应商额度上限；本地 10 次调用硬停止；0 自动重试。
4. 用户明确授权本次调用。
5. 产物保留在 `/tmp/beta-a1-m1/real/m1-8ee2c031-....json`，未写入仓库。

### 5.1 执行器与准备状态（2026-11-07）

已准备一个本地 M1 执行器 `src-tauri/src/bin/m1_run.rs`，它使用真实 `Runtime` 与
`ProtocolRouterProvider`，密钥只从应用本地加密凭据存储读取，**不接受命令行或环境变量里的
Key，也不打印 Key**。执行器要求 `M1_AUTHORIZED=1`、`M1_MAX_CALLS`、`M1_MAX_COST_CNY` 才会
启动；固定使用 24k 上下文、8192 输出、`maxAutomaticRetries=0`、`repairBudget=1`。

本地 dry-run（本地 OpenAI 兼容 mock，无网络、无付费、假 Key）结果：

```text
M1_RESULT status=waiting_approval approvals=2 calls=8
artifacts: context chapterPlanVersion=1 obligationIds=[ch1-req-1,ch1-order-1,ch1-forbid-1,ch1-adv-1]
           review hard obligationEnforcement=hard; repair_notes present; final review clean
key leak: false
```

dry-run 只验证执行器、Run 链路与导出，不是 M1 证据。

准备步骤：

1. 用独立数据目录启动 release 桌面应用：
   `BIZ_NOVEL_DATA_DIR=/tmp/beta-a1-m1/real "/tmp/p17410/target/aarch64-apple-darwin/release/bundle/macos/Biz Novel Studio.app/Contents/MacOS/novel-desktop"`
2. 在应用设置中保存新 Key、endpoint 与模型；**不要把 Key 发到聊天、日志或代码里**。
3. 关闭应用后，执行器读取同一数据目录的加密凭据与 `provider_credential` 配置。
4. 授权后运行：

```bash
cd /tmp/beta-a1
BIZ_NOVEL_DATA_DIR=/tmp/beta-a1-m1/real \
M1_AUTHORIZED=1 M1_MAX_CALLS=10 M1_MAX_COST_CNY=<用户确认的上限> \
M1_MODEL=<应用配置的模型> \
cargo run -q -p novel-desktop --bin m1_run
```

产物写入 `/tmp/beta-a1-m1/real/m1-<runId>.json`，含 Run 状态、context、chapter_list、
正文候选、审校、修复、ContextTrace 与 usage。执行器停在最终确认门，不自动提交正文。

### 5.2 真实 M1 主样本结果

| 项 | 值 |
| --- | --- |
| 模型 | `deepseek-v4.1-flash` |
| Run 状态 | `waiting_approval`（最终确认门，未自动提交正文） |
| 调用次数 | 6（`story_macro`、`character_cast`、`volume_strategy`、`chapter_list`、`draft`、`consistency_review`） |
| token | prompt 18,707 / completion 50,768 |
| 规划门 | 2 次自动确认（人物阵容、卷策略），属于验收执行器动作 |
| 正文候选 | 2,537 字，仅 `draft` 候选；未触发 Repair |
| Reviewer | 1 次，`issues=0`、`advisoryIssueCount=0`、quality debt 0 |
| ContextTrace | `chapterPlanVersion=1`，7 条 hard 义务 ID，`chapter_obligations` 来源含 content hash |
| 费用 | 按供应商账单计；本地记录了 token，未把费用写入仓库 |

#### 逐条义务核验（基于真实正文）

| 义务 | 结果 | 证据摘要 |
| --- | --- | --- |
| `ch1-req-1` 拾玉后亲眼撞见灭口 | PASS | 先写雪地拾得发光玉佩，随后隔院墙豁口看见灰斗篷人动手；老铁匠倒地，林动“连呼吸都忘了” |
| `ch1-req-2` 凶手察觉动静、林动携玉逃亡 | PASS | 林动踩碎冻雪发出轻响，凶手转头指向豁口；林动转身逃入巷子，玉佩在怀中发光 |
| `ch1-par-1` 仅三名剧情参与者 | PASS（有观察项） | 沈砚未登场；正文提及“李屠户”为背景交代，结尾无名住户应门，均未参与剧情。Reviewer 未标记 |
| `ch1-cau-1` 拾玉 + 目击导致卷入旧案 | PASS | 持玉人与灭口活口双重身份成立，凶手追杀直接由目击触发 |
| `ch1-ord-1` 顺序 | PASS | 拾玉 → 回铺外 → 目睹灭口 → 被发现 → 逃亡 |
| `ch1-for-1` 不提前揭示真相 | PASS | 未揭示主谋、玉佩真正来历；只埋“凶手从遗体翻走一物”的线索 |
| `ch1-con-1` 身份起点 | PASS | 边陲少年、铁匠铺帮工、被老铁匠收留均保留 |

#### Reviewer / Repair 与质量判断

- 本次正文没有违反核心 hard 义务，Reviewer 返回 clean 是合理的；但 **Reviewer 识别违反的
  失败路径没有被触发**，原 M1 的“事后到达”改写在本次真实输出中没有复现。
- **Repair 没有被触发**，因此“Repair 是否真正修复冲突、是否引入新矛盾”本轮没有证据。
- 阅读质量（授权代理评估）：场景完整、视角稳定、有雪夜氛围与追逐钩子，基本可读；
  这是代理评估，不是作者人工评分。作者修改成本未记录。
- 观察项：正文出现“李屠户”与结尾无名住户，严格按 `ch1-par-1` 的“只能是三名参与者”
  存在边界模糊；Reviewer 未报告。它不构成 high 级事件/因果矛盾，但说明参与者约束的
  语义判定仍依赖 Reviewer。

### 5.3 失败路径探针与 16k 修复复测（同一冻结上下文）

用主 Run 的 context 与一份“事后到达”的违规候选执行两次 3 次调用探针。

**第一次（8192 预算，beta 行为）**：

| 步骤 | 结果 |
| --- | --- |
| Reviewer 第一次 | 4 条 issue，包含 `ch1-req-1`，evidence 逐字引用违规候选；**Reviewer PASS** |
| Repair | 返回 641 字，句子在“他喊了声”处中断；completion tokens 正好 8192；**输出被截断** |
| Reviewer 复验 | 4 条 issue，残留 `ch1-req-1`；**Repair FAIL** |

**第二次（用户单独授权，修复预算 16,384）**：

| 步骤 | 结果 |
| --- | --- |
| Reviewer 第一次 | 6 条 issue，包含 `ch1-req-1`；**PASS** |
| Repair | 返回 1,512 字完整正文；completion 8,273 < 16,384；包含林动隔门缝目睹灭口、被发现、携玉逃亡；**PASS** |
| Reviewer 复验 | `issues=0`，`residual_req1=false`；**PASS** |

复测 usage：review1 4,671 prompt / 8,825 completion；repair 5,946 / 8,273；
review2 5,688 / 1,124。主样本 6 次 + 两次探针各 3 次，合计 12 次调用、
50,506 prompt / 89,192 completion tokens；第二次探针由用户单独授权，费用仍受供应商额度限制。

第一次探针的根因是确定性契约缺陷：旧实现把达到 8192 输出上限的截断修复文本保存为候选，
复验自然继续报 `ch1-req-1`。16k 复测证明提高修复输出预算后 Reviewer/Repair 能收敛。

#### 阶段性判定（已被 5.4 最终 M1 覆盖）

当时为 **`M1_RETEST_PENDING`**：

- beta `90925ddd` 上的 M1 为 **FAIL**：8192 修复截断导致失败路径不收敛。
- 16k 修复复测已经收敛；PR #24 已合并到 beta（`1d21ba6b`），CI 全部通过。
- 后续在 beta 上执行最终 M1，结果见 5.4（Planner 结构化输出失败，M1_FAIL）。

PR #24 变更：修复输出预算至少 16,384 token；completion 达到预算时不再保存候选，
记录 usage 并进入 DataIntegrity 暂停；新增 repair budget / 截断判定单元测试。

### 5.4 beta 最终 M1（`1d21ba6b`，含 #24）

| 项 | 值 |
| --- | --- |
| Run | `4d4bd5f1-75ba-4690-95ee-4c60d599353d` |
| 模型 | `deepseek-v4.1-flash` |
| 结果 | `paused` / `data_integrity` |
| 失败位置 | Planner（`story_macro`） |
| 暂停原因 | `模型输出不是合法 JSON；修复重试仍失败：结构化产物缺少非空字段 protagonist_core` |
| 产物 | 仅 `novel_seed`；无 story_macro/cast/volume/chapter_list/context/正文/审校 |
| usage | 记录 1 次调用，1,960 prompt / 11,029 completion tokens；暂停原因显示结构化修复也执行过，调用数可能少计一次 |
| 质量债务 | 0 |

判定：**`M1_FAIL`**（Planner 结构化输出失败，未进入章节义务链路）。这次失败不是 #24 引入的
回归——#24 只改 bounded repair；同一模型在 90925ddd 的主样本曾通过 story_macro。应用行为
正确：一次结构化修复后仍不合格即 `DataIntegrity` 暂停，没有候选、没有正式正文，也没有
把失败当成零问题。

没有重试本次 M1：任务禁止无根据的连续付费调用。若继续，需要先决定是换模型，还是先补
Planner 输出诊断（保留失败原始输出/提高结构化规划预算），再获得新的明确授权。

## 6. B1 桌面 GUI 自动化 smoke

### 6.1 构建与运行

- 应用：`pnpm build:tauri:e2e` 从 beta 等价树重新构建。
- 运行方式：`scripts/run-p172-e2e-isolated.sh`，每个 spec 独立数据目录，真实 Tauri/SQLite/UI。
- 运行命令：

```bash
bash scripts/run-p172-e2e-isolated.sh \
  e2e/p17-4-6-gui-acceptance.e2e.js e2e/author-journey.e2e.js \
  e2e/p17-workbench.e2e.js e2e/p17-cancel-navigation.e2e.js \
  e2e/p17-3-context.e2e.js e2e/p17-3-continuity.e2e.js \
  e2e/p17-4-plot-timeline.e2e.js e2e/p17-4-7-chapter-creation.e2e.js \
  e2e/p17-4-7-agent-d-fix.e2e.js e2e/p17-selection-rewrite.e2e.js \
  e2e/p17-edit-position.e2e.js
```

- 未使用 Playwright/WebDriver 冒充人工验收。

### 6.2 结果

| Spec | 覆盖的 B1 场景 | 结果 |
| --- | --- | --- |
| `p17-4-6-gui-acceptance` | 新建小说、章节正文保存、创建伏笔 | PASS |
| `author-journey` | 模型配置、Director 开始、确认前不落正式正文 | PASS |
| `p17-workbench` | 章节导航、工作区页签、小窗口 | PASS |
| `p17-cancel-navigation` | 未保存守卫、取消切章 | PASS |
| `p17-3-context` | 计划/上下文来源、历史 Run 冻结、暂停时可恢复动作 | PASS |
| `p17-3-continuity` | 人物状态、冲突、证据失效与恢复 | PASS |
| `p17-4-plot-timeline` | 伏笔创建、推进、回收、因果与证据 | PASS |
| `p17-4-7-chapter-creation` | 章节创建、排序、失败重试 | PASS（3/3） |
| `p17-4-7-agent-d-fix` | 人物设定与本书世界（不依赖模型） | PASS |
| `p17-selection-rewrite` | 编辑、保存、重新打开、模型请求 | PASS |
| `p17-edit-position` | 光标位置恢复、重启入口 | PASS（3/3） |
| `create-restart` 脚本 | 手动创建小说 → 关闭程序 → 重新打开专业工作台 | PASS（2/2 spec） |

汇总：`11/11 spec 全部通过`；`create-navigation` + `create-restart` 2/2 通过。
`p17-4-6-gui-acceptance` 生成 5 张截图（`/tmp/p1746-gui-*.png`），作为 GUI 操作证据；
截图不是人工验收。

自动化覆盖缺口（不能由 E2E 结果替代）：

- Director 的显式暂停/恢复按钮路径未单独覆盖；`p17-3-context` 只验证暂停状态暴露了
  `resume`/`approve` 合法动作。
- 30 章以上长篇的连续手工管理、未保存守卫的主观提示质量、作者是否理解 AI 理由，仍需人工。

未覆盖或只由 Rust 测试覆盖的项：Director 显式暂停/恢复的 GUI 按钮路径、30 章以上长篇手工管理。
这些列入第 7 节人工流程与第 9 节建议。

## 7. B2 人工验收流程（≤30 分钟）

状态：`MANUAL_GUI_NOT_RUN`。不把自动化结果当作人工回答。

执行者按顺序完成并记录：

| 分钟 | 操作 | 观察 |
| --- | --- | --- |
| 0–5 | 新建专业小说，填写书名与简介 | 是否知道下一步；默认值是否清楚 |
| 5–10 | 创建 2 个章节，编辑正文，切换章节不保存 | 未保存守卫是否出现；取消/确认是否符合预期 |
| 10–13 | 创建人物并绑定本书世界 | 人物信息是否容易查找 |
| 13–18 | 创建伏笔，推进一次，再回收并选择证据 | 是否理解伏笔状态与证据要求 |
| 18–23 | 启动 Director，查看计划、正文、审校；暂停/取消/恢复 | 是否理解 AI 为什么这样写；错误后能否恢复 |
| 23–27 | 关闭应用再打开，回到上次位置 | 是否保留章节、正文与进度 |
| 27–30 | 回答下列问题并打分 | 1–5 分与一句理由 |

问题：

1. 是否能理解下一步该做什么？
2. 是否能够查看并修改章节计划？
3. 是否能理解 AI 为什么这么写？
4. AI 违反计划时是否能发现并修正？
5. 长篇章节管理是否方便？
6. 伏笔和人物信息是否容易查找？
7. 发生错误后是否能恢复创作？
8. 是否愿意继续使用这套工作台？

## 8. P0 / P1 / P2 / P3 与回归

### P0

未发现。自动化 GUI smoke 中未出现数据损坏、作品丢失或应用无法启动。

### P1

无开放 P1。修复输出截断 P1 已由 PR #24 修复并合并到 beta（`1d21ba6b`）：修复输出预算至少
16,384 token，达到上限时 fail-closed；16k 复测收敛，CI 全部通过。最终 M1 在 Planner 处失败
属于模型结构化输出稳定性问题（见 P2），不是已证实的确定性代码 P1。

### P2

1. Planner 上下文仍有字符级截断，超长 `story_macro` 可能被截断。
2. 义务在 Writer 上下文来源、Writer Prompt、Repair Prompt 中重复，增加 token 消耗。
3. Reviewer 的语义判断仍依赖模型；代码只保证 ID 与证据可定位。本次正文出现“李屠户”与
   结尾无名住户，严格按 `ch1-par-1` 存在参与者边界模糊，Reviewer 未标记；不是 high 级
   矛盾，但说明该约束的语义判定仍不稳定。
4. `worker.rs` 仍超过项目 1,300 行硬阈值。
5. 服务端 `chapterAcceptance` 义务模型与 Rust 领域模型尚未建立字段映射。
6. 严格“每章至少一条 hard 义务”可能对过渡章节偏严；本次真实样本生成 7 条 hard 义务并
   通过，未出现误报，但仍需更多样本。
7. **Planner 结构化输出稳定性**：最终 M1 中 `deepseek-v4.1-flash` 首次输出非法 JSON，
   结构化修复后仍缺 `protagonist_core`，应用 fail-closed。同一模型在 90925ddd 上曾通过，
   目前无法区分是模型随机性、网关截断还是输出预算不足；本次失败的原始输出已无法回溯。
   Draft PR #25 已增加 `structured_failure` 诊断产物（脱敏、限长），下一次失败会保留首次
   与修复原始输出、错误、token 与模型。
8. **结构化修复的 usage 可能少计一次调用**：暂停原因显示执行了结构化修复，但 run usage
   只记录 1 次调用；供应商可能按 2 次计费。属于用量透明度 P2。
9. `p17-4-6-gui-acceptance` 等 spec 每轮输出窗口标题警告；测试通过，但属于 E2E 工具噪声，
   不影响产品判断。

### P3

- 未执行人工验收，视觉与文案细节的 P3 清单待人工补充；本轮不凭自动化结果臆测。

### 工程回归（beta 等价树）

- `cargo fmt --all -- --check`：PASS。
- `cargo clippy -p novel-application --all-targets -- -D warnings`：PASS。
- 本轮新增验收测试：15 passed（含 Beta-A1 探针）。
- 取消/恢复/修复套件：3 + 6 + 9 passed。
- P1 修复分支 #24：repair budget/截断判定单元测试 2 passed；3 + 14 + 9 + 6 套件通过；
  clippy `-D warnings` PASS；GitHub Actions（Rust/前端/Linux Bundle/macOS Bundle/Kernel/发布脚本）
  全部通过。
- 失败路径复测：8192 预算 FAIL（截断）；16,384 预算 PASS（Reviewer 6 issues 含 `ch1-req-1`，
  Repair 1,512 字完整，复验 `issues=0`）。
- PR #24 合并到 beta：`1d21ba6b`；beta 树与修复分支 `8eae5d40` 一致。
- beta 最终 M1：`story_macro` 结构化输出失败（非法 JSON；修复后缺 `protagonist_core`），
  应用 DataIntegrity 暂停，无候选/正文；模型输出质量失败，不是应用崩溃。
- GUI smoke：11/11 spec 通过；创建导航 + 重启 2/2 通过。
- 完整 workspace、client 与 CI 结果沿用同一树的最近一次通过记录（`6ddf96cc`，与 beta 树一致）：
  workspace 602 passed、test-harness 114 passed、client 290 passed、GitHub Actions 全部通过；
  beta 合并提交 `90925ddd` 的 Tauri CI 也通过。
  本轮未重复执行这些大套件，原因是代码树相同且验收分支只新增了一个测试。

## 9. 下一阶段建议

1. 评审 Draft PR #25（`fix/beta-a1-planner-diagnostics`）；它把结构化失败的首次/修复原始
   输出保留为 `structured_failure` 产物，不改变暂停语义。合并后下一次 M1 失败可直接诊断，
   不需要靠再付费调用猜原因。
2. 在明确授权下重试一次最终 M1（同一模型或换更稳定的结构化输出模型）；重试前记录模型与
   费用上限。若再次失败，直接读取 PR #25 的诊断产物判断截断/别名/随机性，再决定是否提高
   结构化规划输出预算；不要在无证据时反复调 Prompt。
3. 由真实作者按第 7 节流程做一次人工 GUI 验收；未执行前保持 `MANUAL_GUI_NOT_RUN`。
4. M1 通过前不进入 M2–M10，不自动开始 P17.5；P2/P3 进入 Backlog。

## 10. 最终裁决

- `ENGINEERING_READY`：**YES**（#24 已合并，CI 通过；应用在 Planner 失败时正确暂停）
- `M1_ACCEPTANCE`：**M1_FAIL**（beta `1d21ba6b` 最终 M1 在 `story_macro` 结构化输出处失败）
- `GUI_AUTOMATED_ACCEPTANCE`：**PASS**（11/11 spec + 2/2 重启验收）
- `MANUAL_GUI_ACCEPTANCE`：**MANUAL_GUI_NOT_RUN**
- `PUBLIC_BETA_READINESS`：**NOT_READY**（M1 FAIL 与人工 GUI 未完成）
