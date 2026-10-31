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
| P1-2 | `src-tauri/src/commands/run.rs`、`crates/novel-application/src/worker.rs` | 按环节绑定在 Director 运行链路里不生效（已修复，见下）：Run 创建时只解析 `writer` 一条路由并冻结（`run.rs` 的 `resolve_route("writer")`），worker 三个生成点全部使用 `claimed.contract` 的单一路由；`review` / `repair` / `planner` 等绑定只有前端主动调 `generate_text` 并带 `taskType` 时才会被用 | **已修**：新增 `RunContract.step_routes` 冻结表 + worker 按步骤取路由 + 旧合同回落；新增 3 条用例 |

### P1-2 实施结果（已完成）

- 修复内容：
  - `crates/novel-application/src/provider_service/model_route.rs`：新增 `FrozenStepRoute`（provider/model/protocol/endpoint/allowLocalhostHttp/credentialRef/reasoningEffort，**不含密钥正文**）与 `EffectiveRoute`；
  - `crates/novel-application/src/dto.rs`：`RunContract` 增加 `step_routes: BTreeMap<String, FrozenStepRoute>`（`serde(default)`，旧 Run 读出来是空表）与 `effective_route(task_type)`；
  - `crates/novel-application/src/provider_service/service.rs`：新增 `frozen_step_routes()`，把当前绑定一次性解析成冻结表；
  - `src-tauri/src/commands/run.rs`：Run 创建时解析冻结表并写入合同（仍只写 credential_ref）；
  - `crates/novel-application/src/worker.rs`：`task_type_for_step`（draft→writer、consistency_review→review、bounded_repair→repair、story_macro/character_cast/volume_strategy/chapter_list→planner…），三个生成点按环节取路由，表缺失或该环节未绑定则回落到合同单一路由；调用记录里的 provider/model 也改成按环节记录。
- 测试：`step_routes_take_priority_over_contract_route`、`legacy_contract_without_step_routes_falls_back`、`missing_step_entry_falls_back_to_contract`（`cargo test -p novel-application --lib` 186 passed）。
- 兼容性：`p9_repair_flow`、`p9_context_flow`、`p8_director_loop`、`p10_confirm_edit`、`p11_lease_fence`、`p11_repair_faults`、`p12_long_generation`、`p145_batch_crash_binding`、`recovery`、`runtime_provider_http` 等既有链路测试全部通过（workspace exit 0）。

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

**IMPLEMENTATION_COMPLETE_EXTERNAL_VALIDATION_PENDING** —— P0 全清、P1 全清；本地门禁（fmt/clippy/workspace 66 目标/前端 215 用例/Native E2E 5 规格）全绿。
原因：远端 CI 受「禁止 push」限制无法运行，按规则不宣称 CI 通过。
