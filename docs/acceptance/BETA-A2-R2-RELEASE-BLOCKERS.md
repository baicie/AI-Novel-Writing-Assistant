# Beta-A2-R2 发布阻断修复与 M1 执行器安全门禁

## 结论

- **BUILD：PASS**（Linux x64 与 macOS arm64 Bundle 在修复提交 `bd240f34` 上实际构建通过；
  本轮后续提交只改 M1 执行器与测试，最终 HEAD 的 CI 见 PR）
- **PLANNER_CONTRACT：PARTIAL**（M1 执行器已固定并使用 story-macro-v2 + chapter-list-v3；
  作者章节目标字段仍是数据模型缺口）
- **M1_RUNNER_SAFETY：PASS**（请求前调用预算已强制；人民币费用上限无法本地执行，真实模式
  fail-closed 并要求供应商侧硬额度）
- **ENGINEERING：READY_FOR_REVIEW**
- **M1：RETEST_PENDING**
- **MANUAL_GUI：NOT_RUN**
- **PUBLIC_BETA：NOT_READY**

## 1. 构建回归（Phase 1）

### 根因

`src-tauri` 包在加入 `src/bin/m1_run.rs` 后包含两个 binary target：`novel-desktop`
（`src/main.rs`）与 `m1_run`（验收执行器）。`Cargo.toml` 没有 `package.default-run`，
Tauri CLI 无法确定打包入口，Linux Bundle 失败于 `failed to find main binary`（运行
42441799638、42441948166）。

### 修复

- `src-tauri/Cargo.toml`：`[package] default-run = "novel-desktop"`。
- `src-tauri/tests/main_binary_contract.rs`：双 binary 回归门禁。
- 未改产品名、Bundle 标识、安装路径、应用数据目录与 Release 流程；`--bin m1_run`
  显式调用方式不变。

### 验证

| 检查 | 结果 |
| --- | --- |
| `cargo check -p novel-desktop` | PASS |
| `cargo check -p novel-desktop --bin m1_run` | PASS |
| `cargo test -p novel-desktop` | PASS（40 lib + 13 m1_run + 门禁用例） |
| CI：Bundle 构建检查（Linux x64） | **PASS**（9m56s，运行 42445055424） |
| CI：Bundle 构建检查（macOS arm64） | **PASS**（6m22s） |
| CI：Rust 格式 / Clippy / 单测 | PASS（7m27s） |
| CI：rust-and-desktop-contract | PASS（5m46s） |
| CI：前端类型检查与单测 | PASS（57s） |

`bd240f34` 是包含本修复的 head；本轮后续提交不触碰打包入口与 release 依赖图，最终 HEAD
的 CI 结果以 PR #28 页面为准，不用历史 commit 替代。

## 2. M1 执行器 Planner 版本（Phase 2）

`m1_run.rs` 现在通过 `m1_prompt_versions()` 固定（`src-tauri/src/bin/m1_run.rs`）：

| 步骤 | 版本 |
| --- | --- |
| story_macro | `story-macro-v2` |
| chapter_list | `chapter-list-v3` |
| chapter_draft | `chapter-draft-v3` |
| chapter_repair | `chapter-repair-v2` |

- 运行前打印脱敏清单 `M1_PROMPTS story_macro=… chapter_list=… chapter_draft=… chapter_repair=…`，
  不含 endpoint、凭据或提示词正文。
- `open_run` 的成对校验保证只固定一半时拒绝；旧 Run 的 v1 契约不受影响。
- 仓库里已有 `RunContract.source_versions` 冻结与 `story_macro`/`chapter_list` 版本写入，
  本轮不改其实现。

### 失败先行/契约测试（`src-tauri/src/bin/m1_run.rs`，13 项）

| 测试 | 证明 |
| --- | --- |
| `prompt_versions_pin_the_new_planner_contract` | 四项版本被固定，清单脱敏 |
| `planner_templates_load` | v2/v3 模板真实存在 |
| `m1_run_uses_v2_prompts_and_writer_gets_source_event` | Mock 全链路：Worker 实际使用的 story/chapter 模板为 v2/v3，Writer 收到 `来源事件：ch1-event-1` |
| `missing_key_events_stops_the_m1_run` | v2 契约下缺 keyEvents → DataIntegrity 暂停，Writer 未启动 |
| `rewritten_canonical_event_stops_the_m1_run` | 章节计划改写规范事件 → 暂停 |
| `legacy_v1_prompts_stay_compatible` | 不固定新版本的历史调用仍可跑通 |

“RunContract 真正冻结版本”通过行为验证：只有合同携带 `story_macro=story-macro-v2` 时，
Worker 才会执行严格 keyEvents 校验；缺 keyEvents 的 Mock 输出被暂停，证明合同确实带上了
v2，而不是仅配置字符串。

## 3. M1 费用与调用预算门禁（Phase 3）

`src-tauri/src/bin/m1_run/budget.rs` 新增 `CallBudget` + `BudgetedLlm`（包裹真实 Provider）：

- **请求前检查**：每次 `generate_with_protocol` / `generate` 先占用一次额度；额度用尽时
  返回 `LlmError::BudgetExceeded`，请求不会到达 Provider。
- **逐次占用**：正文生成、结构化调用与结构化修复、探针调用都走同一入口，各自占用预算。
- **未知 usage**：`TokenUsage` 任一字段缺失即记录 `unknown_usage`，不推断零成本。
- **失败与取消**：超时、取消等错误仍占用一次调用并标记 usage 未知（可能已计费）。
- **输出**：`M1_BUDGET_GUARD` 打印上限与费用门禁；`M1_RESULT` 打印
  `requests_started` 与 `unknown_usage`。

### 费用上限

本地没有可信定价与请求前预留，不能强制执行人民币上限。因此：

- 真实模式必须设置 `M1_COST_GUARD=provider_side`，确认供应商侧已设置硬额度；
- 未设置时 **fail-closed**，`M1_REFUSED` 输出阻断原因与改造建议；
- `M1_MAX_COST_CNY` 保留为记录值，明确标注 `local_cost_enforcement=disabled`，不当成
  可执行上限；
- Dry-run 不调用模型，不受该限制。

### 预算测试

`single_request_counts_once`、`structured_call_and_repair_each_consume_budget`、
`call_limit_blocks_before_the_request_is_sent`（拒绝时 inner provider 调用数不增加）、
`missing_usage_is_recorded_as_unknown`、`provider_timeout_still_consumes_the_call_and_marks_usage_unknown`、
`cancelled_request_still_consumes_the_call`、`real_mode_requires_a_provider_side_cost_guard`。

## 4. PR #27 合并后复核（Phase 4）

定向复核以下实现，未发现可复现的新 P0/P1：

- **Planner 契约**：story-macro-v2 缺 keyEvents、空数组、全 advisory → fail-closed；hard
  不能降级；sourceEventId 与规范事件一致；旧产物宽松读取；新入口显式固定 v2/v3。
- **Run 冻结**：`open_run` 快照 + `choose_run_artifact`（本 Run 产物 → 冻结版本 → 旧合同
  最新非 stale）；缺冻结版本 fail-closed；历史 Run 的空冻结表保持旧语义；作者更新后新建
  Run 快照最新版本，旧 Run 不受影响。集成测试与负向对照见 `p17_4_10_run_freeze.rs`。
- **Context**：结构化预算不产生截断 JSON，keyEvents 保留，未来章节义务不进入当前上下文，
  无法满足必需预算时暂停。
- **已记录但不可复现**：`target_plan_from_artifacts` 对冻结版本仍过滤 stale，而
  `choose_run_artifact` 接受冻结的 stale 版本。仓库只在 gate reject/regenerate 时把**本
  Run** 的 draft 产物标记 stale，作者保存新计划不会把旧版本标 stale，因此当前无法构造
  “context 通过、审校失败”的实际场景。按“只修复实际复现的 P0/P1”，本轮不改。

## 5. 回归（Phase 5）

| 检查 | 结果 |
| --- | --- |
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --exclude novel-http --lib -- -D warnings` | PASS |
| `cargo clippy -p novel-desktop --all-targets -- -D warnings` | PASS |
| `cargo test -p novel-desktop` | 40 lib + 13 runner + 门禁用例 PASS |
| `cargo test -p novel-application` | PASS |
| `cargo test --workspace --exclude novel-http` | **645 passed，0 failed** |
| Client typecheck / test | NOT_RUN（本轮未改客户端） |
| Native E2E | NOT_RUN（本轮未改桌面运行路径；manifest 与验收 bin 不参与 E2E 场景） |
| Tauri Bundle | 本机 NOT_RUN；CI 在 `bd240f34` 上 Linux/macOS Bundle PASS |

模块拆分：`m1_run.rs` 692 行、`m1_run/budget.rs` 135 行、`m1_run/tests.rs` 660 行，均在
1,200 行建议阈值内；测试通过 `#[path = "m1_run/tests.rs"]` 引入，不新增 binary target。

## 6. 重新授权执行 M1 的工程条件

**工程侧已满足**（构建、Planner 版本固定、请求前调用预算、费用 fail-closed），但真实运行
仍需本轮**单独授权**：确认模型、调用次数上限、供应商侧费用硬额度与安全凭据，并设置
`M1_AUTHORIZED=1`、`M1_MAX_CALLS`、`M1_MAX_COST_CNY`、`M1_COST_GUARD=provider_side`。
此前任何一次授权不延续。授权项、预检命令、失败路径探针与停止条件见
[BETA-A2-M1-RETEST-RUNBOOK.md](./BETA-A2-M1-RETEST-RUNBOOK.md)。

离线演练已在当前 `beta` HEAD 上复跑通过：离线 mock、无网络、无费用，8 次调用走到
`waiting_approval`，义务携带 `sourceEventId`，Reviewer 识别 `ch1-req-1` 后一次修复收敛。
该演练只证明执行器与契约接线正常，不构成 M1 证据。

M1 仍是 `RETEST_PENDING`；M2–M10 `NOT_RUN`；人工 GUI `NOT_RUN`。

## 7. 尚未完成的 P1/P2

- **P1（数据模型）**：作者当前章节目标没有持久化字段（Phase C 缺口，未在本轮扩大范围）。
- **P2**：本地费用上限不可执行，只能依赖供应商侧硬额度；如后续引入可信定价，可加请求前
  预留与事后对账。
- **P2**：`worker.rs` 仍超过 1,300 行硬阈值。
- **P2**：完整 Native E2E 未重跑（与本轮改动无交集）。

## 8. 本轮约束遵守情况

- 未修改 `main`，未 force push，未自动合并，未创建 Tag/Release。
- 未调用真实付费模型，未使用任何 Key（含此前暴露过的 Key）。
- 未进入 P17.5。
