# 模型配置与推理强度：代码质量审查结论

审查范围：本会话围绕「桌面模型来源 + 推理强度」的 10 个提交（`a827a10e`…`d1b95e58`，均未推送）。
审查目标：在不改变产品行为的前提下，找出结构、重复、错误处理、安全、文案与测试缺口，并处理低风险项。

## 已处理（行为等价，有测试证据）

| 编号 | 位置 | 问题 | 处理 |
| --- | --- | --- | --- |
| Q1 | `client/src/components/onboarding/QuickSetupDialog.tsx` | 组件层反向依赖 `pages/settings`，只为一个错误文案函数和地址判定 | 抽出 `client/src/platform/desktopModelRules.ts`，旧模块改为再导出 |
| Q2 | `client/src/platform/desktopApiAdapter.ts` | 明文 http 判定与设置页各写一份正则，规则容易漂移 | 统一走 `needsPlaintextHttpPermission` |
| Q3 | `requiresLocalhostPermission` | 语义早已变成「任何明文 http」，名字却写着 localhost | 规范名改为 `needsPlaintextHttpPermission`，旧名保留为再导出避免破坏引用 |
| Q4 | `src-tauri/src/commands/model.rs` | `let _ = sync_legacy_setup_to_registry(...)` 静默吞错，注册表同步失败无痕 | 改为失败时 `tracing::warn`，用户路径行为不变 |
| Q5 | `client/tests` | 新增的两条纯函数规则没有测试 | 新增 `desktopModelRules.test.js`（明文 http 判定 + 错误文案优先级） |
| Q6 | 发布说明 / README | 8 个用户可见提交没有更新发布说明，README 还留着两个历史日期块 | 合并到当日条目；README 只保留最新日期块 + 历史链接 |
| Q7 | `crates/novel-platform/src/secrets.rs` | 启用 `os-keychain` 特征时 clippy 报两处 `collapsible_if`，连带 `-D warnings` 门槛失败 | 改为 `is_ok_and` / `ok().and_then(...)` 等价写法，并补回退文件往返用例（写入可读、0600、删除后读不到） |
| Q8 | `crates/novel-application/src/provider_service.rs`（1630 行） | 超仓库 1300 行硬阈值 | 拆成 `provider_service/` 下的 error / model_route / service / tests 四个模块（194 / 180 / 398 / 861 行），导出路径不变，182 条用例数量与拆分前一致 |
| Q9 | `client/src/components/layout/DesktopModelSelector.tsx` | 选了模型未声明的档位时没有任何反馈 | 增加「未声明档位」提示，不阻断保存 |
| Q10 | `client/src/platform/desktopApiAdapter.ts`（968 行） | 已过 1000 行警戒线，映射与分发混在一处 | 拆出 `desktopShapeAdapters.ts`（221 行）放纯映射，适配层降到 771 行；类型检查、211 条前端测试与真机 E2E 14 条通过 |
| Q11 | `client/src/pages/settings/ProviderRegistrySection.tsx`（456 行） | 表单、发现、模型与绑定编辑挤在一个组件 | 抽出 `providerRegistry/ProviderModelList.tsx` 与 `ProviderBindingList.tsx`，组件降到 374 行，只保留状态与编排 |
| Q12 | `crates/novel-platform/src/secrets.rs` | 钥匙库不可用时密钥以明文文件落盘 | 改为 ChaCha20-Poly1305 加密（文件头 + 随机 nonce + 以槽位 id 作 AAD），主密钥优先放钥匙库、否则 0600 文件；旧明文文件读取时自动加密迁移；新增 4 条测试（往返/迁移/篡改/跨槽） |
| Q13 | `crates/novel-application/src/provider_service/service.rs` | 明文 HTTP 授权后，改地址到别的主机也会被静默放行 | 授权绑定主机：换主机必须显式确认（新方法 `upsert_provider_with_confirmation`，命令层加 `confirmPlaintextHostChange`，设置页加确认勾选），并有针对性测试 |
| Q14 | 推理强度与协议能力 | Ollama 协议不支持强度时界面没有任何提示 | 导航栏与任务绑定在 Ollama 来源上显示「该协议不支持强度」，仍不阻断保存 |

验证：`cargo fmt --all -- --check` 干净；`cargo clippy -p novel-desktop -p novel-llm --lib -- -D warnings` 通过；`cargo test -p novel-platform --lib` 4 passed、`cargo test -p novel-llm --lib` 24 passed、
`e2e/provider-selector.e2e.js` 此前已在真实 app 里断言选择器与绑定落库。

## 遗留状态

上一轮记录的 D3（密钥明文回退）、D4（明文 HTTP 任意主机）、D6（Ollama 无强度提示）已全部处理，见 Q12–Q14；
本轮没有新的遗留项。

## 复核建议

1. 新增映射优先加到 desktopShapeAdapters；新增设置项优先加到 providerRegistry 下的子组件。
2. 明文授权已绑定主机（Q13）；后续若要收紧，优先增加「按主机记录授权历史」，不要回退到 HTTPS-only。
3. 密钥已是加密存储（Q12），旧明文文件在首次读取时自动迁移；若要换算法，同样要保留「旧格式仍可读」的兼容期。
4. 审查规则本体见 `docs/wiki/architecture/code-quality-rules.md`，后续按该清单自查。
