# Beta-A1 创作一致性验收与真实作者工作台验收

## 结论摘要

| 裁决项 | 结果 | 证据 |
| --- | --- | --- |
| `ENGINEERING_READY` | **YES** | beta 基线 CI 全部通过；章节义务契约、恢复与取消测试通过；本轮仅新增验收探针测试 |
| `M1_ACCEPTANCE` | **M1_RETEST_PENDING** | 未获得新凭据、模型选择、费用上限与明确授权；未调用任何真实模型 |
| `GUI_AUTOMATED_ACCEPTANCE` | **PASS** | 11/11 桌面 E2E spec 通过；创建自然导航 + 重启 2/2 通过 |
| `MANUAL_GUI_ACCEPTANCE` | **MANUAL_GUI_NOT_RUN** | 没有真实作者执行第 7 节流程 |
| `PUBLIC_BETA_READINESS` | **NOT_READY** | M1 未通过、人工 GUI 未执行，不能用 Mock/E2E 代替 |

## 1. 基线

| 项 | 值 |
| --- | --- |
| `beta` HEAD | `90925ddd93ca041dae44778feceebf49eafad278` |
| `main` | `eff69f2`，未修改 |
| PR #23 | 已合并；合并提交 `90925ddd` |
| 验收分支 | `acceptance/beta-a1`（从 `origin/beta` 创建） |
| 桌面构建树 | 与 `6ddf96cc` 完全一致（`git diff 6ddf96cc 90925ddd` 为空），E2E 应用由此树重新构建 |
| 数据目录 | 每个 E2E spec 独立 `mktemp -d`，不接触正式作品 |
| 真实模型 | 未调用 |

## 2. M1 调用授权状态

| 门禁 | 状态 |
| --- | --- |
| 已暴露 Key 已撤销 | 本轮未使用任何旧 Key；撤销状态由凭据持有人确认，验收程序无法自行证明 |
| 新 Key 通过安全存储配置 | 验收环境未配置新 Key |
| 模型名称已明确 | 未提供 |
| 用户明确授权本次调用 | 未提供 |
| 最大费用 / 调用次数 / 失败重试策略 | 未提供 |

结论：授权门禁未满足，只执行离线验证；M1 保持 `RETEST_PENDING`。未在聊天、日志、代码或截图中写入任何 Key。

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
| 模型 | 待用户确认，未预设 |

必须生成的 hard 义务（允许等价措辞）：

| ID | 类型 | 内容 |
| --- | --- | --- |
| `ch1-req-1` | required_event | 林动亲眼撞见老铁匠被杀 |
| `ch1-req-2` | required_event | 林动在事件发生时在场 |
| `ch1-causal-1` | causal_constraint | 目击灭口是林动被卷入旧案的直接原因 |
| `ch1-order-1` | ordering_constraint | 先发现雪夜异响，再撞见灭口 |
| `ch1-forbid-1` | forbidden_event | 不得写成“到达时老铁匠已经死亡”或等价的事后到场 |

`advisory` 只能用于氛围、文风与场景细节，不能把关键事件降级。

附加约束：第 1 章不得提前揭示后续旧案真相；未来章节才揭示的事实不得进入当前章节的
Planner/Writer 上下文。M1 评审按 `docs/design/P17.4.10-M1-RETEST-PACKAGE.md` 的语义检查表
逐项判断，不把“关键词出现”当作通过。

## 5. A3 / A4 M1 执行情况

**未执行真实模型调用。** 授权门禁未满足（见第 2 节），因此没有 Planner/Writer/Reviewer/Repair
的模型原始输出、真实正文、token/费用或作者修改成本。任何 Mock 或确定性测试都不能据此判
`M1_PASS`。

M1 的完成条件（复测时逐项记录）：

1. 已暴露 Key 撤销，新 Key 只通过应用安全存储配置。
2. 明确模型名称、费用上限、调用次数与失败重试策略。
3. 用户明确授权。
4. 保留 Planner 结构化输出、义务、Writer 正文、Reviewer 结果、Repair 前后对比、ContextTrace。
5. 按 `docs/design/P17.4.10-M1-RETEST-PACKAGE.md` 的 4.1/4.2/4.3 判定。

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

未发现可复现的核心流程阻断。11/11 GUI spec 与 2/2 重启验收通过；M1 未执行属于验收门禁
未满足，不是产品缺陷。

### P2

1. Planner 上下文仍有字符级截断，超长 `story_macro` 可能被截断。
2. 义务在 Writer 上下文来源、Writer Prompt、Repair Prompt 中重复，增加 token 消耗。
3. Reviewer 的语义判断仍依赖模型；代码只保证 ID 与证据可定位。
4. `worker.rs` 仍超过项目 1,300 行硬阈值。
5. 服务端 `chapterAcceptance` 义务模型与 Rust 领域模型尚未建立字段映射。
6. 严格“每章至少一条 hard 义务”可能对过渡章节偏严，需要用真实 M1 数据验证误报率。
7. `p17-4-6-gui-acceptance` 等 spec 每轮输出窗口标题警告；测试通过，但属于 E2E 工具噪声，
   不影响产品判断。

### P3

- 未执行人工验收，视觉与文案细节的 P3 清单待人工补充；本轮不凭自动化结果臆测。

### 工程回归（beta 等价树）

- `cargo fmt --all -- --check`：PASS。
- `cargo clippy -p novel-application --all-targets -- -D warnings`：PASS。
- 本轮新增验收测试：15 passed（含 Beta-A1 探针）。
- 取消/恢复/修复套件：3 + 6 + 9 passed。
- GUI smoke：11/11 spec 通过；创建导航 + 重启 2/2 通过。
- 完整 workspace、client 与 CI 结果沿用同一树的最近一次通过记录（`6ddf96cc`，与 beta 树一致）：
  workspace 602 passed、test-harness 114 passed、client 290 passed、GitHub Actions 全部通过；
  beta 合并提交 `90925ddd` 的 Tauri CI 也通过。
  本轮未重复执行这些大套件，原因是代码树相同且验收分支只新增了一个测试。

## 9. 下一阶段建议

1. 先完成 M1 前置条件（新 Key、安全存储、模型、预算、授权），按固定样本执行一次 M1。
2. 由真实作者按第 7 节流程做一次人工验收；未执行前保持 `MANUAL_GUI_NOT_RUN`。
3. 根据 M1 与人工结果再决定是否进入 M2–M10；不自动开始 P17.5。
4. 若 GUI smoke 暴露 P0/P1，先修可复现问题；P2/P3 进入 Backlog。

## 10. 最终裁决

- `ENGINEERING_READY`：**YES**
- `M1_ACCEPTANCE`：**M1_RETEST_PENDING**
- `GUI_AUTOMATED_ACCEPTANCE`：**PASS**（11/11 spec + 2/2 重启验收）
- `MANUAL_GUI_ACCEPTANCE`：**MANUAL_GUI_NOT_RUN**
- `PUBLIC_BETA_READINESS`：**NOT_READY**（M1 与人工 GUI 未完成）
