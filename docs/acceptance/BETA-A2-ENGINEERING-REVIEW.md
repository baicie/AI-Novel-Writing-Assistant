# Beta-A2 全仓关键路径审计、Planner 事件契约封口与 M1 验收准备

## 结论

**ENGINEERING_STATUS：READY_FOR_REVIEW（评审 P1 已修复，等待最终 CI 与复核）。**
**PLANNER_CONTRACT：PARTIAL（Planner 输入已按 Run 冻结；作者章节目标字段仍是数据模型缺口）。**
**M1_ACCEPTANCE：RETEST_PENDING（Beta-A2 HEAD 需要新的真实模型验收；Beta-A1 HEAD 已有一次授权 PASS）。**
**GUI_AUTOMATED：PASS（见第 7 节）。**
**MANUAL_GUI：MANUAL_GUI_NOT_RUN。**
**PUBLIC_BETA：NOT_READY。**

本轮修复了四个确定性契约缺陷：story-macro-v2 缺失 keyEvents 时静默通过、sourceEventId
只校验形式不校验规范内容、Planner 上下文按字符截断可能损坏 JSON/丢失关键事件，以及评审
发现的 Planner book 输入未按 Run 冻结。修复后 Run 在 `open_run` 时冻结
story_macro / character_cast / volume_strategy / chapter_list 的版本，context、Planner
Prompt、章节计划引用校验与修复审校都只读取本 Run 的冻结来源。作者“当前章节目标”没有
独立的持久化字段，只能写进小说简介；这是明确的数据模型缺口，本轮不凭空新增剧情要求，
记录为 PARTIAL 原因。

## 1. 基线与最终 HEAD

| 项 | 值 |
| --- | --- |
| `beta` 基线 | `d66d565c254c8a70f11c5b5c833f50fba5f50518` |
| `main` | `eff69f229727ab90f4102c05cfc705116cb1ded5`，未修改 |
| 工作分支 | `fix/beta-a2-planner-invariants` |
| 代码 HEAD | 见 PR 最新提交（关键事件强制、规范内容、上下文预算、模块拆分、Planner 输入按 Run 冻结） |
| 客户端/E2E HEAD | `ab22d638`（固定 v2/v3 与 mock 夹具） |
| 报告提交 | 见 PR 最新提交（仅文档） |
| PR | 见第 12 节 |

## 2. 审查过的关键模块

- `crates/novel-application/src/worker/worker_validation.rs`：结构化输出校验、章节计划引用/
  覆盖、Planner 上下文组装。
- `crates/novel-application/src/obligations.rs`：story_macro keyEvents、章节义务、
  sourceEventId、规范事件保持。
- `crates/novel-application/src/worker.rs`：Prompt 版本选择、结构化修复、覆盖校验接线、
  有界修复与暂停。
- `crates/novel-prompt/src/lib.rs`：story-macro-v2、chapter-list-v3。
- `client/src/features/directorNext/*`、`client/src/api/localCreation.ts`：新合同版本固定。
- `e2e/author-journey.e2e.js`、`e2e/p17-3-context.e2e.js`：Mock 计划补齐 keyEvents/
  sourceEventId。
- `crates/novel-application/src/context.rs`、`worker/context_budget.rs`、
  `tests/p17_4_10_obligations.rs`、`tests/p17_4_10_run_freeze.rs`：Run 冻结/版本测试。

## 3. 已确认的 P0 / P1 / P2

### P0

无。

### P1（已修复，需在合并前复核）

1. **story-macro-v2 可缺少 keyEvents**：新 Prompt 要求 keyEvents，但校验在字段缺失时返回
   成功，新计划可以没有任何规范事件。修复：合同显式固定 `story-macro-v2` 时，缺失字段、
   空数组、全部 advisory 都 fail-closed；`story-macro-v1`/旧产物继续宽松读取；仍只允许
   一次既有结构化修复，失败进入 DataIntegrity 并保留 `structured_failure`。
2. **sourceEventId 形式覆盖**：旧校验只检查 ID/类型/章节，章节义务可以写“主角到达时老铁匠
   已死”而仍引用“主角目击灭口”的事件 ID。修复：义务的 `description`、`participants`、
   `type`、`chapterOrder`、hard 级别必须与规范关键事件一致；含义相反、参与者替换、降级、
   跨章节、未知 ID 都拒绝。
3. **Planner 上下文字符截断**：`context_payload` 直接截断序列化 JSON，可能破坏 JSON 结构、
   丢失 keyEvents/constraints。修复：结构化预算，只裁剪解释性文本；优先保留
   keyEvents/constraints/人物身份/卷；必选内容放不下时明确失败，绝不返回被截断的 JSON。
4. **Planner 输入没有按 Run 冻结（本轮评审发现）**：`structured_prompt` 的 character_cast /
   volume_strategy / chapter_list 上下文，以及 `assemble_context` 的 book 来源，此前使用
   `latest_payload` / `artifact_source` 的最新版本；`RunContract` 没有记录 open 时的
   story_macro / character_cast / volume_strategy / chapter_list 版本。Run-A 在 context 之前
   暂停、作者保存新计划后恢复，就会读到作者的更新。修复：

   - `RunContract.source_versions` 在 `open_run` 时记录四个 book 来源的最新非 stale 版本；
     旧合同没有该字段，反序列化为空表，行为不变。
   - 统一选择顺序：本 Run 产出的产物 → 合同冻结版本 → 旧合同按最新非 stale；冻结版本缺失
     时 fail-closed，不退回最新版本（`context::choose_run_artifact`）。
   - `execute_context`、character_cast / volume_strategy / chapter_list 的 Planner Prompt、
     chapter_list 的引用校验、有界修复读取的审校产物都改为 Run 作用域读取。
   - `open_run` 要求 `story_macro` 与 `chapter_list` 的 Planner 契约版本成对固定
     （`story-macro-v2` + `chapter-list-v3`），避免一半新一半旧的 Prompt 契约。

   已知边界：打开 Run 时书里还没有任何计划产物时，冻结表为空，之后新增的计划仍按旧合同
   读取；这条路径在打开时没有可冻结的输入，本轮不改变（见第 6 节）。

### P1（未修复，明确缺口）

5. **作者当前章节目标没有持久化字段**：详见第 5 节。产品数据模型只有小说标题/简介与章节
   标题/正文，没有 `chapterBrief`/`chapterGoal`；作者无法把“第1章必须目击灭口”作为独立、
   可版本化的章节目标输入。M1 样本曾把目标写进小说简介（测试工具注入），这不是产品契约。

### P2

1. **结构化修复 usage 可能少计一次调用**：失败暂停时首次调用与修复调用可能合并成一条
   Invocation；M1 run `4d4bd5f1` 的暂停原因显示执行了修复，但 run usage `calls=1`。
   供应商可能按两次收费。本轮未改；建议每次 Provider 响应单独记录，未知计费保留
   `billed_unknown`。
2. Planner 结构化输出稳定性仍依赖模型；`structured_failure` 已保留原始输出，便于下一次
   失败直接诊断。
3. Reviewer 的文学语义判断仍依赖模型；确定性代码只保证引用、规范内容与覆盖。
4. `worker.rs` 仍超过 1,300 行硬阈值（已知 P2）；本轮把 Planner 上下文预算拆到
   `worker/context_budget.rs`，`worker_validation.rs` 已降到 1,046 行，Worker 主体仍需后续
   按职责拆分。

## 4. 失败先行测试与证据

### 4.1 Phase A：story-macro-v2 keyEvents

- `strict_story_macro_requires_key_events`：缺字段 → 严格失败；legacy 调用仍成功。
- `strict_story_macro_requires_a_hard_key_event`：全部 advisory → 失败。
- `empty_story_macro_key_events_are_rejected`：空数组 → 失败。
- `strict_new_story_macro_without_key_events_pauses`（集成）：两次非法输出后 DataIntegrity
  暂停，Writer 未启动。
- 旧 `story-macro-v1`/无 keyEvents 的既有测试全部通过。

### 4.2 Phase B：规范事件内容保持

- `source_event_content_cannot_be_rewritten`：
  - 正确描述/参与者/类型/章节 → PASS；
  - ID 正确但描述相反 → BLOCK（“描述被改写”）；
  - 参与者替换 → BLOCK（“参与者被替换”）；
  - hard 降级为 advisory → BLOCK（“降级”）。
- `future_chapter_source_event_is_rejected`：第 1 章引用第 2 章事件 → BLOCK。
- `chapter_plan_must_cover_hard_story_key_events`：缺失/未知 sourceEventId → BLOCK。
- `covered_story_key_events_reach_the_writer_with_source_id`（集成）：覆盖后 Writer 收到
  “来源事件”文本。

### 4.3 Phase D：Planner 上下文预算

- `planner_context_keeps_key_events_and_never_truncates_json`：超长宏观/人物/卷下仍输出
  合法 JSON，keyEvents 完整保留。
- `planner_context_fails_when_mandatory_events_exceed_budget`：必选事件超预算 → 明确失败，
  不返回截断 JSON。
- 模块拆分：`worker/context_budget.rs` 承接上下文预算与 Run 作用域产物读取；
  `worker_validation.rs` 1,046 行；拆分仅移动代码，行为由同一组测试覆盖。

### 4.4 Phase E：Planner 输入按 Run 冻结

单元测试（`context::tests`、`worker::context_budget::tests`）：

- `run_artifact_wins_over_pinned_and_latest`：本 Run 产物优先于冻结版本与最新版本。
- `pinned_book_version_ignores_newer_author_update`：冻结版本 1 不被版本 2 替换。
- `legacy_contract_without_pins_keeps_latest_non_stale`：旧合同（空冻结表）保持最新非 stale。
- `pinned_contract_missing_source_does_not_fall_back`：冻结来源缺失时不回退最新。
- `frozen_planner_payload_ignores_a_newer_author_version`：Planner Prompt 上下文使用冻结版本。
- `missing_frozen_planner_source_fails_instead_of_reading_latest`：缺冻结来源时明确失败。
- `repair_reads_its_own_run_review_not_another_runs_newer_one`：修复读取本 Run 的审校产物。
- `legacy_run_without_own_review_keeps_latest`：旧 Run 仍保持既有行为。

集成测试（新文件 `tests/p17_4_10_run_freeze.rs`）：

- `planner_book_inputs_stay_frozen_after_author_updates`：先保存 v1 计划并打开 Run，再保存
  v2 story_macro/chapter_list；Run 成功后 context 的 `chapterPlanVersion` 仍是 1，context
  与 Writer 的 Prompt 只包含 V1，不含 V2。测试同时断言 v2 产物确实已写入（版本 2），
  因此旧实现“取最新”必然读到 V2。
- `run_contract_rejects_a_half_pinned_planner_pair`：只固定 `story-macro-v2` 而不固定
  `chapter-list-v3` 时 `open_run` 拒绝。

负向对照：临时把冻结表替换为空表（等价旧实现）后，`planner_book_inputs_stay_frozen_after_author_updates`
失败于 `chapterPlanVersion = 2`；恢复实现后通过。说明该测试确实覆盖评审指出的窗口。

## 5. Phase C：作者输入来源（缺口）

当前可用输入：

- `NovelSnapshot.title` / `description`：`story_macro` Prompt 的 `story_input` 只使用这两项。
- `ChapterSnapshot.title` / `sort_order` / `content`：章节标题是作者可编辑字段，但没有进入
  Planner Prompt。
- `NovelSnapshot` 的 `first_30_chapter_promise`、`book_selling_point` 等是书级字段，不是
  当前章节目标。
- 没有 `chapterBrief` / `chapterGoal` / `chapterTarget` 字段，也没有对应 revision/版本。

因此：

- 作者要把“第1章必须亲眼目击老铁匠被杀”传给 Planner，只能写进小说简介；M1 样本就是这样
  由验收执行器注入的，不是产品 UI 的章节目标字段。
- 不能证明“某个章节目标字段 → 哪个版本 → 何时进入 Planner”的产品级链路；这是 P1 缺口。
- 本轮不新增虚假章节目标字段，也不把小说简介字符串拼接冒充章节目标契约。

建议后续产品变更：新增持久化 `chapterBrief`（或 `chapterGoal`）与 revision，在创建/编辑
章节时写入，Planner 上下文以结构化来源带入并在 ContextTrace/Planner 输入中记录 hash；
这不是本轮时间盒内可安全完成的迁移。

## 6. Run 与版本一致性（Phase E）

**完整。** context 之后的 review/repair 版本固定由 R2 测试覆盖，context 之前的 Planner
book 输入窗口在本轮补齐（第 4.4 节）：

- `review_pins_the_context_plan_version_per_run`：Run-A/Run-B 各自读取自己的 context 计划版本。
- `a_reviewing_run_keeps_its_own_frozen_plan`：Run-A 审校固定版本 1，不被版本 2 替换。
- `a_run_without_its_own_context_never_reads_another_run`：当前 Run context 缺失不跨 Run 回退。
- `legacy_context_is_attributed_by_artifact_run_id`：旧 context 通过产物 run_id 归属。
- `missing_plan_version_is_recovered_from_trace_hash`：旧 context 用来源 hash 定位冻结计划。
- `unrecoverable_plan_version_fails_closed`：无法定位时暂停。
- 新增：`planner_book_inputs_stay_frozen_after_author_updates` 及其单元测试覆盖 context 之前
  的 story_macro / chapter_list 更新窗口，并用负向对照证明测试有效。

选择规则（`choose_run_artifact`）：本 Run 产物 → 合同冻结版本 → 旧合同最新非 stale。
冻结版本缺失时失败并暂停，不读取更新后的版本。`RunContract.source_versions` 为空有两种
来源：旧合同（无该字段）与打开时书里没有任何计划产物；两者都按旧行为读取最新非 stale。
后一种是已知边界：打开时没有可冻结的输入，运行期新增计划仍会被读取。

## 7. 工程测试与 CI

```text
cargo fmt --all -- --check                         PASS
cargo clippy --workspace --exclude novel-http --lib -- -D warnings   PASS
cargo test -p novel-application                    241 lib + 全部集成套件 PASS
cargo test -p novel-prompt                         6 PASS
cargo test --workspace --exclude novel-http        631 passed，0 failed
cargo test -p novel-storage-sqlite --features test-harness  114 passed，0 failed
client typecheck                                   PASS（本轮未改客户端，沿用分支既有结果）
client test                                        290 passed（同上）
server typecheck                                   PASS（同上）
Native E2E                                        见下
```

GitHub Actions（PR #27，模块拆分阶段 head `e2e8f434`；P1 修复提交的检查见 PR 页面）：

| 检查 | 结果 |
| --- | --- |
| Rust 格式 / Clippy / 单测 | PASS（3m35s） |
| 前端类型检查与单测 | PASS（1m3s） |
| 发布脚本单测 | PASS（12s） |
| Bundle 构建检查（Linux x64） | PASS（5m41s） |
| Bundle 构建检查（macOS arm64） | PASS（10m10s） |
| Tauri Kernel Check（rust-and-desktop-contract） | PASS（9m7s） |
| 原生端到端（可选） | SKIPPED（CI 配置） |

报告提交是仅文档提交；上面的结果对应同一代码 head，最终 PR HEAD 的检查见 PR 页面。

Native E2E：在 P1 修复后重新构建 Beta-A2 E2E 应用（`pnpm build:tauri:e2e`），使用
`scripts/run-p172-e2e-isolated.sh` 的独立数据目录重跑 3 个 spec：

| Spec | 结果 |
| --- | --- |
| `p17-4-7-chapter-creation` | PASS（3/3） |
| `author-journey` | PASS（1/1；mock 已补 keyEvents/sourceEventId） |
| `p17-3-context` | PASS（4/4；上下文来源与冻结 Run） |

3/3 spec 全部通过（P1 修复后重新构建并重跑）。完整套件没有全部重跑；原因：本轮变更
范围是 Planner 契约、Run 冻结、上下文预算与客户端 Prompt 版本，其他 spec 未触及对应
路径。E2E 不是人工验收。

并发说明：模块拆分阶段，一次全量 workspace 运行与 E2E 应用并行时，`chapter_history` 的
`restore_protects_local_drafts_and_records_provenance` 因 SQLite busy 冲突失败；停止
E2E 应用后单独重跑为 621 passed，0 failed。该失败是测试环境并发冲突，不是代码回归。
P1 修复后的最终干净重跑为 **631 passed，0 failed**。

### M1 Mock 矩阵（工程链路，不等于 M1 PASS）

| 场景 | 确定性测试/状态 |
| --- | --- |
| keyEvents 缺失 | `strict_story_macro_requires_key_events`、集成 `strict_new_story_macro_without_key_events_pauses` → 暂停 |
| keyEvents 正确 | `chapter_plan_must_cover_hard_story_key_events`、`covered_story_key_events_reach_the_writer_with_source_id` |
| 义务内容反向 | `source_event_content_cannot_be_rewritten` → BLOCK |
| 参与者替换 | 同上 → BLOCK |
| hard 降级 | 同上 → BLOCK |
| 因果/顺序反转 | 规范 description/type/chapter 校验可拦截被改写的事件；语义判断仍由 Reviewer/M1 |
| 正文违反义务 | 既有 Reviewer/Repair 套件与 M1 失败路径探针 |
| Reviewer 遗漏 high / Repair 仍残留 | 依赖真实模型；Mock 不能证明语义质量 |

Mock 通过只代表工程链路与 fail-closed 行为，不把 M1 改为 PASS；Beta-A2 HEAD 的 M1 仍是
`RETEST_PENDING`。

## 8. 未完成任务及原因

1. Phase C 章节目标字段：数据模型缺口，需要迁移 + UI + contract，未在时间盒内实现。
2. Phase F usage 逐次调用：时间不足，保留 P2 与复现证据，未改代码。
3. Native E2E 的完整套件没有全部重跑；本轮只跑与 Planner 契约/章节创建相关的冒烟。
4. 真实 M1 复测未执行；Beta-A2 新合同需要新的明确授权、模型与预算。
5. `worker.rs` 仍超过 1,300 行硬阈值（已知 P2）；本轮 P1 修复只在既有函数内改接线，
   没有新增职责；按职责拆分留待后续阶段。

## 9. M1 最新真实验收状态

- **beta `d66d565c`（Beta-A1 最终 HEAD）**：授权 M1 主样本 PASS。Run
  `c62263b7-d65a-422e-bca3-0e904abb65ad`，6 次调用，6 条 keyEvents 全覆盖，Reviewer clean，
  候选 2,412 字；作者于 2026-11-08 接受，未要求修改。见
  `acceptance/beta-a1` 的 `BETA-A1-ACCEPTANCE-RESULT.md`。
- **Beta-A2 HEAD**：由于 story-macro-v2 严格 keyEvents、规范内容校验、上下文预算与客户端
  版本固定均是新契约，**M1 为 RETEST_PENDING**；此前 PASS 不能直接继承。
- M2–M10：NOT_RUN。本轮没有调用真实付费模型。

## 10. 人工 GUI

`MANUAL_GUI_NOT_RUN`。自动化 E2E 不能冒充人工验收；第 7 节的 E2E 只覆盖桌面链路。

## 11. 下一阶段建议

1. 复核本 PR 的 Run 冻结实现与测试，确认合并策略；本报告不代替人工评审，也不自动合并。
2. 为 M1 执行器固定 `story_macro: story-macro-v2` / `chapter_list: chapter-list-v3`，
   在新的明确授权、模型与预算下重跑 M1；Mock 通过不能标 M1 PASS。
3. 设计并实现持久化章节目标字段（Phase C），补齐作者输入到 Planner 的可审计链路。
4. 修复 Phase F usage：逐次 Provider 响应一条 Invocation，保留 billed_unknown。
5. 人工 GUI 验收按 Beta-A1 清单执行；通过前保持 `MANUAL_GUI_NOT_RUN`。

## 12. PR

Draft PR：https://github.com/baicie/AI-Novel-Writing-Assistant/pull/27（不自动合并）。
