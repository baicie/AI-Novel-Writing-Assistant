# P16 实施与验收证据（持续更新）

> 分支 `feat/p16-provider-management`。本文件只记录**可复核的证据与判定**，不写改动流水账。

## 1. 阶段状态

| 阶段 | 状态 | 证据 |
| --- | --- | --- |
| G1 Service + CAS + 旧接口 revision | PASS | `cargo test -p novel-application --lib provider_service`；`--test provider_registry`（含 `legacy_save_bumps_provider_revision`） |
| G2 命令 + DTO + transport + 契约测试 | PASS | 13 个命令注册；`cargo test -p novel-desktop --lib provider`（9 条）；`pnpm --filter @ai-novel/client test`（含 5 条 transport 契约） |
| G2 凭据意图补偿 | PASS（受限） | `credential_intent` 迁移 0024；replace 失败留 pending 且不写库；reconcile 可重放 clear，replace 转 `needsUserAction`（密钥正文从不落库） |
| G3 模型管理界面 | PARTIAL | `ProviderRegistrySection` + 8 条逻辑用例；**无组件级 RTL 测试**（仓库未引入 RTL，未擅自加依赖），桌面 UI 需人工验收 |
| G4 路由解析 + 生成接线 | PASS | `resolve_route` / `resolve_generation_target` / `generate_text.taskType`，20 条应用用例 |
| G4 Run 冻结与恢复校验 | PARTIAL | `RunContract.protocol/credentialRef` 可选字段 + worker 优先用冻结引用 + `check_run_route` 校验；**Director 尚未自动填充冻结字段** |
| G5 历史迁移进 CI | PASS | `tauri-ci.yml`、`tauri-kernel-check.yml` 新增 `--features test-harness` 步骤；YAML 解析校验通过 |
| G5 多窗口 CAS | PASS | `concurrent_updates_with_same_revision_conflict`、`concurrent_binding_writes_conflict`、`concurrent_model_delete_conflict`（事务内版本守卫） |
| G5 密钥不外泄 | PASS（已覆盖面） | `error_messages_never_echo_the_api_key`、`provider_view_serializes_camel_case`、`route_resolves_from_binding_without_secret` |
| G5 原生启动与迁移（真机） | PASS | 见第 4 节：当前 HEAD 构建的 `.app` 冷启动无 panic，新库迁移到 0024，`provider_model`/`model_binding`/`credential_intent` 均存在 |
| G5 原生 UI E2E（webdriver） | 未完成 | 需 wdio 驱动与交互脚本，本轮未做 |
| G6 最终验收 | 未执行 | — |

## 2. 关键不变量与对应测试

| 不变量 | 测试 |
| --- | --- |
| 旧 `save_model_setup` 也 bump revision（CAS 才能拦住混用） | `legacy_save_bumps_provider_revision` |
| 发现只追加合并不删除、空选择不清空 | `apply_adds_new_models_without_touching_others`、`apply_with_empty_selection_is_a_noop` |
| 用户覆盖不被远端刷新覆盖 | `apply_keeps_user_overrides` |
| stale discovery 不落库 | `stale_discovery_is_rejected` |
| 危险删除先过业务策略 | `delete_is_blocked_while_bindings_exist`、`default_provider_delete_needs_confirmation` |
| 凭据三态互斥 | `credential_keep_and_clear_reject_key`、`replace_requires_non_empty_secret` |
| 凭据失败不留半条配置 | `credential_failure_never_writes_the_database` |
| 视图/路由/预览不含密钥 | `provider_view_serializes_camel_case`、`route_resolves_from_binding_without_secret`、`discovery_preview_serializes_camel_case_without_secret` |
| 有绑定按绑定、无绑定不变旧行为 | `generation_target_prefers_binding_over_legacy`、`route_is_absent_without_binding` |
| 模型被删后恢复可诊断 | `frozen_route_reports_deleted_model`、`frozen_route_treats_unknown_provider_as_legacy` |

## 3. 已知限制（必须在最终报告如实列出）

1. **Anthropic / Ollama 只有发现适配器**，不能用于创作生成；UI 用 `canBindForGeneration` 限制绑定，命令层未强校验（P1）。
2. **凭据 replace 无法自动补偿**：密钥正文不落库，进程崩溃后只能要求用户重新输入（设计取舍，非缺陷）。
3. **Director 尚未从 route 填充冻结字段**：`RunContract.protocol/credentialRef` 目前恒为空，worker 回退到 `provider`，行为与旧版一致。
4. **无组件级 UI 测试**：仓库未引入 RTL，本轮未新增依赖；界面验收留给人工。
5. **既有债务**：`novel-http` 的 fmt/lint 与 `novel-storage-sqlite` 两个测试文件的 clippy 告警仍未处理（非本轮引入）。
6. **原生 E2E 未运行**：Tauri 端到端（含 wdio）需要构建桌面产物与驱动，本轮未执行，属外部/重量级验证缺口。
## 4. 原生启动与迁移证据（本轮实测）

构建：`pnpm tauri build --target aarch64-apple-darwin --bundles app --config src-tauri/tauri.updater.conf.json`（含更新签名）。

运行：`BIZ_NOVEL_DATA_DIR=/tmp/p16-native <app>/Contents/MacOS/novel-desktop`，12 秒后结束进程。

结果：

- 启动日志首行为 `webview runtime diagnostics … "mode":"desktop"`，说明 webview 正常起来；
- 日志中无 `panicked`；
- 新建数据库 `/tmp/p16-native/novel.sqlite` 里存在 `provider_model`、`model_binding`、`credential_intent` 三张表；
- `schema_migration` 最后四条版本为 `0021, 0022, 0023, 0024`；
- 产物二进制中包含 `list_provider_profiles`、`upsert_provider_profile`、`discover_provider_models`、`apply_provider_discovery`、`check_run_route`、`reconcile_credential_intents` 等命令名，证明 13 个 Provider 命令确实编译进了原生包。

**边界**：这是「原生启动 + 迁移 + 命令注册」的证据，**不是** UI 交互 E2E（未驱动 webdriver、未点击界面）。
