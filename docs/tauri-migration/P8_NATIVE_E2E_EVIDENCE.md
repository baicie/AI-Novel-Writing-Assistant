# P8 Native Director E2E Evidence

基线：`3ab6d54b`。本文件只记录实际执行结果，不把页面渲染、单元测试或 ScriptedLlm 冒充真实 Provider/真实 GUI。

## G1 Provider Runtime：VERIFIED

- Runtime Composition Root：`src-tauri/src/lib.rs`
- Provider：`novel_llm::OpenAiCompatibleProvider`
- 事实代码：

```rust
let llm: Arc<dyn novel_llm::LlmProvider> =
    Arc::new(OpenAiCompatibleProvider::new());
```

- `RuntimeDeps.llm` 使用该 Provider；`OsModelSecrets` 从系统凭据读取 Key。
- 新增测试：`crates/novel-application/tests/runtime_provider_http.rs`
  - 启动本机 `127.0.0.1:0` Mock HTTP/SSE Server
  - 使用真实 `OpenAiCompatibleProvider`
  - 真实 `Runtime` / Worker / SQLite
  - 模型返回结构化 Story Macro SSE
  - 断言 SQLite 中出现结构化 `story_macro` artifact
- 结果：

```text
cargo test -p novel-application --test runtime_provider_http
1 passed

cargo test -p novel-desktop --test runtime_provider
2 passed
```

`novel-desktop` 测试直接调用 Tauri Release composition root 的 `llm_provider()`，通过真实 HTTP/SSE 完成请求，防止把测试 Provider 当成 Tauri 装配。

- 测试使用本地假 Key，不调用真实付费 Provider。
- 无 Key 时 Provider 返回明确错误，不生成成功产物。

## G2 取消与恢复：VERIFIED（后端自动化）

- 取消：`cancel_generation` 触发 CancellationToken；Worker 写 `cancelled`。
- 新增断言：
  - 无 `chapter_commit` artifact
  - 正式正文仍为空
  - 取消后下一次 Worker tick 为 Idle
- 既有测试：`cancel_generation_during_draft_returns_cancelled_instead_of_pausing`
- Bounded Repair fail-closed：
  - `consistency_issues_fail_closed_without_fake_repair_or_commit`
  - issues>0 时 `bounded_repair` 暂停
  - 无 `repair_notes`、无 `chapter_commit`、正文为空
- 安全暂停不能被普通 resume 绕过：
  - `consistency_issues...` 测试中调用 `resume(..., true)` 必须返回冲突
  - Storage 在 `PauseKind::Safety` 上拒绝普通 resume
- Tauri provider 装配测试：`src-tauri/tests/runtime_provider.rs` 直接调用 `novel_desktop::llm_provider()`，通过真实本地 HTTP/SSE 完成一次请求。
- 真实 HTTP 流取消测试：同一 Tauri composition provider 在收到首个 SSE token 后取消，3 秒内返回 `LlmError::Cancelled`，不是只改数据库状态。
- 结果：

```text
cargo test -p novel-application --test p8_director_loop
3 passed

cargo test -p novel-application --test recovery
6 passed
```

## G3 Tauri UI 操作：VERIFIED（核心操作）

已执行真实 macOS Tauri WebDriver（embedded provider）：

- WebdriverIO 9.31.9
- `@wdio/tauri-service` 1.5.0
- `@wdio/tauri-plugin` 1.5.0
- Rust `tauri-plugin-wdio` 1.5.0
- Rust `tauri-plugin-wdio-webdriver` 1.5.0
- 隔离临时 SQLite/应用数据目录
- 本机 HTTP/SSE Mock Provider

通过的真实按钮测试：

```text
pnpm --filter @ai-novel/desktop-e2e test:director
2 independent tests × 3 rounds, all passing
```

- 页面/启动表单（独立测试数据）
- 点击启动真实 Rust Run
- Character Cast Gate 确认
- Volume Strategy Gate 确认
- Chapter Approval Gate 确认
- Run succeeded + 正文/revision 持久化
- 页面刷新恢复
- 连续 3 轮：16s / 14s / 24s，无 flaky

```text
pnpm --filter @ai-novel/desktop-e2e test:cancel
1 passing
```

- 真实点击取消
- cancelled
- 正文为空

```text
BIZ_NOVEL_E2E_SCENARIO=issues pnpm ... test:safety
1 passing
```

- Safety pause
- 无 repair_notes/chapter_commit
- 无普通 resume 按钮
- 正文为空

```text
BIZ_NOVEL_E2E_DATA_DIR=<kept> pnpm ... test:restart
1 passing
```

- 独立 App 进程重启后正文、Revision、Run 历史、Artifact 恢复

未覆盖：双击、双窗口、Channel 断线重连、过期版本 UI、中文拼音输入法。G3 核心操作 **VERIFIED**，边缘用例 PARTIAL。

## G4 持久化：VERIFIED（Native UI）

测试：

```text
structured_director_loop_reaches_committed_chapter_and_survives_reopen
```

覆盖：

- 真实 SQLite
- ScriptedLlm（结构化合同测试）
- artifact / Gate / commit
- 重开 SQLite 后正文和 revision 一致
- RunDetail 投影

未在真实 GUI 重启中操作验证；UI 持久化标记 PARTIAL。

## G5 质量 Gate

- `cargo fmt --check`：PASS
- `cargo clippy --workspace --exclude novel-http --all-targets`：PASS（既有 warning）
- Rust workspace（排除用户未跟踪阻塞文件）：222 tests PASS
- 独立干净 Worktree：`cargo test --workspace --all-targets` PASS（HEAD `d7ad9d2c`，不包含用户未跟踪文件）
- HTTP 追踪测试：57 PASS
- `pnpm typecheck`：PASS
- `pnpm build`：PASS
- client tests：180 PASS
- Tauri app build：PASS
- DMG verify：PASS
- bundle audit：PASS
- Channel smoke：PASS
- Director UI smoke：PASS
- `cargo test --workspace --all-targets`：BLOCKED（用户未跟踪 `integration_runs.rs`）

## Gate 结论

| Gate | 状态 | 依据 |
| --- | --- | --- |
| G1 Provider runtime | VERIFIED | 真实 HTTP/SSE Provider 测试通过 |
| G2 Cancel / resume safety | VERIFIED（后端） | 取消不提交、Safety 暂停不可绕过 |
| G3 UI 操作 | VERIFIED（核心）/ PARTIAL（边缘） | macOS WDIO 真实按钮启动/三 Gate/Commit/Cancel/Safety/刷新 |
| G4 持久化 | VERIFIED（Native UI） | 独立进程重启后正文、Revision、Run、Artifact 恢复 |
| G5 质量 | PARTIAL | 相关检查通过；all-targets 被用户文件阻塞；GUI 未验收 |

G1–G4 已有实际自动化证据；G5 仍 PARTIAL（all-targets 被用户文件阻塞、中文拼音输入法人工验收未做、边缘 UI 未覆盖）。Native Director 核心自动化闭环 VERIFIED，整体 P8 仍不宣布完成。
