# P8 Tauri Director 原生 UI 合同

基线：`5ed98ec0`；本切片实现 `739190fd` 之后。目标是通过 Tauri WebView 操作真实 Rust Run，不使用 Express/Node sidecar。

## 1. 已由 Rust 实现的操作

| 操作 | Tauri Command | 持久化事实 |
| --- | --- | --- |
| 运行列表 | `list_runs` / `list_all_runs` | `workflow_run` + `workflow_control` |
| 当前小说活跃 Run | `get_active_run` | queued/running/waiting_approval/paused |
| Run 详情 | `get_run_detail` | contract、control、gate、cursor、candidate、artifacts、usage |
| 事件补拉 | `replay_run` | `workflow_event` 的 seq |
| 实时事件 | `subscribe_run` | Tauri Channel；Lag 时按 seq 补拉 |
| 创建 Run | `open_run` | 单书单活跃 Run，RunContract 冻结 |
| 确认 Gate | `approve_run` | commandId 幂等；可选 expectedVersion；draft -> confirmed |
| 取消 | `cancel_run` | commandId 幂等；终态 cancelled |
| 恢复候选 | `resume_run` | 仅 paused；保留远端重试预算 |
| 模型设置 | `get_model_setup` / `save_model_setup` | OS Keychain + SQLite |

`get_run_detail` 的状态只来自 `workflow_control.status`，不是 React 拼接或硬编码。

Provider 装配：`src-tauri/src/lib.rs` 的 Release composition root 暴露 `llm_provider()`，使用 `OpenAiCompatibleProvider`；`src-tauri/tests/runtime_provider.rs` 已通过本地 HTTP/SSE 验证该装配，不是 `UnconfiguredProvider`。

安全暂停：`bounded_repair` 尚未实现时 fail-closed；`PauseKind::Safety` 禁止普通 `resume`，UI 同时隐藏该恢复按钮。

## 2. 尚未实现 / Unsupported

- `resolve_gate.regenerate`
- `resolve_gate.confirm_after_edit`
- 独立 `reject`
- `handoff`
- `driver` auto/assisted 切换
- 多章节批量范围、stepIdsInScope
- Story Input 自定义输入（Rust 用小说标题/描述和 Seed）
- `worldMode`、`targetMode`、`issuePolicyMode`、temperature
- 旧版角色候选、完整 Workspace、质量债治理
- 完整 `bounded_repair`：当前 fail-closed，审校有问题直接暂停

这些操作不显示为可执行按钮，不能悄悄映射为 approve/resume。旧 API 未知路径在 desktop adapter 返回 `NOT_MIGRATED` 501。

## 3. 旧版 → Rust 语义差异

### open_run

旧版字段：`driver`、`stepIdsInScope`、`launchInput.storyInput`、`estimatedChapterCount`、`worldMode`、`targetMode`、`executionRange`、`issuePolicyMode`、`temperature`。

Rust `OpenRunRequest` 只接受：

- `commandId`、`novelId`、`targetChapterId`
- `provider`、`model`
- `promptVersions`
- `tokenBudget`、`maxAutomaticRetries`
- `endpoint`、`allowLocalhostHttp`

UI 仅允许“选择已有章节 + 已配置模型 + 单章节 Run”。不得伪造 driver/范围支持。

### resolve_gate

旧版：

```text
resolve_gate { decision: confirm | confirm_after_edit | regenerate, expectedVersion }
```

Rust：

```text
approve_run { commandId, runId, expectedVersion? }
```

- `confirm` → `approve_run`
- `confirm_after_edit`、`regenerate` → Unsupported，不映射
- Rust `expectedVersion` 已支持；过期版本返回 conflict，commandId 仍幂等

### 状态映射

`waiting_gate` → `waiting_approval`；`completed` → `succeeded`；`paused` / `failed` 保留真实值。不得把 `paused`/`failed` 显示成 `succeeded`。

## 4. 旧页面依赖

旧 `DirectorNovelPage`、`DirectorPanel`、`DirectorGate`、`DirectorStart`、`DirectorRunHistoryPage` 仍依赖：

- `getDirectorSummary/Detail/Workspace/Ledgers`
- `getDirectorCharacterCandidates`
- `/director-next/commands`
- 旧角色、运行历史、用量接口

这些未迁移的 Express 投影不能作为 Tauri 后端。桌面路由通过 `hasDesktopShell()` 使用新页面，Web 仍保留旧页面。

## 5. 本轮 Tauri UI 范围

实现状态：**PARTIAL**。Release WebView 路由/启动表单 smoke 通过；真实按钮启动 Run、审批、取消、恢复仍为人工验收 BLOCKED。

已实现：

- `/lab/director/:novelId` 桌面使用 `TauriDirectorPage`
- `/lab/director` 桌面使用 `TauriDirectorHistoryPage`
- 选择章节、显示已配置模型/endpoint/API Key 状态
- 创建单章节 Run（typed `openRun`）
- Channel 订阅、replay 补拉、seq 去重、卸载 cleanup
- 状态、当前 Step、Gate、候选、artifact、usage 展示
- approve（带 expectedVersion）、cancel、paused 候选 resume
- 运行历史只读列表、完成后正文入口
- 不支持操作明确不显示

未实现：

- 真实按钮级 Run/审批/取消/恢复自动化验收或人工 GUI 验收
- 旧 Workspace/角色/审阅空间、多章节、driver、world、quality policy
- 拒绝/重新生成/编辑确认
- 真实 Provider 联调

## 6. macOS Native UI E2E 结果

- WebdriverIO 9.31.9
- `@wdio/tauri-service` 1.5.0
- `@wdio/tauri-plugin` 1.5.0
- `tauri-plugin-wdio` 1.5.0
- `tauri-plugin-wdio-webdriver` 1.5.0
- embedded WebDriver，macOS WKWebView

真实按钮通过：

- 启动 Run
- Character Cast Gate 确认
- Volume Strategy Gate 确认
- Chapter Approval 确认
- Commit 后读取正式正文
- Cancel 后正文为空
- Safety 暂停后无普通 Resume 按钮
- 页面刷新恢复
- 独立 App 进程重启后正文/Revision/Run/Artifact 恢复

未覆盖：双击、双窗口、Channel 断线、过期版本 UI、中文拼音输入法、撤销/重做。

证据：`tests/desktop-e2e/`、`P8_MACOS_UI_E2E.md`、`P8_NATIVE_E2E_EVIDENCE.md`。
