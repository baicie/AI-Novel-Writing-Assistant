# 后端 Rust 化路线图

## 背景

桌面端已经迁到 Tauri 壳子（`src-tauri/`），但产品主链路目前仍然由 Tauri sidecar 启动的 Node Express 服务（`server/`，1088 个 TS 文件、约 22.8 万行）承担在 `127.0.0.1:3000`。Tauri 进程里已经内嵌了一份"本地创作内核"（`crates/novel-domain` / `novel-workflow` / `novel-application` / `novel-llm` / `novel-context` / `novel-prompt` / `novel-storage-sqlite` / `novel-platform`），它通过 `#[tauri::command]` 已经能服务小说、章节、自动导演运行生命周期、模型设置等命令（见 `src-tauri/src/lib.rs` 第 81–435 行）。

但下面这些能力目前 **没有在 Rust 内核里实现**，仍由 Node 服务承担：

- 知识库（knowledge）与 RAG 检索路由
- setup 模块的子路由（world、character、bookAnalysis、onboarding、titleLibrary、genre、astrology、images、writingFormula、creativeHub、promptWorkbench、styleEngine、styleEngineExtraction）
- drama、comic、marketRadar、visualAssets 模块
- export 模块
- 创作编排（director 的 productionInputs / productionComposition / savedContent / opening / existingAssets / batchOutcome / workspace / usage / entrySwitch / projections / recovery）
- llm-live 流式桥与 chat 路由
- tasks 中心与 settings 路由（`/api/tasks/*`、`/api/settings/*` 的大部分）
- agentCatalog / agentRuns
- rag、autoDirectorChannelCallbacks、autoDirectorFollowUps
- 通用 health、access gate、characterConversation

旧 Prisma 库 → 新 SQLite kernel schema 的导入路径已经具备（`crates/novel-storage-sqlite/src/legacy_import.rs`，`LEGACY_SCHEMA_COMMIT = "8e848e6e"`），并由 `migration-tools` 提供命令行入口。

## 目标

把产品主链路从"Webview → Node sidecar（3000）"切到"Webview → Rust 内核（3000）"。阶段完成前，Rust 内核不是产品主入口，每完成一个模块就把对应的 Node 路由从装配里摘除。

切完以后：

- 不再依赖 Node 运行时；不再为产品安装 Node。
- 自动导演、章节生产、知识检索、setup 全部走 Rust 内核。
- HTTP 接口语义和现有调用保持一致；不引入新的鉴权或路由前缀。
- Prisma 旧库通过 `migration-tools` 一次性导入新 SQLite 库，导入报告里列出的 `unsupported` 必须在产品里给出对应处理（停用对应能力或显式提示）。

## 决策

**迁移方式：按 HTTP 模块逐个替换。**

每个阶段都做三件事：

1. 在 `crates/novel-application` 或新的 `crates/novel-http` 里加上对应端口的 HTTP 路由。
2. 在 Tauri 进程里启一个 axum server 监听 3000；该模块对应的 Node 路由在迁移完成后从 `server/src/app.ts` 删除。
3. 在 `src-tauri/src/lib.rs` 的 `runtime_script` 里把 `apiBaseUrl` 指向新端口之前，必须保证该模块已在 Rust 内核里实现且回归通过。

**端口策略：单端口、严格失败。**

Tauri 内核独占 3000。未实现的路由返回 `501 Not Implemented`，并带 `X-AI-NOVEL-MIGRATION-STATUS: pending` 头。前端不强降级，遇到 501 就把请求路由回到迁移前的"阅读视图"或显式禁用该能力。Node sidecar 一旦内核准备就绪就被关掉。

**数据迁移：一次性、显式命令。**

`migration-tools` CLI 是从旧 Prisma SQLite 导入新 SQLite kernel schema 的唯一入口。它必须由用户手动调用，不在 Tauri 启动时自动跑。导入报告里 `unsupported` 不为空时，应用启动时给出 `迁移报告里有 N 项未迁移的字段/表，请查阅 X.md` 提示，不静默继续。

**Prompt 治理：仍走 `server/src/prompting/`，不复制到 Rust。**

当前阶段 `crates/novel-prompt` 只暴露 72 行注册入口，远不足以替代 Node 的 prompt registry。Rust 侧的 prompt 资产（`chapter_draft` / `chapter-draft-v1`）由 Node 的 prompt registry 通过 `novel-prompt::resolve` 解析；模型调用仍在 Node 一侧完成，并把 token 流通过 `subscribe_run` 通道送回 webview。**Prompt 资产全部迁到 Rust 是另一条独立轨道**（见 `docs/wiki/architecture/prompt-registry-migration.md` 的占位说明，未来单独立文档）。

**LLM 调用：Node 侧保持，Rust 侧留 `UnconfiguredProvider`。**

内核当前用 `UnconfiguredProvider` 占位（见 `src-tauri/src/lib.rs` `Runtime::new`），实际生成仍由 Node 服务的 `factory.ts` 负责。Rust 侧的 `novel-llm` 仍用于契约校验与端点授权（`authorize_endpoint`），不直接发起真实生成。`crates/novel-llm` 升级到真生产路径要等 `novel-llm` 现有 provider 列表和 Node 的 provider 配置 1:1 对齐后再做。

**模块边界：沿用 `crates/README.md`。**

依赖方向不变：

```
src-tauri / migration-tools
        │
        ▼
novel-application ── ports
        │
        ├── novel-domain
        ├── novel-workflow
        ├── novel-llm
        ├── novel-context
        └── novel-prompt

novel-storage-sqlite 实现 application 的仓储端口
novel-platform 提供密钥、日志、导出和系统时钟读取
```

新增的 HTTP / 编排服务统一放进 `crates/novel-http/`（待建）；不直接挂在 `src-tauri/` 上，避免 Tauri 进程入口继续膨胀。

## 当前规则

- 任何 HTTP 路由在 Rust 内核里没有实现前，Node 路由继续存在。
- 阶段切流前必须满足：Rust 路由 + Tauri 入口 + 前端调用回归通过；migration 报告无 `unsupported` 涉及该模块。
- 内核启动时只跑 `SqliteKernel::open`，不自动做 `legacy_import`。迁移工具独立调用。
- 旧 Prisma 库和新的 kernel 库在物理上是两个文件，应用数据目录下两个文件并存期间，UI 必须显式提示用户在哪个库上工作。
- 切流期间任一模块出现 5xx 立即回滚：把 `runtime_script` 的 `apiBaseUrl` 指回 Node 3000；不能依赖前端重试或降级。

## 阶段划分

### 阶段 0：地基（Tauri + Node sidecar 双轨现状已具备）

- 已经完成的：Tauri 壳、`NodeBackend` 启动 Node sidecar 的生命周期、Rust 内核在 Tauri 进程内的 `list_novels` / `list_chapters` / `create_novel` / `create_chapter` / `get_chapter` / `save_chapter` / `open_run` / `approve_run` / `cancel_run` / `resume_run` / `replay_run` / `cancel_generation` / `subscribe_run` / `get_model_setup` / `save_model_setup` / `has_api_key` 命令、迁移工具 CLI、Prisma 旧库导入。
- 范围与限制：本阶段的 Rust 命令 **不接入** webview 主调用路径，仅作可行性验证。`runtime_script` 的 `apiBaseUrl` 仍指向 Node 3000。

### 阶段 1：内核独占 3000，已实现模块切流

- 新建 `crates/novel-http/`，引入 axum，注册阶段 0 列出的所有 Tauri 命令对应的 HTTP 路由。
- `src-tauri` 内启动 axum server 监听 3000，Node sidecar 启动失败时回退到 stage-0 行为（继续走 sidecar）。
- 前端调用代码保持 `fetch("/api/...")` 不变；webview 拿到的 `apiBaseUrl` 从 `http://127.0.0.1:3000`（Node）切到同一个 URL（Tauri 内核）。
- 健康检查 `GET /api/health` 在内核里返回 200；`/api/onboarding/first-novel`、`/api/novels`、`/api/access/status`、`/api/settings/llm-selection` 等仍在 Node 端的路由必须返回 501，前端调用方显示"开发中"。
- migration 报告里 `unsupported` 为空才允许切流；否则拒绝切流并显示报告。
- 验收：阶段 0 的所有 Tauri 命令的端到端流程通过；Node 端 `app.ts` 里 stage-0 覆盖的路由从 `import` 列表里摘除但保留文件，作为后续对照。

#### 阶段 1 实际交付（2026-10-14 提交）

- `crates/novel-http/` 已建好，依赖 `axum` / `tower` / `tower-http` / `novel-application` / `novel-platform` / `novel-storage-sqlite`。
- 已注册 15 条 HTTP 路由：`/api/health`、`/api/novels`（GET/POST）、`/api/novels/:id/chapters`（GET/POST）、`/api/novels/:id/chapters/:cid`（GET/PUT）、`/api/novels/:id/runs`（POST）、`/api/runs/:rid/{approve,cancel,resume,cancel-generation}`（POST）、`/api/runs/:rid/events`（GET）、`/api/setup/model`（GET/POST）。
- 兜底路由返回 501 + `{success:false, migration:"pending", hint}`，前端用 `silentErrorStatuses` 列表忽略掉这些 501。
- Tauri 进程新增 `LaunchPlan::KernelIntegrated` 形态：当 `AI_NOVEL_KERNEL_BACKEND=1` 时跳过 Node sidecar 启动，由 `novel-http` 在 Tauri 进程内起 axum；`apiBaseUrl` 跟随 `AI_NOVEL_SERVER_PORT`（默认 3000）。
- `migration-tools` 已能在 30 秒内把空 Prisma 库导入到新 SQLite kernel schema，报告输出到 `<data_dir>/migration-report.json`。
- 集成测试 3/3 通过：health、novel CRUD round-trip、未实现路由 501。
- 端到端 `cargo tauri dev` 验证通过：kernel 在 3001 起（避开 dev Node 3000 冲突），webview 调用 `/api/knowledge` 命中 501，`/api/novels` 走内核新建 200。三个端口并存正常。
- **未做**：`unsupported` 非空时拒绝启动的硬卡（目前只是写报告，不阻断启动）。`/api/novels/...` 子路由里很多 director 编排、setup、knowledge 仍返回 501，等阶段 2-5 逐个迁入。

### 阶段 3.0a：任务中心只读路径迁入

#### 范围

- 迁 2 条只读端点：`GET /api/tasks/overview` 和 `GET /api/tasks/recovery-candidates`。
- 数据源限定为 Rust kernel 已有的 `workflow_run` / `workflow_control` / `quality_debt` 三张表，对应 Node 端 `TaskCenterService` 中 auto_director run 的统计和恢复候选项。
- 其余 9 条任务中心路由（列表 / 详情 / 重试 / 取消 / 归档 / recovery resume / autoDirector follow-ups）继续由 Node 承担，Rust 端保持 501 兜底。

#### 降级范围

3.0a 阶段"运行记录"页对比 Node 端会少 5 类业务项的概览（拆书 / 流水线 / 图像 / 知识 / 风格提取 / AgentRun），`recovery-candidates` 端点也只返回 `novel_workflow` 一类。前端需要识别这个"部分可见"的过渡期状态：成功响应数据不报错，但其他业务项的统计全部为 0 或空，UI 文案配合提示"其他类目正在迁入"。

降级对用户的影响：

- "运行记录"页顶部数字只反映 auto_director run。
- "待恢复任务"列表里没有拆书、流水线、图像、风格提取项。
- 列表 / 详情 / 重试 / 取消 / 归档按钮仍由 Node 端处理，调用链路保持不变。

#### 数据契约

保持与 Node 端 `TaskOverviewSummary` / `RecoverableTaskSummary` 字段名完全一致（`camelCase`），前端不需要改任何代码即可切流。

#### 模块与文件

- `crates/novel-application/src/dto.rs` —— 新增 `TaskOverviewSummary` / `RecoverableTaskSummary` / `RecoverableTaskListResponse`。
- `crates/novel-application/src/ports.rs` —— 新增 `TaskCenterRepository` trait（`task_overview` / `list_recovery_candidates`）。
- `crates/novel-application/src/tasks.rs` —— 新增 `TaskCenterService`。
- `crates/novel-application/src/runtime.rs` —— `Runtime` 字段加 `tasks`，`RuntimeDeps` 加 `task_center`。
- `crates/novel-storage-sqlite/src/tasks.rs` —— 实现 `TaskCenterRepository`，SQL 全部基于 `workflow_run` / `workflow_control` / `quality_debt`。
- `crates/novel-http/src/routes/tasks.rs` —— 2 条 axum handler。
- `crates/novel-http/src/state.rs` —— `supported_path_list` 增加 2 条。
- `crates/novel-http/tests/integration.rs` —— 2 条新测试 + 1 条"5xx 必须 501"保护测试。
- `crates/novel-application/tests/recovery.rs` / `src-tauri/src/lib.rs` —— `RuntimeDeps` 加 `task_center` 字段。

#### 验收

- `cargo test --workspace` 全过（37 个测试，含 2 条新增）。
- `GET /api/tasks/overview` 返回 `queuedCount/runningCount/failedCount/cancelledCount/waitingApprovalCount/recoveryCandidateCount`，空库全为 0。
- `GET /api/tasks/recovery-candidates` 空库返回 `items: []`；有 `paused` run 时返回 `novel_workflow` 项，title / sourceRoute / recoveryHint 都从 Rust 表里取。
- 5 类写操作路由（POST retry / POST cancel / POST archive / GET list / GET detail）仍返回 501 + `migration: pending`，前端"运行记录"页写按钮命中 501 时不报错。

#### 后续阶段

3.0a~g 拆解见 `docs/wiki/architecture/backend-rustization-roadmap.md` 后续小节。3.0b~f 每完成一张业务表的 Rust 化（拆书 / 流水线 / 图像 / 风格提取 / 知识 / AgentRun），就把 3.0a 的两个端点扩到该表。3.0g 收尾时再把 9 条写操作路由迁入 Rust。

### 阶段 2：setup 模块迁入

- 小说基础信息、人物、世界观（world 子路由：core、structure、visualization、generation、slice）整体迁入 `crates/novel-http` 与 `novel-application`。
- World visualization 不迁（前端 React 组件直接消费 Rust 暴露的数据，不再走 Node proxy）。
- prompt registry 继续在 Node；Rust 通过 IPC 拉到 `resolve(...)` 后的字符串。

### 阶段 3：director 编排迁入

- `services/novel/director` 的 productionInputs、productionComposition、savedContent、opening、existingAssets、batchOutcome、workspace、usage、entrySwitch、projections、recovery 迁到 `novel-application`。
- Worker loop 已经在 Rust 里；剩下的编排规则迁完后 Node 这边的 director 路由从 `app.ts` 摘除。

### 阶段 4：知识库 / RAG

- `routes/knowledge.ts`、`routes/rag.ts`、`modules/setup/knowledge/*`（如有）迁到 `novel-context` + `novel-http`。
- `crates/novel-context` 已有 `assemble.rs` / `tokenize.rs` / `vector.rs`，按 Node 的 `context-assembly.ts` 校齐。

### 阶段 5：其它路由

- 创作 Hub、style engine、drama、comic、marketRadar、visualAssets、export、agentCatalog、agentRuns、tasks、settings、llm-live、autoDirector follow-ups、autoDirector channel callbacks、bookAnalysis、characterConversation 按依赖顺序迁入。

### 阶段 6：去掉 Node sidecar

- 切流后保留一个 release cycle 观察，确认没有未识别的 501 / 5xx。
- 删除 `server/src/app/desktop/sidecarEntry.ts`、`src-tauri/src/node_backend/`（保留 snapshot / log / process 这些可复用的 Rust 模块到 `crates/novel-runtime` 或 `crates/novel-process`）、`tauri.bundle.conf.json` 里的 `externalBin` 与 `resources` server 部分。
- Tauri 安装包不再携带 Node 运行时。
- `runtime_script` 里 `isPackaged: true` 不变；webview 通过 IPC 调用内核，HTTP 仅保留给 `tauri://localhost` 内部的 axum server（仅在 webview origin 之内访问）。

## 失败模式

- 阶段 1 切流后前端访问未实现路由返回 501，UI 必须给出"开发中"提示而不是空白；用户不能以为是 bug。
- 旧 Prisma 库和新 kernel 库并存期间，导入顺序写反或路径相同会被 `legacy_import` 拒绝。
- `migration_tools` 报告里出现 `unsupported`，用户不查阅就强切流，会出现字段丢失。阶段 1 起应用启动时强制读取报告并校验。
- Rust 端 Tauri 命令的错误响应里出现 5xx 等于数据完整性问题，需要立即回滚到 sidecar 并把 `apiBaseUrl` 切回。
- prompt 资产留在 Node 是有意识的设计；如果未来有人把 `chapter_draft` 提示词复制到 `crates/novel-prompt`，会和 Node 端漂移，需要在 review 里拦截。
- 阶段 0 现有的 Tauri 命令已经在 `lib.rs` 里实现，但还没有经过完整回归；阶段 1 切流前必须补齐至少一条 happy path 的 E2E。

## 相关模块

- `crates/README.md`（依赖方向与端口契约）
- `docs/wiki/architecture/rust-native-kernel.md`（内核规则）
- `docs/wiki/architecture/server-architecture-migration-plan.md`（Node 内部重构）
- `docs/wiki/architecture/tauri-node-sidecar.md`（sidecar 启动协议，切流后会被废止）
- `docs/wiki/architecture/novel-application-services.md`（阶段 0 服务的入口）

## 源文档

- `src-tauri/src/lib.rs`
- `src-tauri/src/node_backend/mod.rs` / `process.rs` / `server_log.rs` / `snapshot.rs`
- `crates/novel-application/src/{director.rs,worker.rs,ports.rs}`
- `crates/novel-storage-sqlite/src/legacy_import.rs`
- `migration-tools/src/main.rs`
- `server/src/app.ts`（待逐阶段摘除）
- `client/src/platform/desktopTransport.ts`（前端 IPC 抽象）
