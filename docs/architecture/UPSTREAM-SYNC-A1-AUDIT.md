# Upstream-Sync-A1 审计与选择性同步报告

## 摘要

| 项目 | 结果 |
| --- | --- |
| 审计日期 | 2026-11-09 |
| Fork head（审计时） | `7d8e3a00`（`fix/beta-a3-reviewer-and-desktop-acceptance`） |
| Fork beta / main | `0a51abc0` / `eff69f22` |
| Upstream main / beta | `ebee8eef`（与 beta 同 SHA） |
| Merge-base | `5cd6a1324eeabb3990a872ad955eec20f75ecd3d` |
| 上游独有提交 | 118 |
| Fork beta 独有提交 | 372 |
| Fork main 独有提交 | 244 |
| 上游 rename-only 差异文件 | 1168（+63,561 / −8,879） |

**结论先行**：不建议整体合并上游。上游 118 个提交中真正值得引入的是三个工程契约——**物理调用独立记账**、**模型能力档案（ProviderCapabilityProfile）**、**结构化输出失败分类**——以及一批可直接转成 Fork 测试 fixture 的故障场景。本轮已落地其中范围最小、收益最高的一项：物理调用独立记账。其余形成 Backlog。

> 关于 Agent 协作：仓库 AGENTS.md 允许在用户逐次批准后使用 **Luna 系列**子代理，本环境未提供 Luna 模型，因此本轮审计与实现全部由主代理完成，未派生任何子代理。

---

## 1. 分叉状态

审计开始时重新 fetch 了 `origin` 与 `upstream`，未依赖任务描述中的旧 SHA。计算结果与任务给出的基线完全一致：

```
upstream/main ... origin/main : 118 (upstream-only) / 244 (fork-only)
upstream/main ... origin/beta : 118 (upstream-only) / 372 (fork-only)
upstream/beta ...  origin/beta : 118 (upstream-only) / 372 (fork-only)
merge-base                    : 5cd6a1324eeabb3990a872ad955eec20f75ecd3d
```

`upstream/beta` 与 `upstream/main` 同 SHA，说明上游没有独立的预发布分支，不能按 Fork 的 `beta` 模型对照。

## 2. 差异文件统计

上游侧（`merge-base..upstream/main`）：

| 目录 | 文件数 |
| --- | --- |
| `server/` | 836 |
| `client/` | 175 |
| `docs/` | 110 |
| `shared/` | 28 |
| `site/` | 8 |
| `desktop/` | 3 |
| `.github/` | 3 |
| 其他（lock、README、AGENTS、临时 summary） | 4 |

按状态：新增 621、删除 6、修改 541。

上游 118 个提交中，**96 个提交只碰 `docs/` 与 `README.md`**，15 个提交触及 `server/src/llm`，32 个触及 `client/`。真正带运行时语义的改动远少于提交数量给人的印象。

## 3. 架构差异（决定移植方式）

| 维度 | Upstream | Fork |
| --- | --- | --- |
| 运行时 | Node + Express + Prisma | Rust workspace（11 crates，182 个 `.rs`，约 64.8k 行）+ Tauri 2 |
| 桌面入口 | `desktop/` Electron/Tauri 薄壳 | `src-tauri` → `novel-application::Runtime` |
| 持久化 | Prisma migration | 手写 SQL migration（`migrations/0001..0028`）+ `novel-storage-sqlite` |
| 生成入口 | LangChain `@langchain/*` | `novel-llm` 自研 reqwest + SSE |
| 结构化输出 | `structuredInvoke` + Zod + 原生 JSON 模式 | 提示约束 + `serde` 校验 + 一次有界修复 |
| 编排 | `app/director/productionKernel` | `novel-workflow` + `novel-application::worker` |
| 任务中心 | Prisma 投影 | SQLite 投影 + Tauri IPC |

`server/` 在 Fork 中仍保留 1,803 个文件，但它是**旧内核遗留**，Tauri 组合根只装配 Rust crates。任何"把上游 TypeScript 搬进来"的做法都会同时维护两套运行时，因此本报告的所有移植建议都按"提取契约 + 在 Rust 中重写"处理。

## 4. 领域差异矩阵

图例：`UPSTREAM_ONLY` 上游有而 Fork 无；`FORK_ONLY` Fork 独有；`BOTH_DIFFERENT` 双方都有但路线不同；`ALREADY_EQUIVALENT` 能力等效；`INCOMPATIBLE` 依赖旧内核、不可直接引入。

| 领域 | 判定 | 证据与说明 |
| --- | --- | --- |
| 模型 Provider 与路由 | `BOTH_DIFFERENT` | 上游 `server/src/llm/factory.ts` + `modelRouter.ts` 走 LangChain 多协议；Fork `crates/novel-llm/src/router.rs` 只支持 `openai-compatible` 与 `ollama`，Anthropic 等协议仅能读模型列表、不能生成。上游多协议不能直接移植。 |
| 模型能力探测 | `UPSTREAM_ONLY` | 上游 `server/src/llm/compatibility/`（`profile.ts`/`defaults.ts`/`probe.ts`/`adapter.ts`/`store.ts`）；Fork `ProviderCapabilities` 是**静态 struct**，`structured_output: false`、`tool_call: false`（`crates/novel-llm/src/provider.rs:13-19`），无端点级隔离、无探测、无能力存储。 |
| 结构化输出与修复 | `BOTH_DIFFERENT` | 上游按 `json_schema → json_object → tool_calling → prompt_only` 顺序降级并带失败模式记录；Fork 只有提示约束 + 一次修复（`novel-llm/src/structured.rs`，`may_repair`），`parse_structured_json` 仅剥离 code fence。 |
| 结构化失败分类 | `UPSTREAM_ONLY` | 上游 `compatibility/adapter.ts::implicatesStructuredMode` 明确把 401/403/429、限流、网络、取消、超时、预算耗尽排除在"不支持该模式"之外；Fork 无此概念。**这是最值得移植的工程思想之一。** |
| 思考/推理参数 | `BOTH_DIFFERENT` | 上游有 5 种控制方式（`thinking`/`thinking_boolean`/`enable_thinking`/`reasoning_effort`/`none`）与 4 种 reasoning 字段探测；Fork 仅在用户显式选择时透传 `reasoning_effort`（`provider.rs`）。 |
| Prompt 管理 | `BOTH_DIFFERENT` | 上游 Prompt Registry（TS，`server/src/prompting`）；Fork `novel-prompt` 单文件 `PromptAsset` 表，版本化但无 contextPolicy/outputSchema。路线不同，**不合并**。 |
| Token 与费用 | `BOTH_DIFFERENT` | 上游按物理调用记录 + 压缩 payload 存储；Fork 按 step 记录 → 本轮已修正为按物理调用。 |
| 调用观测 | `UPSTREAM_ONLY` → **本轮已同步** | 上游 `0026d0cd`：链式 attempt、首次/修复独立、payload 留存策略。Fork 原实现把修复用量合并进首次请求、失败请求不落库。见第 6 节。 |
| Director/Agent 编排 | `BOTH_DIFFERENT` | 上游 `productionKernel`（阶段 + 检查点 + 未知结果确认）；Fork `novel-workflow` plan + `worker`。语义接近，实现不可复用。 |
| 暂停/恢复/幂等 | `ALREADY_EQUIVALENT` | Fork `RecoveryDecision::HoldInterrupted`（`novel-workflow/src/recover.rs`）已实现"结果未知必须人工确认"，与上游 `93a1c914` 同结论；Fork 还额外有 `AttemptPhase` 与租约栅栏。 |
| 章节规划与正文生成 | `BOTH_DIFFERENT` | 上游 production kernel + 短篇参考；Fork `production_plan` v1/v2/v3 + `chapter-chain-v1`，硬义务（hard obligations）机制更严格。 |
| Reviewer 与质量债 | `ALREADY_EQUIVALENT` | Fork 已有 `chapter_quality_debt`、`structured_failure` 产物、审校证据可定位校验（`obligations::validate_review_evidence`）。上游 `0e52138e`/`9facdb62` 的问题 Fork 已用别的机制解决。 |
| 人物/世界观/伏笔/记忆 | `BOTH_DIFFERENT` | 上游结构化记忆 + 短篇参考；Fork 有 `structured_memory`、`knowledge`、`plot`、`continuity` 独立模块。功能重叠，不建议互换。 |
| 章节编辑器 | `BOTH_DIFFERENT` | 上游 `5ca31f93` 重写 `ChapterTextEditor.tsx`（+759 行）、新增 Ghost Text 与 `editorDocument`。Fork 编辑器已有自己的 document/draftState 模型。**只应吸收交互设计，不能照搬。** |
| 桌面 UI 与运行时 | `FORK_ONLY` | 上游 `desktop/` 仅 3 个文件变更；Fork 已完成 Tauri 2 迁移（`src-tauri` + `client/src/platform/desktopTransport.ts`）。 |
| 数据持久化 | `INCOMPATIBLE` | 上游 Prisma/PostgreSQL+SQLite 双 schema；Fork 手写 SQL + `SqliteKernel`。上游 migration 不能执行。 |
| 安全与权限 | `ALREADY_EQUIVALENT` | Fork 已有端点鉴权（`authorize_endpoint`）、`redact_secrets`、`OsSecretStore`、credential intent。上游 `redactInvocationPayload` 的字段白名单思想可借鉴，但核心能力已有。 |
| 构建/发布/CI | `FORK_ONLY` | Fork 有独立发布链路与 `v0.1.1-beta.6` 版本；上游 `release/desktop-v0.5.22` 等分支与 Fork 无关。 |
| 渠道通知（飞书/钉钉） | `S3` | 上游 `2681d30a`/`ebee8eef`。与当前 Beta 验收无关，且会引入外部回调面。 |
| 官网动画 | `S3` | 上游 `9e0276b4`/`432cb0d0`（`site/`）。与产品无关。 |

## 5. 第二阶段：模型兼容能力专项回答

对照上游 `44e0a335`、`20650f07`、`7c94bb2a`、`9a68e37f`，逐条回答任务提出的十个问题。

| # | 问题 | 结论 |
| --- | --- | --- |
| 1 | Fork 是否支持 json_schema / json_object / tool calling？ | **都不支持。** `ProviderCapabilities` 把 `structured_output`/`tool_call` 固定为 `false`，请求体只有 `model/stream/max_tokens/messages`（`novel-llm/src/provider.rs`）。全部结构化步骤依赖提示约束 + 一次修复。 |
| 2 | 哪些 Provider 能力由静态配置决定？ | 全部。`ProviderCapabilities`（streaming/structured_output/tool_call/reasoning/context_limit）与 `catalog.rs` 中 `supports_reasoning` 是常量表，与端点无关。 |
| 3 | 是否具备端点级能力隔离？ | **没有。** 上游用 `[provider, model, host]` 三元组做 `identityKey`；Fork 只有 `provider`，同一模型名经不同网关会共享同一份静态判断。 |
| 4 | 结构化失败时是否有安全降级？ | 有，但只有一级：解析失败 → 一次修复 → 仍失败则 `DataIntegrity` 暂停并存 `structured_failure` 产物。没有模式级降级。 |
| 5 | 网络/限流/鉴权失败是否被误判为"不支持结构化"？ | **Fork 不存在该误判**，因为它根本没有探测结论可被污染。这正是上游 `implicatesStructuredMode` 要解决的问题——若未来引入探测，必须同时引入该分类，否则会把 429/超时写成"不支持"。 |
| 6 | 同一网关同名模型是否会错误共享能力？ | 会。当前静态表按 provider 归类，`https://gw-a/v1` 与 `https://gw-b/v1` 上的 `deepseek-chat` 无法区分。 |
| 7 | 是否存在思考参数不兼容？ | 存在风险。Fork 只在用户显式选择时发送 `reasoning_effort`，默认不发送，因此不会硬失败；但也不具备上游"禁用思考"的验证能力。 |
| 8 | 自动探测是否可能产生未授权付费调用？ | 上游已用 `allowNetwork` 显式开关 + 默认关闭的 autoProbe 约束。Fork 目前无探测，无此风险；**未来引入时必须沿用"默认不发真实请求"**。 |
| 9 | 能否在不改变旧 Run 冻结参数的情况下应用新能力？ | 可以。上游结论是"能力档案的失败模式属于调用观察，不进入配置哈希"——一次原生模式拒绝只更新观察，不改变已冻结的调用参数；若观察会改变实际参数，围栏仍须拒绝。Fork 的 `RunContract` 冻结模型/端点/预算，同样适用：能力只影响请求构造，不参与合同哈希。 |
| 10 | 是否会增加重复请求与计费风险？ | 若按上游顺序逐个模式尝试，最坏情况一次步骤会发出 4 次请求。因此移植时必须：**成功即停**、按端点缓存成功顺序、并把每次尝试独立记账（本轮已具备该基础）。 |

**判定：`MODEL_COMPATIBILITY: DEFER`**。理由：Fork 目前没有原生结构化模式，引入能力档案的前提是先实现模式本身。按正确顺序应先在 `novel-llm` 内实现 `json_object`（所有 OpenAI 兼容网关支持面最广），再以最小档案（provider+model+host → 模式序列 + 失败模式）驱动，最后才做设置页。一次性搬入 profile+probe+store+UI 风险过高，且探测会产生真实付费请求。

## 6. 第三阶段：LLM 调用观测专项

### 6.1 上游设计要点（`0026d0cd` / `674ad098`）

- `observation.ts` 用 `AsyncLocalStorage` 维护调用链：首次请求与 JSON 修复各自拥有 parse scope，修复是**独立的物理请求**。
- `InvocationPayloadStore` 对 payload gzip 压缩，**默认关闭**（`enabled: false`），保留 7 天 / 100MB 双上限，超限按最旧优先删除；只删 payload，保留 usage 行与正文。
- 脱敏是**字段白名单 + 正则**双保险（`apiKey`/`authorization`/`token`/`secret`/`headers` → `[REDACTED]`，再对 `Bearer`/`sk-` 做文本替换）。

### 6.2 Fork 实际缺陷（本轮修复的正是这两处）

| 缺陷 | 位置 | 后果 |
| --- | --- | --- |
| 结构化修复调用的用量被合并进首次请求 | `worker.rs` 修复分支 `accumulate_usage(&mut usage, &repair_response.usage)` | 修复流量在运行记录中完全不可见；`usage.calls` 恒为 1，无法据此判断真实请求数。 |
| Provider 失败（含 `billed: true`）完全不落库 | `worker.rs` 三个生成入口的 `Err(error) =>` 分支 | 连接中断/首字节超时/5xx 这类**可能已计费**的请求没有任何记录，成本静默消失。 |

### 6.3 本轮实现

新增 `migrations/0028_invocation_observability.sql`（纯增量）：

```sql
ALTER TABLE llm_invocation ADD COLUMN attempt_id TEXT;
ALTER TABLE llm_invocation ADD COLUMN audit_status TEXT NOT NULL DEFAULT 'succeeded';
ALTER TABLE llm_invocation ADD COLUMN usage_state TEXT NOT NULL DEFAULT 'known';
ALTER TABLE llm_invocation ADD COLUMN duration_ms INTEGER;
CREATE INDEX ... ON llm_invocation(run_id, started_at);
```

`novel-domain/src/observability.rs` 定义稳定契约：

- `InvocationOutcome`：`succeeded / repaired / repair_failed / unauthorized / rate_limited / timed_out / cancelled / provider_failed / rejected`
- `InvocationUsageState`：`known / unknown / unbilled`

关键语义（本轮裁决）：

| 结果 | 用量状态 | 依据 |
| --- | --- | --- |
| 成功 / 修复成功 | `known` | Provider 返回了用量 |
| 超时 / 连接中断 / 5xx | `unknown` | 请求可能已到达服务端 |
| 修复请求失败 | `unknown` | 首次请求可能已计费 |
| **取消** | `unknown` | 取消可能发生在请求已发出之后，**不能断言未计费** |
| 鉴权失败 / 限流 / 发出前被拒 | `unbilled` | 请求未被受理 |

> 这里推翻了代码中一处既有注释的假设（"取消路径不写记录"）。理由：`LlmError::Cancelled` 只在客户端侧产生，无法证明上游没有收到并计费；把它归为 `unbilled` 会让费用汇总系统性偏低。归类逻辑是确定性的错误分类，不属于 AI 意图识别范畴（AGENTS.md 允许"对已结构化输入的确定性后处理"）。

`RunUsageView` 扩展为 `calls / known_calls / unknown_calls / unbilled_calls / prompt_tokens / completion_tokens`，`prompt_tokens` 只累加 `known` 行，未知调用不再被当成 0 混入。运行详情页新增"`N 次调用费用未知`"提示。

### 6.4 明确未做的部分（有意的边界）

- **不持久化 prompt / response 全文**。上游默认关闭且有压缩与留存策略；Fork 已有 `input_summary`（80 字符 + 脱敏）与 `structured_failure` 诊断产物，足够定位问题，且避免 SQLite 无界增长与密钥泄漏面。
- **不引入上游 Prisma 模型**。
- **不做首 token 延迟**。当前 SSE 读取路径未记录首字节时间戳；这是下一步观测增强的独立项。

**判定：`LLM_OBSERVABILITY: SYNC`（本轮已完成最小闭环）**。

## 7. 第四阶段：工作流恢复能力对照

上游 `93a1c914` 修复五类恢复冲突：请求摘要别名、RAG 禁用配置比对、验收二次校验、预算估算漂移、结果未知步骤的显式确认解除。

逐项对照 Fork：

| 上游问题 | Fork 现状 | 判定 |
| --- | --- | --- |
| 结果未知必须人工确认才能继续 | `RecoveryDecision::HoldInterrupted`，且 `note_lease` 拒绝对 `WaitingApproval`/`Paused` 领取 | **已等效** |
| 预算估算漂移导致误冲突 | `RunContract` 冻结 `token_budget`，`repair_output_budget` 只在下界提升、不改合同 | **已等效（更强）** |
| 验收二次校验冲突 | Fork 用 `content_sha256` 校验候选正文一致性，不一致即 `DataIntegrity` 暂停 | **已等效** |
| 已确认候选被后续 Run 覆盖 | `PromoteOutcome` + `protected_user_content` + 候选表按 run 隔离 | **已等效** |
| 多进程租约/取消 | `LeaseFence` + `verify_lease_fence` + 每 run `CancellationToken` | **已等效** |
| 质量债务阻塞无关章节 | AGENTS.md 已有明确的非阻塞策略与 `defer_and_continue` 口径 | **已等效** |

本轮**未修改任何状态机**——没有证据表明 Rust 实现存在对应缺陷。上游这三个提交的价值在于**故障场景清单**：`preparationResumeCandidateDigest`、`selectedAssessmentUnboundVerification`、`productionKernelRagDisabledResume`、`productionKernelBudgetSqlite` 四个测试用例值得改写成 Fork fixture（见 Backlog）。

**判定：`WORKFLOW_RECOVERY: ALREADY_EQUIVALENT`**。

## 8. 第五阶段：编辑器 UX

上游 `5ca31f93`（+1168/−69）重写 `ChapterTextEditor.tsx`（+759）、新增 `document/editorDocument.ts`、`document/ghostText.ts` 与其测试；`2c980d4d` 在导演阶段旁显示卷拆章进度。

Fork 对应：`client/src/pages/novels/components/chapterEditor/` 已有自己的 `document/draftState`、`editPosition`、`navigation/chapterNavigatorModel`、`rightPanel/panelModel` 及其测试，说明编辑器文档模型早已独立演进。

可吸收的是**交互结论**而非代码：

1. 光标/滚动位置在章节切换后保持；
2. 未保存修改的离开守卫（Fork 已有 `chapter leave guard`，P17.1 引入）；
3. Ghost Text 的触发与提交语义；
4. 卷拆章进度显示在导演阶段旁（`2c980d4d` 的做法）。

因 Fork 已转为 Tauri IPC，上游实现强依赖原服务端 API，**不能照搬**。

**判定：`EDITOR_UX: DEFER`**。这是 Beta 验收后的体验增强项，不进入本轮。

## 9. 第六阶段：同步清单

### S0 — 必须同步（本轮）

| 项 | 上游提交 | 缺口 | 受益 | 风险 | 测试 | 工作量 |
| --- | --- | --- | --- | --- | --- | --- |
| 物理调用独立记账 + 未知用量可见 | `0026d0cd`、`674ad098` | 修复调用不可见；失败请求零记录 | 费用可追溯；未知不再伪装为 0 | 低（纯增量列） | 6 个新测试 + 1 个迁移测试 | 已完成 |

### S1 — 优先同步（Backlog，按序）

| 项 | 上游提交 | Fork 对应 | 需要做什么 | 风险 |
| --- | --- | --- | --- | --- |
| 结构化失败分类 | `44e0a335`（`adapter.ts`） | 无 | 在 `novel-llm` 加 `classify_structured_failure`，明确 401/403/429/network/timeout/budget 不算"模式不支持"。**不产生任何网络请求。** | 低 |
| 首 token 延迟与逐调用耗时 | `0026d0cd` | 已有 `duration_ms`，缺首字节 | 在 SSE 首次收到数据时记录时间戳 | 低 |
| `max_tokens` 优先级（路由/显式值覆盖安全下限） | `9a68e37f` | `requested_tokens` 已按 contract 传入 | Fork 已有等效保护（`absolute_max_output_tokens`），**无需动作**；仅借鉴其"route > 8k clamp"的优先级断言为测试 | 无 |
| 原生 `json_object` 模式 | `44e0a335`、`20650f07` | 无 | 在 `novel-llm` 为 OpenAI 兼容通道加 `response_format`，成功后按端点缓存 | 中（网关差异） |
| 模型能力档案（最小版） | `44e0a335` | 静态 `ProviderCapabilities` | provider+model+host → 模式序列 + 失败模式 + 来源；无 UI、无探测 | 中 |
| 上游恢复故障 fixture | `93a1c914`、`71b4fd6b`、`d7fbd2c1` | 语义已等效 | 把 4 个上游测试场景改写为 `novel-workflow`/`novel-storage-sqlite` 用例 | 低 |

### S2 — 择机同步

- 编辑器交互（光标保持、Ghost Text、卷拆章进度）——`5ca31f93`、`2c980d4d`。
- 章节诊断给出具体修复目标与保留项——`579ba451`。
- 短篇按目标篇幅生成与逐段恢复——`41b4130f`、`f9072e55`（需先确认 Fork 是否规划短篇模式）。

### S3 — 暂不同步

- 飞书/钉钉/企业微信通知——`2681d30a`、`ebee8eef`。
- 官网首屏动画——`9e0276b4`、`432cb0d0`。
- 上游 production kernel 全量实现——`c5ac06d1`、`4e9472a0`、`26767919`、`72a68d6f`、`1307a3df`、`f9072e55`。
- 上游 Prisma migration 与 `schema.sqlite.prisma`。
- 探测 UI 与自动探测——`44e0a335`（会产生真实付费请求，需另行授权）。

## 10. 不应移植的重复功能

| 上游能力 | Fork 已有实现 | 结论 |
| --- | --- | --- |
| 导演恢复的未知结果确认 | `RecoveryDecision::HoldInterrupted` | 重复，Fork 更严格（带租约栅栏） |
| 预算漂移防护 | `RunContract.token_budget` 冻结 + `repair_output_budget` 下界 | 重复 |
| 候选覆盖保护 | `content_sha256` + `protected_user_content` | 重复 |
| 质量债非阻塞 | AGENTS.md 策略 + `defer_and_continue` | 重复 |
| 调用记录基础表 | `llm_invocation` + `usage_ledger` | 重复（本轮只补齐语义列） |
| 结构化失败诊断产物 | `structured_failure` artifact（含 firstRaw/repairRaw） | 重复，Fork 已保存原文 |

## 11. 本轮实际代码修改

基线：`origin/beta`（`0a51abc0`），独立分支 `feat/upstream-sync-a1-invocation-observability`，独立 worktree，未触碰 Beta-A3 工作区与 `main`。

| 文件 | 变更 |
| --- | --- |
| `migrations/0028_invocation_observability.sql` | 新增（纯增量 4 列 + 1 索引） |
| `crates/novel-domain/src/observability.rs` | 新增：`InvocationOutcome`、`InvocationUsageState` 及其语义与单元测试 |
| `crates/novel-domain/src/lib.rs` | 导出新类型 |
| `crates/novel-application/src/ports.rs` | `NewInvocation` 增加 `attempt_id`/`outcome`/`usage_state` |
| `crates/novel-application/src/worker.rs` | 首次请求与修复请求分别记账；失败/取消路径补记；新增 `record_failed_invocation`、`record_repair_invocation`、`invocation_outcome_for_error` |
| `crates/novel-application/src/dto.rs` | `RunUsageView` 增加 `known_calls`/`unknown_calls`/`unbilled_calls` |
| `crates/novel-application/src/director.rs` | 投影透传新字段 |
| `crates/novel-storage-sqlite/src/db.rs` | 注册 0028 |
| `crates/novel-storage-sqlite/src/workflow.rs` | 插入新列；`run_usage` 按 `usage_state` 分组聚合 |
| `crates/novel-application/tests/sync_a1_invocation_observability.rs` | 新增 6 个测试 |
| `crates/novel-storage-sqlite/tests/invocation_observability_migration.rs` | 新增历史库升级测试 |
| `crates/novel-application/tests/p8_director_loop.rs` | 4 个测试的记账契约从"合并为 1 条"更新为"物理调用 2 条" |
| `crates/novel-storage-sqlite/tests/workflow_lease.rs` | 构造函数补新字段 |
| `client/src/platform/desktopTransport.ts` | `RunUsageView` 类型补新字段 |
| `client/src/features/directorNext/TauriDirectorPage.tsx` | 未知费用提示 |

新增测试覆盖：修复调用独立记账、未知计费失败留痕、鉴权/取消不伪造计费、聚合分离 known/unknown、取消后不再调度、旧行兼容、历史库升级幂等。

## 12. 测试与 CI 结果

| 检查 | 结果 |
| --- | --- |
| `cargo fmt --all` | PASS |
| `cargo clippy --workspace --all-targets` | PASS（新增代码 0 warning） |
| `cargo test --workspace` | PASS，101 个测试二进制，0 失败 |
| `novel-domain`（含新契约单测） | PASS |
| `novel-application`（含 6 个新测试） | PASS |
| `novel-application` p8_director_loop（10 个） | PASS |
| `novel-workflow` | PASS |
| `novel-storage-sqlite`（含迁移升级测试） | PASS |
| `novel-desktop`（src-tauri 编译） | PASS（workspace 内构建） |
| Client typecheck | **部分**：本 worktree 为符号链接依赖环境，全量 typecheck 报 119 个**既有**错误（`main` 未改动基线相同）；改动前 119、改动后 119，`TauriDirectorPage.tsx` 与 `desktopTransport.ts` **零新增错误** |
| Client 单测 | NOT_RUN（Node 测试需要完整 workspace 依赖图，未在隔离 worktree 中安装） |
| Tauri macOS/Linux bundle | NOT_RUN（本轮未改打包配置） |
| Native E2E | NOT_RUN（UI 验收按 AGENTS.md 交由用户） |
| `git diff --check` | 见下方验证记录 |

重点验证项：旧 Run 恢复（未改状态机，全套 recovery 测试通过）、结构化修复（p8_director_loop 全绿）、Provider 失败与取消（新增 3 个用例）、请求预算（未改）、物理调用计费（新增 6 个用例 + 迁移用例）、M1 Mock fixture（未改）。

## 13. 同步后的潜在冲突

1. **Beta-A3 工作区**：本轮分支基于 `origin/beta`，Beta-A3 在其上还有 4 个提交（含 reviewer 与桌面验收改动）。`worker.rs` 是双方都可能触碰的热点文件，合并时需按 hunk 复核。
2. **`RunUsageView` 是 IPC 契约**：新增字段向后兼容（旧客户端忽略未知字段），但若有测试对 `usage` 做严格对象比较需要同步更新——本轮已更新 `p8_director_loop` 与 `workflow_lease`。
3. **迁移编号**：0028 之后若有并行分支新增迁移，需按序重新编号。
4. **`usage.calls` 语义变化**：从"步骤数"变为"物理调用数"，含修复与失败请求。若后续有报表按 `calls` 推断步骤数，需要同时看 `step_id`。

## 14. 最终裁决

```
UPSTREAM_AUDIT:        COMPLETE
FULL_MERGE:            NOT_RECOMMENDED
MODEL_COMPATIBILITY:   DEFER
LLM_OBSERVABILITY:     SYNC（本轮已完成最小闭环）
WORKFLOW_RECOVERY:     ALREADY_EQUIVALENT
EDITOR_UX:             DEFER
CODE_MIGRATION:        DONE
PR:                    Draft PR（见提交说明）
BETA_A3_INTERFERENCE:  NONE（独立 worktree + 独立分支，未触碰 Beta-A3 与 main）
```

## 15. 七问七答

**1. 上游 118 个独有提交中，究竟哪些能力值得我们引入？**

三项工程契约与一批测试场景：物理调用独立记账（`0026d0cd`，本轮已做）、结构化输出失败分类（`44e0a335` 的 `implicatesStructuredMode`）、端点级能力档案（`44e0a335` 的 `identityKey`）。此外 `9a68e37f` 的 `max_tokens` 优先级结论、`93a1c914` 的四类恢复冲突场景，可作为测试输入。

**2. 哪些功能 Fork 已经有更好的等效实现？**

工作流恢复（`HoldInterrupted` + 租约栅栏，比上游更严格）、请求预算冻结、候选覆盖保护、质量债非阻塞策略、结构化失败诊断产物（Fork 已保存首次与修复的原始输出）。

**3. 有没有当前必须立即修复的上游安全或稳定性问题？**

没有"必须从上游修"的项。但审计中发现 Fork **自身**的两个计费可追溯缺陷（修复调用不记账、失败调用零记录），已在本轮修复——它们属于费用静默丢失，等级为 S0。

**4. 哪些功能需要从 TypeScript 移植到 Rust？**

只有三项，且都是契约而非实现：`ProviderCapabilityProfile`（含 `identityKey` 端点隔离）、结构化模式降级序列、失败原因分类。三者都应在 `novel-llm` 内重写，不复用上游代码。

**5. 哪些功能只需要吸收测试案例或设计思想？**

恢复冲突的四类场景、探测必须显式授权且默认不发真实请求、能力档案失败模式不进配置哈希、payload 留存策略（默认关闭 + 双上限 + 字段白名单脱敏）、思考开关"不能只凭 HTTP 200 判定"的验证原则。

**6. 上游同步对目前 Beta 发布的实际价值有多大？**

中等偏低但方向明确。118 个提交里 96 个只动文档，32 个动前端（多为上游自有 UI 结构），15 个动 `server/src/llm`。对 Beta 验收真正有直接帮助的是记账与失败分类这两项——前者本轮已落地，后者可减少"结构化失败被误判为模型不支持"导致的无效排障。渠道通知、官网、production kernel 与本 Beta 无关。

**7. 下一次同步应该安排在什么阶段？**

建议在 Beta-A3 合入 `beta` 并通过验收之后、下一次大版本规划之前，安排一次 Upstream-Sync-A2，范围限定为 S1 前三项：失败分类 → 首 token 延迟 → 原生 `json_object` + 最小能力档案。能力档案必须与"默认不探测、探测显式授权"一起设计，避免引入未授权付费调用。

---

## 附：Backlog 摘要

| 优先级 | 项目 | 依赖 | 预估 |
| --- | --- | --- | --- |
| S1-1 | `novel-llm` 结构化失败分类（无网络请求） | 无 | 小 |
| S1-2 | 首 token 延迟记录 | 无 | 小 |
| S1-3 | OpenAI 兼容 `json_object` 模式 | S1-1 | 中 |
| S1-4 | 最小能力档案（provider+model+host） | S1-3 | 中 |
| S1-5 | 上游恢复故障 fixture 改写 | 无 | 小 |
| S2-1 | 编辑器光标保持 / Ghost Text / 卷拆章进度 | Beta 验收后 | 中 |
| S2-2 | 章节诊断给出具体修复目标 | 无 | 小 |
| S3 | 通知渠道、官网、production kernel | 暂不 | — |
