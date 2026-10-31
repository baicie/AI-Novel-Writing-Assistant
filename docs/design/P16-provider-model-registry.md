# P16 — AI Provider & Model Registry（审计与设计）

> 状态：P16.0 完成（本文件即产物）；P16.1 已落地（见第 11 节）。P16.2–P16.4 未开始。
> 仓库：`baicie/AI-Novel-Writing-Assistant`　基准：`beta`（`29e8e09e`）
> 说明：任务书要求的目标分支 `migration/tauri-2-rust` 已在 PR #3 squash 合入后删除，当前唯一在用的集成分支是 `beta`，因此 P16 在 `beta` 上进行，不重建该分支。

## 1. 现状（逐条带证据）

### 1.1 存储：只有一条「默认 provider」记录

`crates/novel-storage-sqlite/src/model.rs`：

- 表 `provider_credential`（迁移 `migrations/0002_model_setup.sql`）
- 主键固定为 `id='default'`，`provider` 固定写 `'openai-compatible'`，唯一密钥引用是常量 `MODEL_SECRET_REF = "openai-compatible"`
- 提供 `save_model_setup` / `load_model_setup` 两个方法，字段只有 `endpoint / model / allow_localhost_http / secret_ref`

结论：**没有 provider 概念，也没有模型清单**。一个应用只能有一个地址 + 一个模型名。

### 1.2 凭据：按 provider 字符串查系统凭据

`src-tauri/src/commands/model.rs` 的 `OsModelSecrets`（实现 `novel_application::ModelSecrets`）：
`api_key(provider) -> OsSecretStore.get(provider)`，即**provider 字符串就是凭据键**；
`get_quick_setup_status`（`crates/novel-application/src/setup.rs:108`）也直接探测 `"openai-compatible"` 这个键。

### 1.3 请求链路（真实调用顺序）

```text
React 组件
  → client/src/api/*.ts (axios)
  → client/src/platform/desktopApiAdapter.ts   （桌面把 HTTP 路由映射到 Tauri invoke，未映射返回 501）
  → client/src/platform/desktopTransport.ts    （invoke 名与入参）
  → src-tauri/src/commands/*.rs                （IPC 边界）
  → crates/novel-application/src/*             （用例编排；worker/director 在此层）
  → crates/novel-storage-sqlite/src/*          （SQLite 持久化）
  → crates/novel-llm/src/provider.rs           （OpenAiCompatibleProvider，直接吃 endpoint+api_key+model）
```

生成时的模型解析：`crates/novel-application/src/worker.rs`（391/615/1271 行附近）
先 `secrets.api_key(&contract.provider)`，再把 `contract.endpoint / model / allow_localhost_http` 装进 `LlmRequest`。
即 **Run contract 在创建时冻结 provider+model+endpoint，执行时只按 provider 取密钥**。

`crates/novel-application/src/director.rs`：`provider` / `model` 作为输入参数（32-33 行），随后写入 run summary（499-500、931-932 行）。

### 1.4 前端

- 网页版模型配置在 `client/src/pages/settings/SettingsPage.tsx` + `components/ProviderSettingsSection.tsx`，调用 `/settings/api-keys`、`/settings/providers`、`/settings/model-routes` 等接口
- 桌面版这些路由**全部未映射**，`desktopApiAdapter.ts` 的 `unsupported()` 会返回 501
- 上一版（0.1.1-beta.6）已把桌面设置页改成 `DesktopSettingsView` + `DesktopModelSettingsCard`（单套地址/模型/密钥 + 真实连通性测试），只是**仍然只有一套配置**

## 2. 缺陷根因（对照用户痛点）

| 用户痛点 | 根因 |
| --- | --- |
| 添加 Provider 不方便 | 存储层只有 `id='default'` 一行；前端桌面版只有一套表单 |
| 模型列表无法自动发现 | 完全没有 discovery 代码：`novel-llm` 只有 `generate`，没有 `GET /models`；前端模型名靠手写 |
| 模型能力参数难维护 | 领域层没有 `ModelCapabilities`；上下文/最大输出只存在于 `ProviderPolicy` 的全局默认（4k/32k） |
| 任务级模型选择不统一 | 只有 Run contract 里的 `provider+model` 字符串；没有统一的 `ModelBinding` 概念与任务级选择界面 |

## 3. 外部参考：DSH 的可观测结构（含来源与缺口）

**缺口（P1）**：任务书列出的 `packages/llm/llm-pi-ai/src/{discovery,catalog}.ts`、
`packages/client/ui-settings-models/src/client/{ModelListEditor.tsx,operations.ts}`、`docs/user/guide/providers.md`
在本机不存在（`~/.dsh` 只装了运行时，没有 harness 源码；全局 node_modules 也没有）。**未读这些文件，不做任何引用式模仿**。

可观测事实（来源：本机 `~/.dsh/profiles/desktop/cordis.patch.yml`，密钥已脱敏）：
```yaml
- id: llm-pi-ai            # provider registry 配置块
  config:
    providers:
      workbuddy:           # provider id 即稳定标识
        displayName: ...   # 可选展示名
        apiKeyEnv: WORKBUDDY_API_KEY      # 凭据是「引用」，不是明文
        api: openai-completions           # 协议判别字段（另有 openai-responses）
        baseURL: http://.../v1
        models:
          - id: cn:deepseek-v4.1-flash    # 远端模型 ID 原样保留（含命名空间前缀）
            name: Deepseek-V4.1-Flash
            contextWindow: 1000000
            maxTokens: 393216
            input: [text, image]
            reasoningEfforts: { low: low, high: high, max: max }
- id: agent-default-model  # 全局绑定
  config: { provider: workbuddy, model: cn:deepseek-v4.1-flash, reasoningEffort: max }
```

三条可迁移的结论：
1. **Provider 是稳定 id，协议是字段**（`api`），不靠模型名猜协议；
2. **模型身份 = (provider id, model id)**，模型 id 原样保留远端写法；
3. **能力是可缺省的结构化字段**（未声明就是未知），**凭据是引用**（env/secret ref），不落明文。

## 4. 方案对比与 ADR

### 备选
- **A. 直接搬 DSH 的 YAML + pi-ai 实现**：任务禁止引入 Cordis/DSH Runtime，且我们已有 SQLite 与凭据系统 → 否决。
- **B. 另建一套 `provider_registry` 并行存储**：违反「不得与已有模型配置并行两套权威存储」 → 否决。
- **C. 在 `provider_credential` 上扩展成真正的 registry**（本方案采纳）。

### ADR-1：以 `provider_credential` 为唯一权威存储，扩展而不是重建
保留表名与 `id='default'` 的兼容读写（老版本读到的仍是这一行），新增列与两张从表：
- `provider_credential` 增列：`protocol`、`display_name`、`is_builtin`、`sort_order`、`updated_at`
- 新表 `provider_model`：`(provider_id, model_id)` 主键、能力字段、`source`、`user_overrides`
- 新表 `model_binding`：`task_type` 主键 → `(provider_id, model_id, reasoning_effort)`

### ADR-2：能力用「四层来源 + 用户覆盖最高优先级」
`builtin < discovered < supplemental < user`。读取时按优先级合并；**未知能力保持 `None`，不默认 true/false**。

### ADR-3：discovery 属于协议适配器，不属于领域
`ProviderModelDiscovery` 只约束「输入 ProviderProfile + 凭据 → 输出 ModelDiscoveryOutcome」；
OpenAI-compatible / Anthropic / Ollama 各自实现。领域层不写 HTTP。

### ADR-4：Run 快照只冻结引用
Run 继续冻结 `provider_id + model_id + protocol + endpoint` 与 `credential_ref`，**不冻结密钥**；
恢复时校验 `(provider_id, model_id)` 仍可解析，否则给可恢复错误，绝不静默换模型。

## 5. 数据库迁移（0022）

> 说明：原计划占用 0021，实施时发现 0021 已被 `0021_structured_fact_version.sql` 使用，遂顺延为 0022（文件 `migrations/0022_provider_registry.sql`）。

```sql
-- 0019/0020/0021 已存在（0021 已被 structured_fact_version 占用），本次用 0022
ALTER TABLE provider_credential ADD COLUMN protocol TEXT NOT NULL DEFAULT 'openai-compatible';
ALTER TABLE provider_credential ADD COLUMN display_name TEXT;
ALTER TABLE provider_credential ADD COLUMN is_builtin INTEGER NOT NULL DEFAULT 0;
ALTER TABLE provider_credential ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0;
ALTER TABLE provider_credential ADD COLUMN updated_at INTEGER NOT NULL DEFAULT 0;

CREATE TABLE IF NOT EXISTS provider_model (
  provider_id TEXT NOT NULL,
  model_id    TEXT NOT NULL,
  display_name TEXT,
  context_window INTEGER,
  max_output_tokens INTEGER,
  supports_streaming INTEGER,   -- 三态：NULL 未知
  supports_tools INTEGER,
  supports_reasoning INTEGER,
  input_modalities TEXT,        -- JSON 数组，NULL 表示未知
  reasoning_efforts TEXT,       -- JSON 对象，NULL 表示未知
  source TEXT NOT NULL DEFAULT 'manual',
  user_overrides TEXT,          -- JSON：仅保存用户覆盖的字段
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  PRIMARY KEY (provider_id, model_id)
);

CREATE TABLE IF NOT EXISTS model_binding (
  task_type TEXT PRIMARY KEY,
  provider_id TEXT NOT NULL,
  model_id TEXT NOT NULL,
  reasoning_effort TEXT,
  updated_at INTEGER NOT NULL
);
```

兼容：`id='default'` 那一行保持可读可写；老版本应用打开新库不受影响（只多列与两张新表）。
回滚：迁移是纯增量（ADD COLUMN / CREATE TABLE IF NOT EXISTS），老代码忽略新列。

## 6. 领域 DTO（P16.1 落地）

```rust
pub struct ProviderProfile { pub id, display_name: Option<String>, pub protocol: ProviderProtocol,
    pub base_url: String, pub credential: CredentialReference, pub is_builtin: bool,
    pub allow_localhost_http: bool, pub sort_order: i32 }
pub enum ProviderProtocol { OpenAiCompatible, AnthropicMessages, Ollama, }
pub struct CredentialReference { pub kind: CredentialKind /* OsSecret | EnvVar */, pub reference: String }
pub struct ModelProfile { pub provider_id, pub model_id: String, pub display_name: Option<String>,
    pub capabilities: ModelCapabilities, pub source: ModelSource, pub overridden: BTreeSet<CapabilityField> }
pub struct ModelCapabilities { pub context_window: Option<u32>, pub max_output_tokens: Option<u32>,
    pub supports_streaming: Option<bool>, pub supports_tools: Option<bool>,
    pub supports_reasoning: Option<bool>, pub input_modalities: Option<Vec<Modality>>,
    pub reasoning_efforts: Option<BTreeSet<String>> }
pub struct ModelBinding { pub task_type: String, pub provider_id: String, pub model_id: String,
    pub reasoning_effort: Option<String> }
pub struct ModelDiscoveryOutcome { pub models: Vec<ModelProfile>, pub source: ModelSource,
    pub warnings: Vec<String> }
pub enum DiscoveryErrorCode { AuthFailed, Forbidden, RateLimited, NotSupported, InvalidResponse,
    NetworkError, Timeout, Cancelled }
```

**未知语义规则**：`Option<T>` 三态；`None` 表示「未知」，任何消费方不得把 `None` 解释成支持或不支持。

## 7. 模块依赖（只允许向下）

```text
novel-domain        （纯类型 + 校验，无 IO）
  ↑
novel-application   （用例：registry 服务、binding 解析、discovery 编排）
  ↑                     ↘ novel-llm（协议适配器：generate / list_models）
novel-storage-sqlite（0021 迁移 + repo）
  ↑
src-tauri           （IPC：commands/provider.rs）
  ↑
client (React)      （desktopTransport → desktopApiAdapter → UI）
```

## 8. 兼容策略

1. 老库打开即升级，`provider_credential` 的既有行自动获得默认 `protocol='openai-compatible'`。
2. 旧 UI 路径（`get_model_setup/save_model_setup`）保留为「默认 provider 的读写封装」，行为不变。
3. 网页版接口（`/settings/api-keys` 等）不动；P16.3 只替换桌面版设置页。
4. Run 快照字段不删不改，新增 `credential_ref` 与 `protocol` 为可选字段。

## 9. 测试矩阵（逐阶段执行）

| 阶段 | 层 | 用例 |
| --- | --- | --- |
| P16.1 | domain 单测 | 能力三态合并优先级；未知不解释；provider/model id 校验；binding 校验 |
| P16.1 | SQLite 集成 | 迁移 0021 幂等；老库升级后旧行可读；CRUD；用户覆盖不被发现结果覆盖；删除 provider 级联 |
| P16.2 | 适配器单测 | 成功/空列表/重复 id/异常 JSON/401/403/404/429/5xx/超时/取消/恶意重定向/无密钥/本地 http/带前缀 URL |
| P16.3 | 组件测试 | 草稿 dirty、竞态（旧响应不覆盖新表单）、去重、批量勾选、错误与空态、不回显密钥 |
| P16.4 | 集成 | 任务绑定解析；Run 冻结与恢复；模型删除后的可恢复错误；CAS 与停点不被破坏 |

**所有网络用例使用本地 mock server，不产生真实付费调用。**

## 10. 分阶段任务

| 阶段 | 内容 | 交付判据 |
| --- | --- | --- |
| P16.0 | 本文件 | 审计可复核、ADR 明确 |
| P16.1 | 领域类型 + 0021 迁移 + repo + 测试 | `cargo test -p novel-domain -p novel-storage-sqlite` 全绿 |
| P16.2 | `ProviderModelDiscovery` + OpenAI-compatible / 内置 / 手动 + 错误码 | 适配器测试覆盖第 9 节整行 |
| P16.3 | 桌面 Settings → Models 重做 | 组件测试 + 桌面启动无异常 |
| P16.4 | 任务绑定接入与 Run 冻结校验 | workspace 测试 + 前端 typecheck + 原生验收 |
## 11. P16.1 实施记录

落地内容（`crates/novel-domain/src/provider.rs`、`migrations/0022_provider_registry.sql`、`crates/novel-storage-sqlite/src/provider.rs`）：

- 领域类型：`ProviderProfile` / `ProviderProtocol` / `CredentialReference` / `ModelProfile` /
  `ModelCapabilities`（全三态）/ `ModelBinding` / `ModelDiscoveryOutcome` / `DiscoveryErrorCode`；
  能力合并 `overlay` 逐字段取 `Some`，合并发现结果时尊重 `overridden`；
- 存储：`provider_credential` 增列（protocol / display_name / is_builtin / sort_order / updated_at），
  新增 `provider_model` 与 `model_binding`；提供 upsert/list/delete（级联）与绑定读写；
  老的 `save_model_setup` / `load_model_setup` 行为不变；
- 测试证据：`cargo test -p novel-domain` **20 passed**；
  `cargo test -p novel-storage-sqlite --features test-harness` 全绿，
  其中新增 `tests/provider_registry.rs` **6 passed**（含「老库升级后 default 行仍可读、新列取默认值」与「重复打开幂等」）；
  `cargo clippy -p novel-domain -p novel-storage-sqlite --lib -- -D warnings` 通过；`cargo fmt` 通过。

顺带修正：`p151_historical_migration` 里硬编码的迁移条数改为 `novel_storage_sqlite::MIGRATION_COUNT`，
以后新增迁移不必再改断言。

## 12. P16.2 实施记录（部分完成）

已落地（`crates/novel-llm/src/discovery.rs`）：

- `ProviderModelDiscovery` 抽象 + `DiscoveryRequest / DiscoveryOutcome / DiscoveredModel`；
- **OpenAI 兼容**：`GET {base}/models`、`Authorization: Bearer`、`data` 数组与 `models` 对象映射两种写法、去重、原始 id 保留；
- **Anthropic**：`GET {base}/v1/models`、`x-api-key` + `anthropic-version`、`has_more / last_id` 游标分页（上限 20 页）；
- 8 个稳定错误码：`AUTH_FAILED / FORBIDDEN / RATE_LIMITED / NOT_SUPPORTED / INVALID_RESPONSE / NETWORK_ERROR / TIMEOUT / CANCELLED`；
- 安全：复用 `authorize_endpoint`（回环地址必须显式允许）、不跟随重定向、响应体 512 KB 上限、超时与取消、请求全部在 Rust 侧；
- `discovery_for(protocol)` 只认两个协议，**不猜**（Ollama 返回 `None`，留待独立适配器）。

测试证据：`cargo test -p novel-llm` → **18 单测 + 15 条真实 HTTP 用例**（本地 mock server）全绿，
覆盖成功、空列表、重复 id、异常 JSON、401/403/404/429/5xx、超时、取消、重定向不跟随、无密钥、
本地 http 未授权、带路径前缀 URL、Anthropic 分页与 401 归因；
`cargo clippy -p novel-llm --lib --all-targets -- -D warnings` 通过。

**未完成（P16.2 剩余）**：内置服务商版本化目录、Ollama `/api/tags` 适配器、
「手动添加不因远端未返回而删除」的编排（依赖 P16.3 的保存路径）。
