# Native MVP Release Readiness

基线：`340c8b01`。本文件区分“开发者可用技术预览版”和“正式生产发布版本”。

## 1. 验收状态

| 项目 | 状态 | 证据 |
| --- | --- | --- |
| Runtime / Provider | VERIFIED | Tauri `llm_provider()` 使用 `OpenAiCompatibleProvider`；真实 Runtime + 本机 HTTP/SSE + SQLite 测试；`src-tauri/tests/runtime_provider.rs`、`runtime_provider_http.rs` |
| Workflow / Gate | VERIFIED | 真实 macOS WDIO：启动、Character Cast、Volume Strategy、Chapter Approval、Commit |
| UI 操作 | VERIFIED（核心） | WDIO embedded WebDriver；director 2 tests × 3 rounds 全通过；cancel/safety/restart 各通过 |
| 持久化 | VERIFIED（Native UI） | 独立 App 进程重启后正文、officialVersion、Run、Artifact 恢复 |
| 取消 / 恢复 | VERIFIED（取消）/ PARTIAL（恢复） | UI 取消真实通过；Safety 禁止普通 resume；Provider paused 的 UI Resume 未做按钮 E2E |
| 竞态处理 | PARTIAL | 单书单活跃 Run、stale expectedVersion、重复命令幂等有后端测试；双击/双窗口/Channel 断线 UI 未覆盖 |
| Editor | PARTIAL | 本地草稿、显式保存、CAS、保存中禁用、切换提示、draft race 单测；无 autosave debounce；中文输入法未验收 |
| 生产构建隔离 | VERIFIED | `withGlobalTauri:false`、capabilities 仅 default、无 WDIO binary markers、生产前端无 wdio |
| macOS 发布准备 | PARTIAL | .app/DMG 可用但未签名/公证；完整 GUI 人工验收未做 |

## 2. 自动化证据

### Rust

```bash
cargo test --workspace --exclude novel-http
# 221 passed

# 用户未跟踪文件不进入的独立临时 worktree：
git worktree add /tmp/biz-novel-clean HEAD
CARGO_TARGET_DIR=<repo>/target cargo test --workspace --all-targets
# PASS
```

原工作树 `--all-targets` 仍被用户旧文件阻塞；独立干净 Worktree 在最终 HEAD `340c8b01` 的 `cargo test --workspace --all-targets` 再次 PASS，这是独立阻塞，不是已提交代码失败。

### Native UI E2E

```text
pnpm --filter @ai-novel/desktop-e2e test:director
2 passing × 3 rounds
Round 1: 16s, pass
Round 2: 14s, pass
Round 3: 24s, pass
No flaky failure.
```

```text
test:cancel      1 passing
test:safety      1 passing (issues > 0)
test:restart     1 passing (new app process)
```

### 编辑草稿单元测试

```bash
pnpm --filter @ai-novel/client test
186 passed
```

新增：

- busy refresh 不覆盖作者草稿
- 旧保存响应不覆盖更新输入
- 既有 external edit / restored draft / malformed draft 测试

## 3. 生产构建隔离

- `src-tauri/tauri.conf.json`：`withGlobalTauri:false`，capabilities 仅 `default`
- 生产 binary 无 `tauri_plugin_wdio`、`tauri-plugin-wdio`、`wdio-webdriver`、`TAURI_WEBDRIVER_PORT`
- 生产前端 bundle 无 wdio
- E2E 插件只在 `--features e2e`
- E2E 假 Key 只在 `feature=e2e` 的 `E2eModelSecrets`
- Mock endpoint 只在 `BIZ_NOVEL_E2E_ENDPOINT` 的 E2E 启动路径
- 生产无 Node/Electron/sidecar

## 4. 人工验收清单

### 编辑器与输入法

1. 打开测试章节，切换到系统拼音输入法。
2. 连续输入中文组合，观察组词、候选、标点与光标。
3. 在 composition 未结束时触发保存，确认正文不丢字、不重复。
4. 修改正文后切换章节，确认未保存提示/草稿恢复。
5. 保存后切换章节再返回，确认正文一致。
6. 执行撤销/重做、粘贴、长文滚动。
7. 关闭窗口再重开，确认正文和草稿恢复。
8. 失败时保存：数据库副本、Run/日志路径、操作步骤、输入法版本、截图。

### Director UI

1. 用真实模型配置启动单章节 Run。
2. 观察三个 Gate，核对产物内容。
3. 审批中点击取消，确认最终状态合法。
4. 刷新页面，确认 Run 历史和当前状态恢复。
5. 关闭 App 再重开，确认正文和 Run 历史。
6. 双窗口/双击操作确认不产生两个活跃 Run。
7. 失败时保存 WDIO/后端日志、截图、Run ID、controlVersion。

## 5. 发布结论

- **开发者可用技术预览版：YES**。macOS arm64 未签名 `.app`/DMG 可用；核心 Native Director 自动化闭环通过。
- **正式生产发布版：NO**。未签名/公证，未完成真实 Provider/真实输入法/完整产品功能等价验收。
- 原 Electron 产品的全量功能不等价；图像、短剧、漫画、移动端、云端协作未迁移。

## 6. P9 建议

1. 补齐双击、双窗口、Channel 断线、过期 Gate UI 竞态。
2. 补 Provider paused / budget paused 的 UI Resume 按钮 E2E。
3. 完成 Plate 中文输入法 composition、撤销/重做、关闭重开人工验收。
4. 真实 Provider 小预算联调与真实 usage/限流验证。
5. 签名、公证、DMG/Tauri bundler、升级与回滚。
6. 逐模块迁移旧 Workspace/角色/质量债/Propmt 语义。
