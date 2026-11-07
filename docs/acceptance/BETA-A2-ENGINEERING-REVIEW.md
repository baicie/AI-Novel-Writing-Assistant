# Beta-A2 全仓关键路径审计、Planner 事件契约封口与 M1 验收准备

## 结论

**ENGINEERING_STATUS：READY_FOR_REVIEW。**
**PLANNER_CONTRACT：PARTIAL。**
**M1_ACCEPTANCE：RETEST_PENDING（Beta-A2 HEAD 需要新的真实模型验收；Beta-A1 HEAD 已有一次授权 PASS）。**
**GUI_AUTOMATED：PASS（见第 7 节）。**
**MANUAL_GUI：MANUAL_GUI_NOT_RUN。**
**PUBLIC_BETA：NOT_READY。**

本轮修复了三个确定性契约缺陷：story-macro-v2 缺失 keyEvents 时静默通过、sourceEventId
只校验形式不校验规范内容、Planner 上下文按字符截断可能损坏 JSON/丢失关键事件。
作者“当前章节目标”没有独立的持久化字段，只能写进小说简介；这是明确的数据模型缺口，
本轮不凭空新增剧情要求，记录为 PARTIAL 原因。

## 1. 基线与最终 HEAD

| 项 | 值 |
| --- | --- |
| `beta` 基线 | `d66d565c254c8a70f11c5b5c833f50fba5f50518` |
| `main` | `eff69f229727ab90f4102c05cfc705116cb1ded5`，未修改 |
| 工作分支 | `fix/beta-a2-planner-invariants` |
| 代码 HEAD | `5a6bae18`（关键事件强制、规范内容、上下文预算） |
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
- `crates/novel-application/src/context.rs`、`p17_4_10_obligations.rs`：Run 冻结/版本测试。

## 3. 已确认的 P0 / P1 / P2

### P0

无。

### P1（已修复）

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

### P1（未修复，明确缺口）

4. **作者当前章节目标没有持久化字段**：详见第 5 节。产品数据模型只有小说标题/简介与章节
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
4. `worker.rs` 仍超过 1,300 行硬阈值，后续按职责拆分。

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

已确认并复用既有确定性测试，无需改代码：

- `review_pins_the_context_plan_version_per_run`：Run-A/Run-B 各自读取自己的 context 计划版本。
- `a_reviewing_run_keeps_its_own_frozen_plan`：Run-A 审校固定版本 1，不被版本 2 替换。
- `a_run_without_its_own_context_never_reads_another_run`：当前 Run context 缺失不跨 Run 回退。
- `legacy_context_is_attributed_by_artifact_run_id`：旧 context 通过产物 run_id 归属。
- `missing_plan_version_is_recovered_from_trace_hash`：旧 context 用来源 hash 定位冻结计划。
- `unrecoverable_plan_version_fails_closed`：无法定位时暂停。

本轮全量测试通过；未复现新的版本混用缺陷，因此没有修改 Run/CAS/租约逻辑。

## 7. 工程测试与 CI

```text
cargo fmt --all -- --check                         PASS
cargo clippy --workspace --exclude novel-http --lib -- -D warnings   PASS
cargo test -p novel-application                    233 lib + 全部集成套件 PASS
cargo test -p novel-prompt                         6 PASS
cargo test --workspace --exclude novel-http        621 passed，0 failed
cargo test -p novel-storage-sqlite --features test-harness  114 passed，0 failed
client typecheck                                   PASS
client test                                        290 passed
server typecheck                                   PASS
Native E2E                                        见下
```

Native E2E：从 Beta-A2 工作树重新构建 E2E 应用，使用独立数据目录运行 3 个 spec：

| Spec | 结果 |
| --- | --- |
| `p17-4-7-chapter-creation` | PASS（3/3） |
| `author-journey` | PASS（1/1；mock 已补 keyEvents/sourceEventId） |
| `p17-3-context` | PASS（4/4；上下文来源与冻结 Run） |

3/3 spec 全部通过。完整 11-spec 套件没有全部重跑；原因：本轮变更范围是 Planner 契约、
上下文预算与客户端 Prompt 版本，其他 spec 未触及对应路径。E2E 不是人工验收。

## 8. 未完成任务及原因

1. Phase C 章节目标字段：数据模型缺口，需要迁移 + UI + contract，未在时间盒内实现。
2. Phase F usage 逐次调用：时间不足，保留 P2 与复现证据，未改代码。
3. Native E2E 的完整套件没有全部重跑；本轮只跑与 Planner 契约/章节创建相关的冒烟。
4. 真实 M1 复测未执行；Beta-A2 新合同需要新的明确授权、模型与预算。

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

1. 评审并合并本 Draft PR（不在本轮自动合并）。
2. 为 M1 执行器固定 `story_macro: story-macro-v2` / `chapter_list: chapter-list-v3`，
   在新的明确授权、模型与预算下重跑 M1；Mock 通过不能标 M1 PASS。
3. 设计并实现持久化章节目标字段（Phase C），补齐作者输入到 Planner 的可审计链路。
4. 修复 Phase F usage：逐次 Provider 响应一条 Invocation，保留 billed_unknown。
5. 人工 GUI 验收按 Beta-A1 清单执行；通过前保持 `MANUAL_GUI_NOT_RUN`。

## 12. PR

Draft PR：本分支的 Draft PR（链接在提交后回填；不自动合并）。
