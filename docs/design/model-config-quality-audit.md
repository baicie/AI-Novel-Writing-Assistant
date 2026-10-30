# 模型配置与推理强度：代码质量审查结论

审查范围：本会话围绕「桌面模型来源 + 推理强度」的 8 个提交（`a827a10e`…`4a335086`，均未推送）。
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

验证：`pnpm --filter @ai-novel/client test` 211 passed、`typecheck` 通过、`cargo check -p novel-desktop --lib` 无错、
`e2e/provider-selector.e2e.js` 在真实 app 里断言选择器与绑定落库。

## 记录在案但本轮不处理（含原因）

| 编号 | 位置 | 问题 | 为何不做 |
| --- | --- | --- | --- |
| D1 | `crates/novel-application/src/provider_service.rs`（1630 行） | 超过仓库硬阈值 1300 行，按架构规则应当拆分 | 拆分要重排 port 与用例边界，属于结构改造，必须单独一轮并配全套测试，不能夹在质量清理里做 |
| D2 | `client/src/platform/desktopApiAdapter.ts`（967 行） | 接近 1000 行警戒线，映射与 DTO 适配混在一处 | 尚未越线；下一批新增映射前应先按「路由映射 / 形状适配」拆成两个模块 |
| D3 | `crates/novel-platform/src/secrets.rs` | 钥匙库不可靠时密钥以 0600 明文文件落盘 | 是已知取舍：本地 ad-hoc 签名环境下钥匙库写入不可读，宁可可用且可见；要换加密存储需引入密钥派生与迁移，另开任务 |
| D4 | `crates/novel-llm/src/endpoint.rs` | 显式允许后，明文 http 可指向任意主机（含内网） | 与用户自建网关的现实需求一致，元数据与链路本地地址仍然拒绝；若要收紧应做成「按来源记住一次授权」，需要产品决策 |
| D5 | 推理强度档位 | 档位是自由文本，写错不会被校验，直接发给网关 | 档位集合来自各家网关差异，过早强校验会误伤；先在模型条目上声明，后续可加「声明之外的档位给警告」 |
| D6 | `crates/novel-llm/src/provider.rs` | Ollama 协议忽略推理强度 | 该协议没有对应字段，显式忽略并在代码注明，不伪造支持 |
| D7 | `client/src/pages/settings/ProviderRegistrySection.tsx`（456 行） | 表单状态、发现流程、绑定编辑挤在一个组件 | 属 D1/D2 同类的结构债，等设置页整体拆分时一起做 |

## 复核建议

1. D1/D2 拆分前不要继续往这两处加功能（尤其 provider_service.rs）。
2. 若要收紧 D4，优先做「记住一次授权」而不是回退到 HTTPS-only，否则又会挡住自建网关。
3. D3 若要改成加密存储，需要同时给出迁移路径与「旧文件仍可读」的兼容期。
