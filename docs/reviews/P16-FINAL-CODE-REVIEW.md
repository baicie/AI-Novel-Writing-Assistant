# P16 最终独立代码审查

分支 `feat/p16-provider-management`，审查范围 `22cebf96..HEAD`（49 个提交）。
审查立场：以代码不变量与数据安全为准，不以设计文档措辞为准。

## 0. 先说结论

- **P0（阻塞）**：本轮共发现 4 项，全部已修复并复跑门禁。
- **P1（必修）**：发现 2 项，其中 1 项已修复（计费提示），1 项未完成（按任务执行环节分别选模型的运行时链路），**这是最终判定为 PARTIAL 的唯一原因**。
- **P2（记录）**：3 项，见 `P16-REMAINING-RISKS.md`。

## 1. P0：会让门禁或产品直接坏掉的问题（已修）

| # | 位置 | 问题 | 处理 | 证据 |
| --- | --- | --- | --- | --- |
| P0-1 | `crates/novel-storage-sqlite/tests/workflow_lease.rs` | 给 `RunContract` 增加 `reasoning_effort` 后，该测试构造字面量缺字段，整包 `cargo test --workspace` 编译失败 | 补 `reasoning_effort: None` | workspace tests exit 0 |
| P0-2 | `crates/novel-llm/tests/discovery_http.rs` | `generate_with_protocol` 增加强度参数后，调用点少一个实参，编译失败 | 补 `None` | 同上 |
| P0-3 | `crates/novel-platform/src/secrets.rs` | 主密钥「钥匙库优先」在特性统一构建下出现双权威：同一进程可能拿到两把密钥，刚写的密文自己解不开（2 条用例在 workspace 全量下必失败） | 改为**文件为唯一权威**（钥匙库只做尽力而为副本）+ 进程内互斥 + `create_new` 抢占 | 连续 5 次 7/7 通过；workspace exit 0 |
| P0-4 | `e2e/wdio.conf.js` | 导航栏选择器规格 `provider-selector.e2e.js` 从 specs 列表丢失，等于该回归没在跑 | 重新注册 | e2e 5/5 规格通过 |

## 2. P1：影响真实可用性的问题

| # | 位置 | 问题 | 状态 |
| --- | --- | --- | --- |
| P1-1 | `client/src/pages/settings/DesktopModelSettingsCard.tsx` | 「保存并测试」实际会向网关发一次真实生成请求（`src-tauri/src/commands/model.rs::test_model_setup`），界面没有任何费用提示，违反「付费测试不得伪装成免费探测」 | **已修**：加提示「会发送一次真实请求（只要求回一个字），可能产生少量费用」，按钮改为「保存并发送测试请求」 |
| P1-2 | `src-tauri/src/commands/run.rs`、`crates/novel-application/src/worker.rs` | **按环节绑定在 Director 运行链路里不生效**：Run 创建时只解析 `writer` 一条路由并冻结（`run.rs` 的 `resolve_route("writer")`），worker 三个生成点全部使用 `claimed.contract` 的单一路由；`review` / `repair` / `planner` 等绑定只有前端主动调 `generate_text` 并带 `taskType` 时才会被用 | **未完成**（见下节） |

### P1-2 详细说明与实施计划

- 现状证据：
  - `src-tauri/src/commands/run.rs:90`：`.resolve_route("writer")`
  - `crates/novel-application/src/worker.rs:424 / 655 / 1323`：三个生成点都用 `claimed.contract.*`，无按环节路由
  - `crates/novel-application/src/director.rs`：只 `use crate::provider_service::FrozenRoute`
- 影响：界面允许为 11 个环节分别绑定模型，但在 Director 跑长文时，审校/修复仍走正文那条冻结路由。这属于「UI 显示支持、运行时未生效」。
- 计划（不破坏既有合同）：
  1. `RunContract` 增加 `step_routes: BTreeMap<String, FrozenStepRoute>`（`serde(default)`，旧 Run 读出来是空表，行为不变）；
  2. Run 创建时按真实环节键解析并冻结（`chapter_draft`/`review`/`repair`/…），仍然**不写明文密钥**，只写 credential_ref；
  3. worker 三个生成点按自身环节键查表，查不到则回落到合同里的单一路由（保证旧 Run 与未绑定环节一致）；
  4. 补测试：多环节冻结表生效、旧合同回落、环节缺失模型时报错而不是静默换来源。

## 3. 依赖方向与结构核查

| 检查项 | 结论 |
| --- | --- |
| 领域层是否依赖上层 | 否：`novel-domain` 只有实体与规则 |
| Application 是否直接碰 SQL/钥匙库 | 否：通过 `ProviderRegistry`/`CredentialVault` 端口；Tauri 命令只做 DTO 与调用 |
| 是否存在第二套模型配置 | 存在兼容入口（`save_model_setup`），但已与注册表同表同源并在写入时 bump revision；两条入口共用 `provider_credential`，不构成第二权威 |
| 是否新增未接线的 Registry | 否：命令层、run、generation 均已接入，`provider_flow` 集成测试覆盖 |
| 文件规模 | `provider_service/` 已拆为 error/model_route/service/tests（194/180/436/895），适配层 770 + 形状层 221，设置页 387 + 2 个子组件 |

## 4. 测试有效性核查（不把 Mock 当真实支持证明）

- 所有「模型发现 / 生成」测试使用本地 mock HTTP server，只能证明**协议实现正确**，不能证明某家厂商线上可用；`P16-REMAINING-RISKS.md` 已写明。
- Anthropic 目前只有发现适配，`protocol_supports_generation` 与前端 `GENERATION_READY_PROTOCOLS` 都把它排除在生成之外，UI 绑定选择器不会允许其用于生成——属于「明确禁止」而不是「假装能用」。
- Ollama 有生成实现，但无推理强度参数；界面已提示「该协议不支持强度」。

## 5. 旧数据与 P15 安全机制回归

- `p149_backup_restore`、`p151_historical_migration` 全绿；`0022/0023/0024` 迁移与 legacy import 未被破坏。
- 人工正文保护、质量停点、候选稿 CAS、Lease Fencing 的既有测试均在全量 workspace 中通过（未被本轮改动触碰）。

## 6. 过程问题（必须披露）

`crates/novel-http/tests/integration_runs.rs` 属于受保护文件，但它在提交 `7b89c0fd`（全仓 fmt 清理）中被一并加入版本库。本轮**没有**再次修改或删除它，也未用破坏性 Git 操作回滚。处理建议见 `P16-REMAINING-RISKS.md` 第 4 条。

## 7. 最终判定

**PARTIAL** —— 唯一阻塞是 P1-2（Director 运行链路未按环节使用绑定模型）。其余 P0 与 P1-1 已修复，门禁与 Native E2E 全绿。
