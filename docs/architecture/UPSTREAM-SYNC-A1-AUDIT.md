# 上游同步审计 A1：差异盘点与选择性同步

本文件记录 `AI-Novel-Writing-Assistant` fork 与上游 `ExplosiveCoderflome/AI-Novel-Writing-Assistant` 的一次全量差异审计，以及本轮实际落地的移植。

审计立场：**不因为上游更新就默认上游更好。** fork 已在 Rust/Tauri 内核上重写了创作主链路，上游的 Node/Prisma 实现多数不适用；本文件区分「值得移植的能力」「只需要吸收的测试与设计思路」「不应移植的实现」。

## 1. 基线

审计开始前重新 `fetch --all --prune`，以下 SHA 为重新计算后的实际值（与任务书给出的起点一致）。

| 项 | SHA | 说明 |
|---|---|---|
| `upstream/main` | `ebee8eef2287864b30be826fb2f1a39eef757d44` | 上游唯一活跃分支 |
| `upstream/beta` | `ebee8eef` | 与 upstream/main 相同 |
| `origin/main` | `eff69f229727ab90f4102c05cfc705116cb1ded5` | fork 主线 |
| `origin/beta` | `0a51abc06515866ea7197e8ef4fa8b3c07d6c388` | fork 当前研发基线 |
| merge-base | `5cd6a1324eeabb3990a872ad955eec20f75ecd3d` | 2026-10-11 |

双向提交数量：

```
upstream/main 独有   118
fork/beta 独有       372
fork/main 独有       244
```

## 2. 完整差异文件统计

`git diff --name-status origin/beta upstream/main` 的完整结果（不是 GitHub 界面截断的前 300 条）：

```
总计 2224 个文件
  A（新增）  621
  D（删除） 1012
  M（修改）  591
```

按顶层目录：

| 目录 | 文件数 | 解读 |
|---|---|---|
| `server/src` | 578 | 上游 Express/Prisma 实现，fork 未采用 |
| `server/tests` | 259 | 上游 Node 测试 |
| `client/src` | 232 | React 前端，与 fork 同源但有分叉 |
| `src-tauri/permissions` | 136 | 上游桌面壳权限清单 |
| `docs/tauri-migration` | 120 | 上游迁移文档 |
| `crates/*` | 约 200 | **fork 独有**的 Rust 内核 |
| `backups/` | 54 | fork 侧的历史备份目录 |
| `tests/desktop-e2e` | 53 | E2E |

其中 1012 个 `D` 绝大多数是 fork 主动删除的上游 `server/` 与 `docs/` 文件——因为 fork 用 Rust 重写了同一批能力。

## 3. 关键结构性事实

这一条决定了后面所有判断：

fork 存在**两套 LLM 实现**，但桌面产品只跑 Rust 那套。

- **Rust 内核（产品路径）**：`src-tauri/src/lib.rs:29-32` 的 `llm_provider()` 直接返回 `ProtocolRouterProvider::new()`。Tauri 进程是纯 Rust，`tauri.conf.json` 无 sidecar。
- **Node server（遗留副本）**：`server/src/llm/**` 仍在 `origin/beta` 中，`Dockerfile.api` 仍构建它作为 Web API；但桌面端不启动它。`client/src/platform/desktopApiAdapter.ts:496-497` 把 `/llm/model-routes` 等路径全部截到 Tauri command。

因此「fork 是否支持 X」必须分别回答。本轮所有结论以 **Rust 内核**为准。

## 4. 功能领域差异矩阵

标注含义：`UPSTREAM_ONLY` 上游有 fork 无；`FORK_ONLY` fork 有上游无；`BOTH_DIFFERENT` 双方都有但路线不同；`ALREADY_EQUIVALENT` 已等效；`INCOMPATIBLE` 架构冲突不宜引入。

| 领域 | 判定 | 说明 |
|---|---|---|
| **模型 Provider 与路由** | BOTH_DIFFERENT | fork 有三态能力字段与四层来源优先级（更严格）；上游有端点级 profile |
| **结构化输出与修复** | UPSTREAM_ONLY | fork 只会「prompt 要求 JSON + 一次修复」；上游有 json_schema / json_object / tool_calling |
| **Prompt 管理** | BOTH_DIFFERENT | fork 用 `crates/novel-prompt` 版本化资产；上游用模板工作台 |
| **Token 与费用** | BOTH_DIFFERENT | fork 用 `RunContract.token_budget` 冻结；上游用 8192 安全钳制 |
| **Director/Agent 编排** | BOTH_DIFFERENT | fork 用版本化 `production_plan`；上游用 `productionKernel`（52 文件） |
| **工作流暂停、恢复与幂等** | FORK_ONLY（更强） | fork 有 9 种 `StopSignalKind`、attempt 状态机、`OpenRunHash` 13 字段 |
| **章节规划和正文生成** | BOTH_DIFFERENT | 各自实现 |
| **Reviewer 与 Quality Debt** | ALREADY_EQUIVALENT | fork 有 `quality_debt` 表与 fail-closed 审校 |
| **人物、世界观、伏笔与记忆** | BOTH_DIFFERENT | fork 在 `novel-application/memory.rs` 重新实现 |
| **章节编辑器** | BOTH_DIFFERENT | fork 工作台结构更丰富（导航器/右栏/版本抽屉）；上游编辑态保持更成熟（见 §9） |
| **桌面 UI 与运行时** | FORK_ONLY | fork 是 Tauri，上游是 Electron + Express |
| **数据持久化** | INCOMPATIBLE | fork 用 SQLite + sqlx 迁移；上游用 Prisma |
| **安全与权限** | 见 §6 | fork 用 ChaCha20Poly1305 + keyring；**但仓库历史泄露了真实密钥** |
| **构建、发布与 CI** | FORK_ONLY | Tauri 打包链路 |
| **渠道通知（飞书/钉钉）** | S3 暂不同步 | 与 beta 验收无关 |
| **官网动画** | S3 暂不同步 | 产品无关 |

## 5. 模型兼容性与结构化输出

### 5.1 fork 现状

**能力声明是死的。** `ProviderCapabilities.structured_output` 与 `tool_call` 在三个实现里全部硬编码为 `false`：

- `crates/novel-llm/src/provider.rs:86-94`（`UnconfiguredProvider`）
- `crates/novel-llm/src/provider.rs:161-169`（`OpenAiCompatibleProvider`）
- `crates/novel-llm/src/provider.rs:41-49`（`OllamaProvider`）

而且 `capabilities()` 在生成路径上**从未被读取**——`grep` 全仓只有 `crates/novel-llm/src/router.rs:45-47` 自身转发。

**从不发送 `response_format`。** 请求体固定在 `crates/novel-llm/src/provider.rs:221-226`：

```rust
let mut body = serde_json::json!({
    "model": request.model,
    "stream": true,
    "max_tokens": requested_tokens,
    "messages": [{"role": "user", "content": request.prompt}]
});
```

结构化完全依赖「prompt 要求严格 JSON → `serde_json::from_str` → 失败则修复一次」。

### 5.2 上游设计（值得吸收的部分）

上游 `44e0a335` 把静态表升级为「内置默认 + 探测 + 用户覆盖」三层，核心在 `server/src/llm/compatibility/`：

- **`profile.ts:16-18`** —— 身份键是 `[provider, model, modelHost(baseURL)]`。README 明确写「存储身份保留完整模型名，防止网关别名串用结果」。
- **`defaults.ts:1-47`** —— 按 provider 家族给出 `structuredModes`，并区分「直连」与「网关」：例如 DeepSeek 只有在 `api.deepseek.com` 直连时才给 `json_object`。
- **`probe.ts:78-88`** —— 失败分类的关键安全边界：**401/403/429、网络故障、5xx 一律不算作「不支持结构化模式」**，只把 400/422 且明确提及 `response_format` 的拒绝记入 `failedModes`。
- **`probe.ts:32-33`** —— 探测必须显式 `allowNetwork`，启动应用不探测。

### 5.3 fork 的反向优势（不应回退）

- **三态能力**：`crates/novel-domain/src/provider.rs:103-114` 用 `None` 表示「未知」，不允许把未知解释为支持或不支持。上游没有这个区分。
- **冻结路由**：`RunContract` 存 provider/model/endpoint/protocol/credential_ref/reasoning_effort 并参与请求哈希（`crates/novel-application/src/dto.rs:375-408`、`director.rs:572-587`）。上游的备用模型（`structuredFallbackSettings`）会绕过冻结路由，与 fork 的安全模型冲突。

### 5.4 本轮修正的一个真实缺陷

`crates/novel-application/src/worker/worker_validation.rs:342` 此前直接：

```rust
serde_json::from_str(raw)
```

模型把合法 JSON 包进 Markdown 围栏（非常常见）时，整段解析失败，随即触发**唯一一次**有界修复调用——输出本身是对的，费用却白花。

本轮改为先提取第一个完整 JSON 值再校验（见 §9）。**只移植上游 `extractJSONValue` 的「提取」思路，不移植 `tryFixTruncatedJson`**——补齐截断后当成成功会让半截计划通过校验。

### 5.5 一个被误判为缺陷的设计

`crates/novel-application/src/worker.rs:2296-2302` 的修复预算会把合同预算抬到 16 384：

```rust
fn repair_output_budget(contract_budget: Option<u64>) -> u64 {
    contract_budget.unwrap_or(DEFAULT_OUTPUT_RESERVE).max(REPAIR_MIN_OUTPUT_TOKENS)
}
```

初看像是「越过冻结参数」，但注释与测试（`worker.rs:2681-2682`）表明这是有意的：默认 8k 常被推理 token 吃光，导致候选被截断在中句，所以抬高下限并在触顶时 fail-closed。上游 `9a68e37f` 是反方向（往下压到 8k）。**判定为 FORK_ONLY 的更强行为，不改。**

## 6. 安全：仓库历史泄露了真实 API 密钥（S0）

**这是本次审计最重要的发现，与上游同步无关，但必须优先处理。**

fork 的 `origin/main` 与 `origin/beta` 中跟踪着 3 个 SQLite 备份文件：

```
server/tmp/db-backups/dev_20260313_1655_before_rebuild.db   9.7M
server/tmp/db-backups/dev_20260313_1705_replaced.db         9.7M
server/tmp/db-backups/dev_restore_backup_20260313_171051.db  668K
```

前两个文件的 `APIKey` 表各含 **3 条非空记录**，特征（只看长度与前缀，未打印完整值）：

| provider | 长度 | 前缀 |
|---|---|---|
| deepseek | 35 | `sk-a***` |
| siliconflow | 51 | `sk-x***` |
| grok | 84 | `xai-***` |

- 引入提交 `fb39b485`（2026-03-13），已暴露约 8 个月
- 仓库 `visibility=PUBLIC`
- `.gitignore` 第 16 行有 `server/tmp/**/*.db`，但这 3 个文件更早被强制加入，gitignore 对已跟踪文件无效
- **上游已修复**：`9ddbac86`「源码分发不再包含本地数据库备份」删掉了同批文件，但 fork 未同步

缓解事实：Rust 内核用 ChaCha20Poly1305 + 系统 keyring 加密密钥（`crates/novel-platform/src/secrets.rs:177-195`），不读取这些 db。泄露仅在仓库历史层面。

处置要求（需仓库所有者执行）：

1. 在 DeepSeek / SiliconFlow / xAI 控制台吊销并重签这 3 个密钥
2. 用 `git filter-repo` 清理历史；仅从最新提交删除不够——`git log` 仍可取到

## 7. LLM 调用观测

### 7.1 fork 已有

`migrations/0001_init.sql:178-192` 的 `llm_invocation` 表：

```sql
id, request_id (UNIQUE), run_id, step_id, provider, model,
input_summary, started_at, finished_at,
prompt_tokens, completion_tokens, estimated_cost_micros, status
```

- **每次物理请求一条记录**：`crates/novel-application/src/worker.rs:2498-2533` 的 `invocation()` 构造，`record_invocation` 落库
- **修复请求独立记账**：`bounded_repair` 有独立 `step_id`，失败路径也经 `record_usage_before_stop`（`worker.rs:2385-2401`）写入
- **未知 usage 保留**：`TokenUsage { prompt_tokens: Option<u64>, completion_tokens: Option<u64> }`（`provider.rs:23-28`），`None` 就是未知，不会被写成 0
- **调用耗时可直接算出**：`finished_at - started_at` 已存在，无需新字段

### 7.2 fork 缺失

| 能力 | 状态 | 依据 |
|---|---|---|
| 首 token 延迟（TTFT） | 缺 | 无字段，`sse.rs` 不记录首片时间 |
| 失败原因分类落库 | 缺 | `status` 是自由字符串 |
| 结构化模式记录 | 缺 | 无 `structuredMode` 字段 |
| 调用链（chainId / parentInvocationId） | 缺 | 无字段 |
| Prompt/Response 全文历史 | 缺 | 有意不存（见下） |

上游 `0026d0cd` 的 migration 新增 `durationMs`、`firstTokenMs`、`attemptNumber`、`chainId`、`parentInvocationId`、`structuredMode`，另有 `LlmInvocationPayload` 压缩载荷表。

### 7.3 判断

- `durationMs` 不需要照搬：fork 已有起止时间，展示层算即可
- 值得补：`firstTokenMs`、`structuredMode`、失败原因分类
- **不应照搬**：`LlmInvocationPayload` 全文持久化。它会带来无限增长与敏感信息留存风险，fork 的 `structured_failure` 产物只保存失败原始输出已是更克制的做法

## 8. 工作流恢复

上游相关提交：`93a1c914`、`71b4fd6b`、`d7fbd2c1`。

| 审查点 | fork 现状 | 判定 |
|---|---|---|
| 请求身份包含完整有效输入 | `OpenRunHash` 含 13 字段（`director.rs:572-587`） | FORK_ONLY（更严格） |
| 冻结参数可被静默改变 | `RunContract` + `FrozenStepRoute` 存 endpoint | 否 |
| 重试时预算漂移导致误冲突 | 哈希含 `token_budget`/`context_token_budget` | 已覆盖 |
| Provider 结果未知导致重复计费 | 失败即 pause，不自动重试；`LlmError::Timeout` 明确不可重试（`error.rs:26-40`） | FORK_ONLY（更严格） |
| 显式人工确认解除安全停点 | 9 种 `StopSignalKind`（`novel-domain/src/run.rs:66-76`） | 已具备 |
| 已确认候选被后续 Run 覆盖 | attempt 状态机 `Started→RemoteStarted→CandidateSaved→Committed/Interrupted` | 已覆盖 |
| 多进程租约、取消、恢复 | `workflow_lease` 测试覆盖 | 已具备 |
| 质量债务阻塞不相关章节 | `quality_debt` 按章节作用域 | 已具备 |

**结论：工作流恢复领域 ALREADY_EQUIVALENT 偏 FORK_ONLY。** 上游修复的多数问题，fork 因为采用「失败即停、不自动重试」的更强契约而不存在。无需移植，但上游的回归场景值得转成 fork 的测试 fixture（列入 Backlog）。

## 9. 编辑器与产品功能

这一领域不能用一个标签概括：**fork 在工作台结构上更丰富，上游在编辑态保持上更成熟。**

### 9.1 fork 明显更强（不要用上游覆盖）

| 能力 | fork 位置 | 上游 |
|---|---|---|
| 章节导航器（卷/章分组、搜索、上一/下一、Alt+←/→） | `chapterEditor/navigation/ChapterNavigator.tsx`（182 行） | 无 |
| 章节切换守卫（3 种原因 × 3 种去向分别措辞） | `navigation/chapterNavigatorModel.ts:286` `shouldBlockChapterSwitch` | 只有 `window.confirm` |
| 编辑位置恢复（按书隔离、版本/长度校验） | `document/editPosition.ts` | 无 |
| 右栏工作区（剧情、连贯性、Run 恢复） | `rightPanel/`（7 个组件，约 1800 行） | 无 |
| 版本抽屉、AI Diff 面板 | `ChapterVersionDrawer.tsx`、`AIDiffPanel.tsx` | 无 |

其余 `client/src` 与上游差异共 232 个文件，多数是 fork 侧新增。

### 9.2 fork 明显更弱（S1 候选）

**A1 —— 外部写回时整篇重建编辑器。**

`ChapterTextEditor.tsx:231-239`：只要 `value` 变化就 `setEditorSeed(n+1)`，`usePlateEditor` 因依赖变化重建整个实例。后果是 AI 候选采纳、载入历史版本、后台保存刷新之后，光标跳回开头、选区丢失、撤销历史清空。长篇创作里每次 AI 写回都付一次。

上游 `5ca31f93` 的对策是段落级增量补丁：`computeParagraphSpan` + `getPointFromAbsoluteOffset`（`document/editorDocument.ts:214,249`），只替换变化的段，`withNewBatch` 内还原选区。

**注意时序**：fork 有 `restorePosition` 机制（`ChapterTextEditor.tsx:252-283`），A1 落地必须保持「先文档、后光标」，否则光标恢复会失效。

**C3 —— `hasExternalChange` 只比时间戳。**

`document/useChapterDraft.ts:63`：

```ts
const hasExternalChange = incoming.updatedAt > draft.savedUpdatedAt
```

作者自己保存后刷新，也可能弹「本章已有新的保存版本」。上游 `66fc3894` 改为「时间更新 **且** 内容不同」。

**A5 / A3 / A7 / A8 —— 纯前端能力缺失**：章内查找替换、字号行距与专注模式、布局三态（standard/wide/immersive）、左右栏可收起。这些不依赖任何新接口，只用到 localStorage。

### 9.3 需要先补数据源（ADAPT）

| 项 | 阻塞点 |
|---|---|
| Ghost Text 续写建议 | 算法（`ghostText.ts` 56 行纯函数）可直搬，但桌面 `previewChapterCursorContinuation` 无 IPC 映射，只有「本地创作模式」可用。直接搬会得到一个「默认关闭且打开后无反应」的开关 |
| 诊断段落标记 | 桌面 `get_chapter_editor_workspace` 的 `diagnostic_cards` 恒为 `[]`（`src-tauri/src/commands/editor.rs`） |
| 质量提醒收进左栏 | 桌面未映射 `chapter.riskFlags` |
| 卷拆章进度 | fork 的 Rust 内核没有 `volume_chapter_list` 步骤产物 |
| 导演阅读页进正文编辑 | 桌面 `TauriDirectorPage` 无 `DashboardView` 工作区数据 |

### 9.4 落地顺序建议

1. **A1**（编辑态保持）—— 其他 UX 改动的前提；上游 A3 专注模式、A5 查找替换都依赖「实例不重建」
2. **C3**（`hasExternalChange` 一行修正）—— 成本最低、误报最烦人
3. **A2 + A7 + A8**（布局与浮动条定位）
4. Ghost Text 与诊断标记等数据源补齐后再做

### 9.5 明确排除

官网动画、飞书/钉钉/企业微信通知、远程导演确认、首页进度入口、ZIP/漫画导出，均与当前 beta 验收无关或桌面端无对应后端。

## 10. S0 / S1 / S2 / S3 分级

### S0 必须处理

| 项 | 来源 | 动作 |
|---|---|---|
| 仓库历史泄露 3 个真实 API 密钥 | fork 自身，上游 `9ddbac86` 已修 | **吊销密钥 + 清理历史**（需仓库所有者执行） |

### S1 优先同步

| 项 | 上游 | Fork 落点 | 理由 |
|---|---|---|---|
| 结构化输出提取围栏 JSON | `structuredInvokeParser.ts:60-111` | `worker_validation.rs` | **本轮已实现** |
| 端点级能力档案 | `compatibility/profile.ts` | `novel-domain/src/provider.rs` + migration | 后续原生结构化前置 |
| 失败分类携带 httpStatus | `adapter.ts:14-20` | `error.rs:43-54` | 能力档案的正确性依赖它 |
| 思考控制适配 + 思考字段采集 | `adapter.ts:2-12`、`reasoning.ts:97-111` | `provider.rs` + `sse.rs` | 推理模型当前返回「成功但空正文」 |
| 生产级请求预算 | `RequestBudget.ts:27-29` | `novel-llm` 或 `novel-application` | 多模式链的前置条件 |

### S2 择机同步

- 原生 `response_format` / `tool_calling` 传输（需先有 S1 的能力档案与预算）
- `firstTokenMs`、`structuredMode` 落库
- **A1 段落级补丁保持编辑态**（见 §9.2，实际优先级高于这里——列在 S2 是因为它属 UI 层，不阻塞后端同步）
- **C3 `hasExternalChange` 内容比对**（同上）
- 查找替换、字号行距与专注模式、布局三态、左右栏收起
- Ghost Text（需先补桌面续写 IPC）
- 卷拆章进度显示

### S3 暂不同步

- 渠道通知（飞书/钉钉/企业微信）
- 官网动画与手绘书稿首屏
- 远程导演确认
- `structuredFallbackSettings` 备用模型（与冻结路由冲突）
- 上游 `compatibility/invocation.ts` / `observation.ts`（Prisma + AsyncLocalStorage 台账）
- 任何 `server/` 下的 Node 实现

## 11. 本轮实际修改

单一改动，范围最小、收益明确、可独立验证：

**`crates/novel-application/src/worker/worker_validation.rs`**

新增 `extract_json_payload()`：在送进 `serde_json::from_str` 之前，提取第一个完整 JSON 值。字符串感知的括号配对（正确处理 `"` 与 `\\` 转义），未闭合时原样返回交给解析器报错。

```rust
let mut value: serde_json::Value = serde_json::from_str(extract_json_payload(raw))
    .map_err(|_| "模型输出不是合法 JSON".to_string())?;
```

**未改动 fail-closed 语义**：截断输出、纯散文、schema 不满足仍然照常失败。新增的两个 fail-closed 测试锁定这一点。

## 12. 测试与结果

```
cargo fmt --check                                  通过
cargo test -p novel-application --lib worker_validation::tests
                                                   18 passed（含 4 个新增）
cargo test -p novel-application                    245 passed + 全部集成测试通过
cargo test -p novel-llm                            66 passed
cargo test -p novel-workflow                       6 passed
cargo test -p novel-storage-sqlite                 9 passed
cargo test -p novel-domain                         20 passed
cargo clippy -p novel-application --lib -- -D warnings   通过
```

新增测试：

| 测试 | 目的 |
|---|---|
| `fenced_json_is_accepted_without_a_repair_round` | 三种围栏写法都能通过 |
| `text_around_the_fenced_json_does_not_break_validation` | 前后有散文也能提取 |
| `truncated_json_still_fails_closed` | 截断仍然失败 |
| `prose_without_json_still_fails_closed` | 纯散文仍然失败 |

**已知既有问题（非本改动引入）**：`cargo clippy -p novel-application --all-targets` 在 `crates/novel-application/src/context.rs:1051` 报 `clippy::bool-assert-comparison`。已用 `git stash` 验证该错误在干净的 `0a51abc0` 上同样存在。不在本轮范围内修改。

未运行：

| 检查 | 状态 |
|---|---|
| `cargo test -p novel-desktop` | NOT_RUN |
| Client typecheck / test | NOT_RUN |
| Tauri Linux/macOS Bundle | NOT_RUN |
| Native E2E | NOT_RUN |
| `git diff --check` | 通过 |

## 13. 同步后的潜在冲突

- 本轮改动只在 `novel-application` 的校验层，**不影响 Beta-A3 的 Reviewer 与 Provider 修复**（`fix/beta-a3-reviewer-and-desktop-acceptance` 分支未触碰）
- 后续若引入「能力档案 + 多模式链」，会与 `worker.rs` 的结构化分支（`:1529-1676`）产生交集。建议在 Beta-A3 合并后再做，避免与 Reviewer 修复重叠
- 引入请求预算会修改 `RunContract` 字段与 `OpenRunHash`——必须带 `#[serde(default)]` 以保证旧 Run 可反序列化

## 14. 下一轮建议

1. 先处理 S0 密钥泄露（需所有者操作）
2. 再做 S1 的 `error.rs` 失败分类携带 httpStatus —— 它是能力档案正确性的前提
3. 然后端点级能力档案（含 host 进身份键、改 baseURL 失效旧能力）
4. 最后才是原生结构化传输与请求预算

顺序理由：没有可靠的失败分类，能力档案会把瞬时故障记成「不支持」；没有请求预算，多模式链会把单步请求数放大 3–4 倍。

编辑器侧（可与后端同步并行，互不冲突）：

1. **A1 段落级补丁**——当前每次 AI 写回都重建编辑器实例，光标、选区、撤销历史全丢。这是长篇创作里最高频的摩擦
2. **C3 `hasExternalChange` 加内容比对**——一行修正，消除「自己保存后也提示有新版本」的误报
3. A2/A7/A8 布局与浮动条定位
4. Ghost Text、诊断段落标记等数据源补齐后再做

A1 必须先于 A3/A5：上游的专注模式与查找替换都依赖「编辑器实例不重建」，反过来做会在重建时丢状态，体验更差。
