# Beta-A2-R2 发布阻断修复与 M1 执行器安全门禁

## 结论

- **BUILD：FAIL**（根因已定位并修复；Linux Bundle 未实际构建成功，需新 HEAD CI 确认）
- **PLANNER_CONTRACT：PARTIAL**（Beta-A2 已合并的契约未被改动；M1 执行器仍未固定 v2/v3）
- **M1_RUNNER_SAFETY：BLOCKED**（无请求前调用预算，无强制费用上限）
- **ENGINEERING：NEEDS_FIX**
- **M1：RETEST_PENDING**
- **MANUAL_GUI：NOT_RUN**
- **PUBLIC_BETA：NOT_READY**

## 0. 时间与范围

任务硬截止为 2026-11-08 12:00（UTC+8）。本轮开始执行时为 11:55，实际可开发时间不足
5 分钟。按优先级只完成 Phase 1 的最小修复与回归门禁；Phase 2–5 未能开始，以下逐项说明
现状，不把未执行的工作写成通过。

## 1. Phase 1：Tauri Bundle 构建回归（已修复，待 CI 验证）

### 根因

`src-tauri` 包在加入 `src/bin/m1_run.rs` 后包含两个 binary target：

- `novel-desktop`（`src/main.rs`，正式桌面应用）
- `m1_run`（`src/bin/m1_run.rs`，内部 M1 验收执行器）

`src-tauri/Cargo.toml` 没有 `package.default-run`。Tauri CLI 无法判断打包入口，Linux
Bundle 失败于 `failed to find main binary, make sure you have a package > default-run in
the Cargo.toml file`（运行 42441799638、42441948166）。

### 修复

`src-tauri/Cargo.toml` 的 `[package]` 增加：

```toml
default-run = "novel-desktop"
```

不改产品名、Bundle 标识、安装路径、应用数据目录与 Release 流程；不引入 Cargo feature，
`cargo run --bin m1_run` / `cargo build --bin m1_run` 的显式调用方式保持不变。

### 双 binary 回归门禁

新增 `src-tauri/tests/main_binary_contract.rs`：

- 断言 `package.default-run` 指向 `novel-desktop`；
- 断言 `src/main.rs` 仍是桌面入口；
- 断言 `src/bin/m1_run.rs` 仍以显式 target 存在。

移除 `default-run` 或误把 M1 执行器设为默认入口时，该测试直接失败。

### 本轮实际验证

| 检查 | 结果 |
| --- | --- |
| `cargo check -p novel-desktop` | PASS |
| `cargo check -p novel-desktop --bin m1_run` | PASS |
| `cargo test -p novel-desktop --test main_binary_contract` | PASS（1/1） |
| `cargo fmt --all --check` | PASS |
| Tauri Linux Bundle | **NOT_RUN**（本机 macOS，无法执行 Linux 打包） |
| Tauri macOS Bundle | **NOT_RUN**（时间不足） |

按任务要求“只有实际构建成功才能判定此问题修复”，BUILD 保持 **FAIL**，直到新 HEAD 的
CI Linux Bundle 为 PASS。本轮不把源码修复等同于构建通过。

## 2. Phase 2：M1 执行器 Planner 版本未固定（未完成）

`src-tauri/src/bin/m1_run.rs` 的 `prompt_versions` 仍只包含：

- `chapter_draft` → `chapter-draft-v3`
- `chapter_repair` → `chapter-repair-v2`

`story_macro` / `chapter_list` 未显式固定，真实运行时可能走旧 Planner，无法验证 Beta-A2
契约。任务要求的脱敏版本清单、RunContract 冻结断言、模板级验证与 7 项 Mock 测试均未实现。

结论：真实 M1 运行路径当前**不满足**验收条件。

## 3. Phase 3：费用与调用预算门禁（未完成，BLOCKED）

现状（未改代码）：

- `M1_MAX_COST_CNY` 只被解析、校验与展示，没有扣减或预留逻辑。
- `M1_MAX_CALLS` 在一次 `worker.tick()` 结束后检查；一次 tick 内可能发生结构化修复等多次
  Provider 请求，因此不是请求级上限。
- 未知 usage 与“超时但可能已计费”没有请求前预算保护。

结论：不存在可强制执行的人民币费用硬上限，M1_RUNNER_SAFETY 判 **BLOCKED**。在实现
请求前预算检查与逐次调用记账之前，不得启用真实付费运行；真实运行必须依赖供应商侧硬额度，
或在本地 fail-closed。

改造建议（不搭建新计费系统）：

1. 在 Provider 请求统一入口检查剩余调用预算，超限即拒绝并记录阻断原因。
2. 每次 Provider 响应单独记一条 Invocation；结构化修复单独占用一次预算。
3. 未知 usage 保留 `billed_unknown`，不得按零成本推断。
4. 费用上限仅在“可信定价 + 请求前预留 + 事后对账”三者同时成立时声明可执行。

## 4. Phase 4：PR #27 合并后正确性复核（未完成）

时间不足，未完成对 `obligations.rs`、`worker_validation.rs`、`worker/context_budget.rs`、
`context.rs`、`director.rs`、workflow plan/recover、SQLite workflow repository 与 Prompt
Compiler 的系统复核。PR #27 合并后的代码未在本轮改动，Beta-A2 的冻结与契约测试仍以
`a2` 分支已合并的 631 项 workspace 测试为准；本轮没有新增反例证据。

## 5. Phase 5：回归与 CI（部分完成）

本轮执行：`cargo check`（桌面与 m1_run）、`main_binary_contract` 测试、`cargo fmt`。
未执行：clippy 全量、novel-application / novel-prompt 测试、Run freeze 测试、Client
typecheck/test、Native E2E——本轮只改了 Tauri manifest 与一个测试文件。

CI 状态：

- beta `14e7944c`：**FAIL**（Linux Bundle，运行 42441799638、42441948166）。
- 本修复的新 HEAD：见 Draft PR 页面；本轮不以历史 commit 的成功替代新 HEAD 验证。

## 6. Phase 6：Beta 验收准备

- M1：`RETEST_PENDING`。
- M2–M10：`NOT_RUN`。
- 人工 GUI：`NOT_RUN`，沿用既有验收流程，本轮不新建冗余文档。
- 本轮未调用任何真实模型，未使用任何 Key（包括此前暴露过的 Key）。

## 7. 重新授权执行 M1 的工程条件

**当前不满足。** 需要同时具备：

1. 新 HEAD 的 CI Linux Bundle PASS（本修复合入并通过）；
2. M1 执行器固定 `story-macro-v2` + `chapter-list-v3`，并输出脱敏版本清单；
3. 请求前调用预算与未知计费处理到位，费用上限真实可执行或明确 fail-closed；
4. 本轮单独的模型、调用上限、费用控制与安全凭据授权。

此前任一次 M1 授权不延续到下一次。

## 8. 尚未完成的 P1/P2

- **P1**：M1 执行器 Planner 版本固定与 Mock 验证（Phase 2）。
- **P1**：请求前调用预算、费用上限与未知计费（Phase 3）。
- **P1/P2**：Phase 4 复核清单。
- **P2**：Phase 5 的完整回归与 Native E2E。

## 9. 修复位置

- `src-tauri/Cargo.toml`：`[package] default-run = "novel-desktop"`。
- `src-tauri/tests/main_binary_contract.rs`：双 binary 回归门禁。
