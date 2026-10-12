# Biz Novel Studio — Rust 原生后端重构技术设计

> 版本：v1.0（架构提案） · 日期：2026-10-12  
> 目标：以 Biz Novel Studio 现有开源仓库为迁移对象，保留 React/Plate 前端，使用 Tauri 2 + Rust 重建本地业务内核，发布不依赖 Node.js Runtime 的 macOS/Windows/Linux 桌面端。  
> 状态：待实现、待 PoC 验证；本文中的体积、性能和工期均为设计目标或工程估算，非实测结论。

## 1. 摘要与 ADR

**ADR-001：采用单进程 Rust 原生桌面内核，不保留 Node Sidecar。** Tauri 负责窗口和进程；Rust 业务模块负责数据库、模型调用、Agent 工作流和长期记忆。前端保留 React 19、Vite、Plate，通过 Tauri Commands 请求响应，通过 Channels 流式接收内容和进度。

**ADR-002：不对 LangGraph 做逐行翻译。** 用版本化工作流定义、不可变运行合同、SQLite 事实台账、CAS 控制态和后台 Tokio Worker 替换 TS LangGraph Runtime。保留现有“自动导演”核心语义：唯一活跃 Run、产物版本、人工 Gate、质量债、幂等命令、失败恢复及章节正文覆盖保护。

**ADR-003：以新数据库安全导入，不直接原地修改旧 Prisma SQLite 文件。** 第一阶段只迁移完整可用的小说工作流数据；剩余功能进入独立兼容清单，不宣称全量等价。

**ADR-004：单机离线优先，AI 调用允许云端。** 不强制安装 Qdrant、Docker、PostgreSQL；无网络时本地编辑、检索和导出继续工作。离线 LLM/Embedding 是可选 Provider，不是首版前提。

### 1.1 范围

包含：小说/卷/章、人物/世界设定、章节快照、基本知识库、Prompt Registry、LLM 流式调用、章生成与审校、自动导演状态机、RAG、本地任务恢复、导入导出、日志、应用更新。

首版不包含：云端多用户协作、原仓库所有第三方平台功能全量迁移、图像/短剧生产线全面迁移、分布式 Worker、复杂 MCP 插件执行平台、手机端；这些需要单独兼容性里程碑。

## 2. 技术选型

| 层 | 选型 | 说明 |
| --- | --- | --- |
| 桌面 | Tauri 2 | 原生 WebView，macOS 使用 WKWebView |
| UI | React 19 + Vite + Plate | 尽量复用现有编辑器和业务页面 |
| 语言/异步 | Rust 2024 Edition + Tokio | Rust 1.90+ 作为初始版本约束候选，最终由兼容性矩阵锁定 |
| IPC | Tauri Commands + `Channel<T>` | 查询/变更走 Commands，生成/进度走 Channels |
| 持久化 | SQLite + SQLx | 本地一个主库；WAL、FK、忙等待、嵌入迁移 |
| LLM | reqwest + serde + tokio-util | 抽象 Provider，解析 SSE/JSON 流，取消和超时 |
| Agent | 自研 Durable Workflow Engine | 领域 DAG + 步骤执行器 + 持久化日志 |
| 检索 | SQLite FTS5 + 本地向量适配层 | 中文分词需单独处理；不依赖服务型向量库 |
| 日志 | tracing / tracing-subscriber | 统一 span、故障诊断、脱敏 |
| 密钥 | OS Keychain 适配层 | Keychain / Credential Manager / Secret Service 等 |
| 发布 | Tauri Bundler + Updater | 签名、公证、更新签名校验 |

不引入 Axum 本地 HTTP 服务作为默认方案；如果未来需要 Headless Server，再通过可选独立入口挂 Axum，并复用相同 application crate。

## 3. 总体架构

```text
┌────────────────────────── React / Plate ──────────────────────────────┐
│ 小说编辑器 / 世界观 / 人物 / 章节 / Director UI / 资料书架 / 任务中心 │
└───────────────────────┬───────────────────────────────────────────────┘
                        │ invoke + Channel / typed DTO
┌───────────────────────▼───────────────────────────────────────────────┐
│ Tauri Shell: window / capabilities / updater / dialogs / lifecycle    │
│ Command Facade: validation / error mapping / subscription             │
├───────────────────────────────────────────────────────────────────────┤
│ Application: Novel / Production / Director / Knowledge / Export       │
│ Domain: entities / invariants / RunContract / workflow transitions    │
│ Ports: NovelRepo / RunLedger / LlmProvider / Embedding / Clock        │
├───────────────────────────────────────────────────────────────────────┤
│ Infrastructure: SQLite/SQLx · reqwest Providers · FTS5 · Keychain     │
└───────────────────────────────────────────────────────────────────────┘
```

依赖方向为：`tauri_app → application → domain`；`infrastructure` 实现 application/domain 声明的 Ports，最终由 composition root 注入。`domain` 不允许引用 Tauri、SQLx、HTTP Client 或系统时间。

### 3.1 Rust Workspace

```text
client/                         # 复用现有 React/Plate，改造 transport adapter
src-tauri/                      # Tauri 应用、窗口、命令注册、权限和发布
crates/
  novel-domain/                 # Novel、Chapter、World、Character、业务规则
  novel-application/            # 用例、DTO 契约、事务边界和 Ports
  novel-storage-sqlite/         # SQLx Repo、迁移、FTS5、备份/导入
  novel-llm/                    # Provider、SSE、结构化输出、限流与成本
  novel-context/                # 文档分块、召回、重排、上下文预算
  novel-workflow/               # DAG、Run、StepRegistry、Worker、恢复
  novel-prompt/                 # Prompt Registry、版本、模板渲染
  novel-platform/               # Keychain、日志、导入导出、文件资源
  novel-testkit/                # Mock Provider、Fake Clock、故障注入
contracts/                      # 前后端数据合同、schema/代码生成
migrations/                     # Rust 新库迁移
migration-tools/                # 原 Prisma SQLite → 新库的单向导入工具
```

首个 PoC 可先合并 application/domain 至 2–3 个 crate，避免过早模块化；功能边界明确后再拆分。

## 4. 交互 API 和响应契约

禁止在 React 业务页面直接散落 `invoke()`；通过 `client/src/platform/desktopTransport.ts` 封装。原有 Axios `/api/*` 不是自动兼容 Tauri IPC，需按业务服务建立映射。公共类型使用 `specta`/`tauri-specta` 或 `ts-rs` 生成 TypeScript，避免手工同步重复 DTO。

```typescript
// 示例合同（概念性 API，尚未实现）
type AppError = {
  code: 'VALIDATION' | 'CONFLICT' | 'NOT_FOUND' | 'CANCELLED' |
        'MODEL_RATE_LIMIT' | 'STORAGE_CORRUPT' | 'INTEGRITY_ERROR';
  message: string;
  retryable: boolean;
  details?: unknown;
};

interface NovelTransport {
  listNovels(): Promise<NovelSummary[]>;
  getChapter(input: { novelId: string; chapterId: string }): Promise<Chapter>;
  saveChapter(input: { chapterId: string; expectedVersion: number; content: string }): Promise<Chapter>;
  startGeneration(input: StartGeneration, onEvent: (event: GenerationEvent) => void): Promise<{ runId: string }>;
  cancelRun(input: { runId: string; commandId: string }): Promise<void>;
  replayRunEvents(input: { runId: string; afterSeq: number }): Promise<GenerationEvent[]>;
}
```

`startGeneration` 建议先在 SQLite 写入 Run + Command 返回 `runId`，然后由命令附带 Channel 或通过 `subscribeRun(runId, afterSeq, channel)` 订阅；不会依赖短暂的 IPC Channel 保证消息持久化。页面刷新/应用重启后通过事件序号补拉丢失事件，再恢复订阅。

**流式事件：** `run_started`、`step_started`、`token_delta`、`checkpoint_saved`、`needs_approval`、`step_succeeded`、`run_paused`、`run_failed`、`run_completed`，每个持久化事件带 `runId / seq / stepId / occurredAt`；`token_delta` 允许临时、合并发送，不逐 token 持久化。事件大 payload 不穿透 IPC，引用本地 Artifact ID。

## 5. 本地数据设计

### 5.1 核心表（示意，不是原 Prisma 表的逐表翻译）

| 领域 | 表/实体 | 不变式 |
| --- | --- | --- |
| 作品 | `novel`, `volume`, `chapter` | 作品 ID、卷序、章节序稳定 |
| 版本 | `chapter_revision`, `document_version` | 用户编辑不可被 AI 自动覆盖 |
| 世界/人物 | `world_fact`, `character`, `character_state`, `timeline_event` | 事实有出处、有效时间、可信等级 |
| 运行 | `workflow_run`, `workflow_control`, `workflow_command` | 一书一个活跃 Run、命令幂等、CAS |
| 步骤产物 | `workflow_artifact`, `workflow_event`, `quality_debt` | 不可变版本、可追溯、明确质量债 |
| AI 账单 | `llm_invocation`, `usage_ledger` | 请求/模型/预算/计费状态可追溯 |
| 知识库 | `knowledge_document`, `knowledge_chunk`, `embedding` | 版本和索引状态一致 |
| 检索诊断 | `retrieval_trace` | 默认只保存 ID 和摘要，不存原文 |
| 迁移 | `legacy_id_map`, `migration_run` | 可追踪来源、可重复验证 |

核心数据结构（示例）：

```sql
CREATE TABLE workflow_run (
  id TEXT PRIMARY KEY,
  novel_id TEXT NOT NULL REFERENCES novel(id),
  plan_version TEXT NOT NULL,
  contract_json TEXT NOT NULL,
  created_at INTEGER NOT NULL
);
CREATE TABLE workflow_control (
  run_id TEXT PRIMARY KEY REFERENCES workflow_run(id),
  novel_id TEXT NOT NULL,
  status TEXT NOT NULL CHECK(status IN (
    'queued','running','waiting_approval','paused','succeeded','failed','cancelled')),
  version INTEGER NOT NULL DEFAULT 0,
  lease_owner TEXT,
  lease_until INTEGER
);
CREATE UNIQUE INDEX one_active_run_per_novel
ON workflow_control(novel_id)
WHERE status IN ('queued','running','waiting_approval','paused');
CREATE TABLE workflow_command (
  command_id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES workflow_run(id),
  kind TEXT NOT NULL,
  request_hash TEXT NOT NULL,
  result_json TEXT NOT NULL,
  created_at INTEGER NOT NULL
);
CREATE TABLE workflow_event (
  run_id TEXT NOT NULL REFERENCES workflow_run(id),
  seq INTEGER NOT NULL,
  kind TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  occurred_at INTEGER NOT NULL,
  PRIMARY KEY(run_id, seq)
);
```

控制态更新在 SQLite 事务中执行 `UPDATE workflow_control SET ..., version = version + 1 WHERE run_id = ? AND version = ?`，并检查 `rows_affected == 1`。任何事务外序号计算、状态迁移和产物版本分配都禁止。

SQLite 设置建议：`foreign_keys=ON`、`journal_mode=WAL`、`busy_timeout=5000ms`；迁移前关闭其他进程、做一致性备份。`synchronous` 策略由数据安全测试决定，不能为压榨性能默认降到风险更高的配置。

### 5.2 中文全文检索与向量

SQLite FTS5 的默认分词器不等于合格的中文语义分词。首版使用 Rust 中文分词将检索文本转为词项，再写 FTS5 `unicode61` 索引；原文保存在普通表中。验证双字词、专有名词、人名、魂兽名称的召回，必要时补别名表和 n-gram 索引。

向量接口定义 `EmbeddingProvider`、`VectorIndexPort`。默认优先对小规模知识库使用 SQLite BLOB 保存向量 + Rust 线性扫描余弦相似度；规模变大再启用本地 ANN 实现（并评估其跨平台构建及授权）。向量长度、维度、模型名、版本及标准化标志必须与 Chunk 绑定；更换 embedding 模型需要重建索引，不能跨维度混搜。

检索：强作用域过滤 → 关键词 TopK 与向量 TopK → RRF 融合 → 可选 reranker → 叙事距离衰减 → Context Broker 预算裁剪；显式文档选择比全局资料优先，`ownerTypes` 是硬过滤，空结果可按配置作无 facet 回退。评估 Hit@K、MRR 和引文正确率，不以“向量化成功”等同于“召回正确”。

## 6. LLM Provider Gateway

```rust
#[async_trait::async_trait]
pub trait LlmProvider: Send + Sync {
    async fn generate(
        &self,
        request: LlmRequest,
        sink: TokenSink,
        cancel: tokio_util::sync::CancellationToken,
    ) -> Result<LlmResponse, LlmError>;
}
```

第一批适配 OpenAI-compatible（可配置 `/chat/completions`）、单独的 Anthropic 协议（按需求），本地 Ollama 可作为可选插件；不要假设所有兼容模型都支持统一 JSON Schema、usage 字段和 token 计数。每个 Provider 的 capabilities 明确 `streaming / structured_output / tool_call / reasoning / context_limit` 等能力，结构化输出以 `serde` 反序列化 + JSON Schema 校验 + 有界修复重试。

- 支持每请求硬上限、模型路由、超时、显式取消、并发和速率限制；网络错误采用分类退避，禁止无条件重复扣费请求。
- 原始 API Key 存储于操作系统凭据管理器；数据库只保存引用 ID。日志、trace、导出包默认脱敏。
- `llm_invocation` 保存 `request_id`、`run_id`、`step_id`、模型名、输入摘要、开始结束时间、token usage（可空）、估算费用、结果状态；usage 未知时不能伪装成 0。
- 远端检索文档和正文视作不可信内容：检索结果不得提升成系统指令，工具能力在应用层严格授权。

## 7. Durable Workflow Engine（迁移重点）

### 7.1 模型

`WorkflowDefinition { version, nodes, edges, guards, gates }` 是可序列化的版本化 DAG；`RunContract` 在 Run 创建时冻结小说范围、选择模型、Prompt 版本、预算、失败策略与依赖版本。`StepRegistry` 以步骤类型注册 `StepHandler`，领域状态机在 domain crate，调度和事务管理在 application/workflow crate。

**示例计划：** `novel_seed → story_macro → character_cast → volume_strategy → chapter_list → chapter_batch`。章内链：`context → draft → consistency_review → bounded_repair → approval → commit`。生成章节内容只写候选修订，作者确认才提升为正式正文；任何上游重新计算都不得自动覆盖 `user_edited` 或已发布的正文。

### 7.2 运行协议

1. `open_run(command_id)`：事务写入不可变合同、控制态和命令响应，返回 Run ID。
2. Worker 只领取 `queued/running` 中到期且可领取的 Run；`waiting_approval/paused` 不自动领取。
3. 每次只推进一个可计算领域步骤，先记录步骤尝试事实，执行依赖和预算守卫。
4. LLM 调用在数据库事务外完成；流式字串写临时候选草稿并做节流 Checkpoint。
5. 将有效产物、用量台账、步骤完成事件及控制游标作为一个本地提交单元；依赖 `expectedVersion` 做 CAS。
6. 用户 `approve/retry/cancel/resume` 一律通过带 command ID 的幂等命令，禁止由 UI 直接改控制态。
7. 重启恢复根据已持久化产物和事件重算下一步，不使用进程内游标当唯一事实。

**必须明确的交付语义：** 本地 SQLite 事务可实现一次性提交，但无法保证外部 LLM 请求 exactly-once。发生进程崩溃时，远端可能已产生费用但本地未收到响应。默认保留中断候选并要求人工确认重试；对于可安全复算步骤也必须有预算上限、请求 ID 和重试事实，避免无限自动重生成。

### 7.3 人工 Gate 和章节保护

运行状态采用 `queued → running → waiting_approval / paused / succeeded / failed / cancelled`，只允许定义中的合法转换。`stop_signal` 使用结构化类型（budget、data_integrity、provider_unavailable、approval_required 等），不从报错字符串推断。作者修改形成 `user_edited` 产物；审校仅提出修复建议或候选版本，不能自动覆盖人工编辑。

### 7.4 并发与恢复

单进程 Tokio Worker Pool，首版全局并发默认 1、可调至 2；单个 Novel 同时最多一个活跃 Run。Worker 租约写进 `workflow_control`，每次领取检查到期与版本；异常恢复基于已冻结的 `maxAutomaticRetries`，禁止改变在途 Run 的策略。关机时停止领新任务、取消可中断网络流、完成在途本地事务并保存可恢复状态。

## 8. 兼容与数据迁移

原仓库已有 `DirectorNextRun/RunControl/Artifact/QualityDebt/Event/Command` 六类事实、CAS、唯一活跃 Run、质量债及 RAG 的 owner/facet 检索规则。Rust 化需要**保留这些业务不变式**，而不只是迁移表数据。

**迁移策略：**

1. 锁定旧仓库 commit、Prisma schema + SQLite migrations、Prompt 版本和字段语义；生成 `legacy-schema-manifest.json`。
2. 退出旧版应用，对原数据库执行 SQLite Backup API 或一致性快照（含 WAL）；存为只读备份，绝不能在 live DB 上直接文件复制且遗漏 WAL。
3. Rust 新库运行自身 migrations；用一次性 `migration-tool` 将旧表映射为规范化模型。所有旧 ID 保持可追溯（`legacy_id_map`），长文与资源文件计算 SHA-256。
4. 全量校验小说/卷/章数量、字符数、主外键、最近修订、人物/世界设定、Prompt、Director Run/Artifact/控制态；不支持的字段输出 `migration_unsupported.json`，不可静默丢弃。
5. 不可恢复的旧 Running Run 转为 `paused + migration_review_required`，禁止自动重放；用户人工核实后恢复。
6. 新版读新库，默认不修改旧库；导出标准化 JSON/Markdown 作为第二条逃生通道。需要回退时重新打开旧版备份，不能反向复用新数据库。

最重要的兼容性门槛是现有 UI 所依赖的实体模型与 API 行为，而非仅仅数据库表结构；通过业务合同清单追踪每项 API 从 Express 到 Tauri 的实现状态。

## 9. 安全与发布

- IPC Commands 最小授权，文件读写范围受限；不提供任意 shell 命令执行；不从任意远程网页加载具备 Tauri 权限的 UI。
- 用户可配置 Provider endpoint，但远程默认强制 HTTPS；localhost 特例需明确授权，阻止内网 SSRF 和凭据意外外传。
- 导入时校验路径边界、文件大小、格式、压缩包解压穿越和外链；日志不写 API Key 或未授权的章节正文。
- macOS arm64 为 P0 发布目标；Windows x64 和 Linux x64 为 P1。公开 macOS 分发需要签名、公证；自动更新必须按 Tauri Updater 生成并验证签名，私钥只保存在受保护 CI Secret 中。
- 许可证：继承/修改上游 AGPL-3.0-only 的部分必须遵守其授权要求；第三方 crate、向量实现和资源逐项做 license audit，不以“Rust 重写”为规避原许可的理由。

## 10. 验收与性能预算

| 指标 | 首版目标（待 PoC 实测） | 测试方法 |
| --- | --- | --- |
| 运行依赖 | 发布包没有 Node/Electron/Python | 干净机器安装和安装包扫描 |
| macOS | Apple Silicon 可独立安装运行 | DMG、签名、公证、冷启动 |
| 安装包 | macOS arm64 DMG **目标 ≤ 100 MB** | Release 构建实际度量 |
| 冷启动 | 正常设备 **目标 p95 < 3 秒**（不含云端 AI） | 30 次冷启动测量 |
| 空闲 RSS | **目标 < 250 MB**（含 WebView，机器/OS 说明） | Instruments / Activity Monitor |
| 章节持久化 | 连续编辑、崩溃重启无已确认正文丢失 | 故障注入 + 版本校验 |
| 任务恢复 | 崩溃后可识别中断步骤，不覆盖人工稿 | kill -9 / 断网 / 取消注入 |
| RAG | 角色、设定、伏笔召回不低于冻结基线 | Hit@K / MRR / Citation Precision |
| LLM | 能流式输出、取消、超时、错误分类 | Mock SSE + 真实多 Provider 测试 |

性能目标是门槛假设；首个 PoC 先做相同小说数据集的 Electron vs Tauri 对照，再决定是否需要优化索引和编译配置。由于 WebView 内存计算口径与 Electron 多进程不同，比较时应同时列应用进程树总 RSS、基线和冷启动样本。

## 11. 工程实施路线与工期

| 阶段 | 交付 | 工期粗估（单人全职） | Gate |
| --- | --- | --- | --- |
| P0：基线和合同 | 功能/API/表映射、固定测试集、旧版数据库备份 | 1–2 周 | 迁移覆盖清单和退出策略明确 |
| P1：Tauri+SQLx 核心 | 小说/卷/章 CRUD、修订、只读旧数据导入 | 2–3 周 | 可编辑、重启不丢数据 |
| P2：LLM Gateway | 多模型、SSE、取消、token/费用记录 | 2–3 周 | 章节流式生成端到端 |
| P3：Durable Workflow | 导演 DAG、Gate、CAS、重启恢复 | 3–5 周 | 故障注入通过 |
| P4：Context/RAG | 知识库、中文 FTS、Embedding 与组装 | 2–4 周 | 冻结评测集达标 |
| P5：发布/回归 | macOS DMG/公证、导入、兼容回归 | 2–3 周 | 干净设备可独立使用 |

**首个可用 P0/P1 创作 MVP：约 6–10 周。涵盖导演 + RAG 的核心可用产品：约 12–20 周。完整替代上游全部功能：可能 6 个月以上，需重新估算。** 工期受实际功能覆盖、旧数据复杂度和平台测试影响，不保证线性叠加。

### 应优先做的三个原型

1. **Native Runtime Spike（1–2 天）**：Tauri + SQLx 写/读一章 10 万字、Plate 中文输入、独立打包并测量安装体积/RSS。
2. **Provider Spike（2–3 天）**：Rust reqwest SSE 对接现有模型，保证取消、结构化输出、错误分类和 token 用量追踪。
3. **Recovery Spike（3–5 天）**：`open_run → draft → kill process → restore → approve`，证明不会覆盖作者修改，说明重复远端调用如何记录。

三个 Spike 的测试结果达标后，再批准完整迁移；若任一核心能力不可接受，可保留 Electron 的 macOS 打包作为短期使用方案。

## 12. 风险台账

| 风险 | 等级 | 控制措施 |
| --- | --- | --- |
| 原项目功能范围庞大，Rust 覆盖不全 | 高 | P0 冻结功能矩阵、按业务纵切迁移 |
| LangGraph 状态语义与恢复不等价 | 高 | 复刻 Run 合同和事实不变式，故障注入 |
| Prisma DB 中关系/迁移复杂 | 高 | 单向导入、显式字段清单、原库只读备份 |
| 中文检索召回质量下降 | 高 | 分词/别名/混合检索基线和离线评测 |
| Plate 在 WKWebView 下输入异常 | 中 | macOS 中文 IME、粘贴、撤销、超长正文专项测试 |
| 包体积/内存未达到预期 | 中 | Release 体积分析与分模块基准测试 |
| macOS 签名公证/更新复杂 | 中 | CI 提前验证签名、稳定版本通道 |

## 13. 开发规范与验收定义

- 每个领域功能的 PR 必须包含：业务规则说明、请求/返回 DTO、Rust 单元测试、SQLite 集成测试、必要的前端 E2E。
- 流程恢复按进程级真实 SQLite 测试，不用纯内存 Fake 代替事务、锁和唯一索引验证。
- 所有 Prompt、WorkflowPlan、Migration 都必须版本化；新增版本不能无声重解释在途 Run。
- 使用 `cargo fmt --check`、`cargo clippy -- -D warnings`、`cargo test --workspace`、前端 typecheck/lint/E2E 与 macOS clean-room 打包验证。
- 不允许静默覆盖人工正文、静默丢失旧数据、静默重复远端模型请求；任何不可判定状态须明确提示人工处理。

## 14. 本文所依据的源代码/官方文档

**项目源码及既有业务不变式（读取时点 2026-10-12）：**
- https://github.com/ExplosiveCoderflome/AI-Novel-Writing-Assistant/blob/main/desktop/src/runtime/server.ts
- https://github.com/ExplosiveCoderflome/AI-Novel-Writing-Assistant/blob/main/client/src/lib/desktop.ts
- https://github.com/ExplosiveCoderflome/AI-Novel-Writing-Assistant/blob/main/server/src/prisma/schema.sqlite.prisma
- https://github.com/ExplosiveCoderflome/AI-Novel-Writing-Assistant/blob/main/docs/wiki/architecture/director-next-fact-layer.md
- https://github.com/ExplosiveCoderflome/AI-Novel-Writing-Assistant/blob/main/docs/wiki/rag/knowledge-and-context-assembly.md

**官方技术文档：**
- Tauri Commands/Channels：https://v2.tauri.app/develop/calling-rust/
- Tauri Updater：https://v2.tauri.app/plugin/updater/
- Tauri macOS Bundling：https://v2.tauri.app/distribute/macos-application-bundle/
- SQLx SQLite：https://docs.rs/sqlx/latest/sqlx/sqlite/
- SQLite FTS5：https://www.sqlite.org/fts5.html
