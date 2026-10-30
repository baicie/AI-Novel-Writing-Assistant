# P16 验收证据（本地）

生成时间：2026-10（本机 macOS arm64）
分支：`feat/p16-provider-management`
HEAD：`0adc19b8`（本轮新增提交见文末）

## 1. 门禁结果

| 门禁 | 命令 | exit code | 证据 |
| --- | --- | --- | --- |
| 格式 | `cargo fmt --all -- --check` | 0 | 无 diff |
| 静态检查 | `cargo clippy --workspace --exclude novel-http --lib -- -D warnings` | 0 | 无告警 |
| 后端全量测试 | `cargo test --workspace --exclude novel-http` | 0 | 66 个测试目标全 ok，0 FAILED（日志 `/tmp/p16-ws2.log`） |
| 前端类型 | `pnpm --filter @ai-novel/client typecheck` | 0 | tsc 无错 |
| 前端测试 | `pnpm --filter @ai-novel/client test` | 0 | 215 passed / 0 fail |
| Native E2E 构建 | `pnpm tauri build --target aarch64-apple-darwin --bundles app --features e2e --config src-tauri/tauri.updater.conf.json` | 0 | 产出 `target/aarch64-apple-darwin/release/bundle/macos/Biz Novel Studio.app` |
| Native E2E 运行 | `BIZ_NOVEL_DATA_DIR=/tmp/p16-gate-e2e2 npx wdio run e2e/wdio.conf.js` | 0 | 5 个规格文件全通过（smoke / flow / no-missing-routes / quick-setup-models / selector） |

## 2. 关键测试清单（真实执行的用例）

| 关注点 | 位置 | 结果 |
| --- | --- | --- |
| 凭据三态（Keep/Replace/Clear）、CAS 冲突、删除保护、跨来源不串密钥 | `crates/novel-application/src/provider_service/tests.rs`（25 条 provider 用例） | ok |
| 绑定优先于旧默认配置、冻结路由校验、缺失模型报错 | 同上 | ok |
| 发现结果不覆盖人工修改 | `crates/novel-storage-sqlite/src/provider.rs` 的 `preserved` 计数 | ok |
| 明文 HTTP 主机绑定（换主机需确认） | `provider_service/tests.rs::plaintext_authorization_is_bound_to_host` | ok |
| 密钥加密落盘、旧明文迁移、篡改拒绝、跨槽隔离 | `crates/novel-platform/src/secrets.rs` 的 4 条 fallback 用例 | ok |
| 推理强度随请求发送 | `crates/novel-llm/tests/gateway.rs::reasoning_effort_is_forwarded_when_selected` | ok |
| 发现解析与 /v1 回退 | `crates/novel-llm/tests/discovery_http.rs`（22 条） | ok |
| 迁移与备份 | `crates/novel-storage-sqlite/tests/p151_historical_migration.rs`、`p149_backup_restore.rs` | ok |
| 桌面适配层不落未映射接口 | `e2e/provider-no-missing-routes.e2e.js` | ok |
| 导航栏来源/模型/强度选择与落库 | `e2e/provider-selector.e2e.js` | ok |
| 首次上手获取模型列表 | `e2e/quick-setup-models.e2e.js` | ok |

## 3. 本轮修复导致的门禁变化

```text
修复前：workspace tests exit=101（workflow_lease.rs 缺 reasoning_effort 字段）
修复后：workspace tests exit=0（66 个测试目标全 ok）
修复前：workspace tests exit=101（discovery_http.rs 调用少一个参数）
修复后：exit=0
修复前：novel-platform 2 条用例在特性统一构建下失败（钥匙库与文件双权威）
修复后：连续 5 次 7/7 通过，workspace 全量 0 失败
修复前：e2e 只跑 4 个规格（selector 规格被漏注册）
修复后：5 个规格全通过
```

## 4. 提交

```text
7b89c0fd … 0adc19b8   49 个提交（含本轮质量治理与缺陷修复）
本轮新增提交见 P16-FINAL-CODE-REVIEW.md 末尾
```
