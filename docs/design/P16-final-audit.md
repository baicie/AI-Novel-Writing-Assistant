# P16 最终验收对照（12_COMPLETION_GATES）

> 分支 `feat/p16-provider-management`。未 push / 未 merge / 未 release。
> 判定口径严格按设计包：**任一 FAIL/BLOCKED 即不得写 `P16_COMPLETE`**。

## Gate A — 功能

| 条目 | 判定 | 证据 |
| --- | --- | --- |
| P16.2 catalog / Ollama / selection 回归 | PASS | `cargo test -p novel-llm`（23 单元 + 19 本地 HTTP，含 Ollama 与脱敏） |
| Service/Repository/CredentialVault 接线，Command 不写事务逻辑 | PASS | `commands/provider.rs` 只做 DTO→Service；`provider_registry_adapter.rs` 实现端口 |
| Provider CRUD / 模型编辑删除 / 发现 preview+apply / binding 全部有命令与前端入口 | PASS（后端+transport）；UI 为 PARTIAL | 13 命令 + `providerTransport.ts`；`ProviderRegistrySection` 未做组件级测试 |
| 未保存 Provider 可发现并选模型，保存后重启仍在 | PASS | 草稿 discovery + previewToken；原生冷启动后 `provider_model` 表存在（P16-evidence §4） |
| 手动模型不因发现失败/空返回丢失，覆盖不被刷新 | PASS | `apply_with_empty_selection_is_a_noop`、`apply_keeps_user_overrides` |
| 新旧配置共享权威 + legacy revision bump + 跨窗口 stale 拒绝 | PASS | `legacy_save_bumps_provider_revision`、三条并发用例（Provider/绑定/删除模型） |
| Keep/Replace/Clear、崩溃补偿、无明文、不跨域借用默认 Key | PASS | 意图表 + reconcile；`credential_failure_never_writes_the_database`；凭据槽 = provider id |
| 默认模型 / 任务绑定 / 生成 / Director / Review-Repair 路由 | PASS（review/repair 走同一冻结路由） | `resolve_route`、`apply_route_to_run`、`generate_text.taskType` |
| Running Run 冻结模型/端点/凭据引用，恢复不漂移 | PARTIAL | `RunContract.protocol/credentialRef` + worker 优先用引用；**Director 仅对 writer 绑定填冻结值**，review/repair 复用同一 contract（无独立绑定入口） |
| 「可用于创作」的协议有真实 generation adapter | PARTIAL | `protocol_supports_generation` 仅放行 openai-compatible；Anthropic/Ollama 仍只有发现能力，已禁止绑定 |

## Gate B — 质量/安全

| 条目 | 判定 | 证据 |
| --- | --- | --- |
| `cargo fmt --all -- --check` | FAIL（未全仓验证） | 仅对涉及 crate 跑 `cargo fmt`；`novel-http` 历史 fmt 债务未清 |
| 涉及 crate Rust test/clippy | PASS | application 181 / storage 17 / desktop 21 / llm 42；lib 级 `-D warnings` 通过 |
| `p151_historical_migration` 进 CI | PASS | 两个工作流新增 `--features test-harness` 步骤；本地同命令绿 |
| 真实本地 HTTP mock（认证/分页/重定向/取消/超时/无付费） | PASS | `discovery_http.rs` 19 条 |
| 前端 typecheck / contract | PASS | `pnpm --filter @ai-novel/client typecheck`；209 条前端测试（含 5 条 transport 契约） |
| Tauri Native E2E | PARTIAL | 已做「原生冷启动 + 迁移 + 命令进入产物」；**未做 webdriver 交互** |
| 密钥不可见（响应/日志/导出） | PASS（已覆盖面） | 视图/路由/预览快照断言 + 错误消息不回显密钥用例 |
| 受保护未跟踪文件未触碰 | PASS | `crates/novel-http/tests/integration_runs.rs` 始终 `??` |

## Gate C — 审计/交付

| 条目 | 判定 | 证据 |
| --- | --- | --- |
| 每阶段独立 commit、HEAD 可核对 | PASS | 见 `git log` |
| 设计与测试证据在仓库 | PASS | `docs/design/P16-*.md` |
| 用户可见变更说明与回滚说明 | PASS（见下） | 本文件 §回滚 |
| 不 push / 不 merge / 不发布 / 无付费调用 | PASS | 全程本地 |
| 最终报告逐项 PASS/FAIL/BLOCKED | PASS | 本文件 |

## 回滚说明

- 迁移 0022/0023/0024 全部是**纯增量**（ADD COLUMN / CREATE TABLE IF NOT EXISTS）：旧版本应用打开新库可忽略新列与新表，无需降级脚本。
- 回滚代码：`git revert` 对应 commit 即可；不存在数据删除，因此不需要数据修复。
- 密钥：凭据始终在系统钥匙库，键名为 provider id；回滚不会删除用户密钥。
- 若只想停用新界面：`client/src/pages/settings/DesktopSettingsView.tsx` 移除 `ProviderRegistrySection` 即可，旧「默认模型」卡片仍可用。

## 结论

状态：**PARTIAL**。未通过项：

1. Gate A「Running Run 冻结」——review/repair 没有独立绑定入口，冻结值只覆盖 writer 路径；
2. Gate A「可用于创作协议」——Anthropic/Ollama 仍无生成适配器；
3. Gate B「`cargo fmt --all -- --check`」——未全仓验证，`novel-http` 有历史债务；
4. Gate B「Tauri Native E2E」——缺 webdriver 交互证据。

在这些项补齐前，不写 `P16_COMPLETE`。
