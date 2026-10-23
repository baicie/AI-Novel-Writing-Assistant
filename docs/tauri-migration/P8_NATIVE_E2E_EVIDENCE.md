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
1 passed
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
- 结果：

```text
cargo test -p novel-application --test p8_director_loop
3 passed

cargo test -p novel-application --test recovery
6 passed
```

## G3 Tauri UI 操作：PARTIAL / BLOCKED

已执行：

```text
scripts/smoke-tauri-director.sh
Tauri Director UI smoke passed
director ui smoke ok page=true start=true
```

该 smoke 证明：

- Release Tauri WebView 可打开 `#/lab/director/<novel>`
- 新 `TauriDirectorPage` 渲染
- 启动表单可见
- 不是旧 Express 页面

未执行：

- 通过真实鼠标/键盘点击“启动导演”
- 通过真实点击完成 Character Cast / Volume Strategy / Chapter Approval Gate
- 通过真实点击取消和恢复
- 关闭重开 GUI 后的正文/历史恢复

当前环境没有可用的 Tauri WebView 点击自动化驱动；不以 DOM 存在冒充点击测试。G3 标记 **PARTIAL / GUI 人工验收 BLOCKED**。

## G4 持久化：VERIFIED（后端）

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
- Rust workspace（排除用户未跟踪阻塞文件）：220 tests PASS
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
| G3 UI 操作 | PARTIAL / BLOCKED | 路由/表单 smoke；真实点击自动化不可用 |
| G4 持久化 | VERIFIED（后端） | SQLite 重开、RunDetail、正文/revision 测试 |
| G5 质量 | PARTIAL | 相关检查通过；all-targets 被用户文件阻塞；GUI 未验收 |

Native Director MVP 自动化闭环不能整体标记 VERIFIED，因为 G3 缺少真实按钮点击证据，且真实 GUI 人工验收未完成。
