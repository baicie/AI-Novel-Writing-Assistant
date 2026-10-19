# Phase 3.0 任务分解与执行清单

> 版本：v1.0  
> 日期：2026-10-20  
> 分支：`feature/rust-tasks-3.0` （继续）  
> 工期目标：4 周内完成核心迭代

## 任务概览

| 任务 | 优先级 | 工期 | 状态 | 负责 |
|------|-------|------|------|------|
| Task 3.1 | P0 | 2d | ⏳ 待启动 | 待指派 |
| Task 3.2 | P0 | 3d | ⏳ 待启动 | 待指派 |
| Task 3.3 | P0 | 2d | ⏳ 待启动 | 待指派 |
| Task 3.4 | P0 | 3d | ⏳ 待启动 | 待指派 |
| Task 3.5 | P0 | 2d | ⏳ 待启动 | 待指派 |
| Task 3.6 | P1 | 3d | ⏳ 待启动 | 待指派 |
| Task 3.7 | P0 | 4d | ⏳ 待启动 | 待指派 |
| Task 3.8 | P0 | 3d | ⏳ 待启动 | 待指派 |
| Task 3.9 | P1 | 2d | ⏳ 待启动 | 待指派 |
| Task 3.10 | P0 | 4d | ⏳ 待启动 | 待指派 |

---

## 详细任务清单

### Task 3.1：核心领域实体定义（Domain Layer）

**前置条件**：无  
**依赖**：无  
**工期**：2 天  
**优先级**：P0  

**目标**：
- ✓ 完成 Novel、Chapter、World、Character 等核心实体定义
- ✓ 实现状态机与不变量检查
- ✓ 所有实体支持 Serde 序列化/反序列化

**具体工作**：

1. **新建 crate**
   ```bash
   cd crates
   cargo new novel-domain --lib
   ```
   编辑 `Cargo.toml`：
   - 添加依赖：`serde`、`uuid`、`chrono`、`thiserror`
   - 特性：`serde/derive`

2. **实现实体**
   - 文件：`crates/novel-domain/src/entities/novel.rs`
   - 文件：`crates/novel-domain/src/entities/chapter.rs`
   - 文件：`crates/novel-domain/src/entities/world.rs`
   - 文件：`crates/novel-domain/src/entities/character.rs`
   - 文件：`crates/novel-domain/src/entities/workflow.rs`

3. **错误定义**
   - 文件：`crates/novel-domain/src/error.rs`
   ```rust
   #[derive(Debug, thiserror::Error)]
   pub enum DomainError {
     #[error("Validation error: {0}")]
     Validation(String),
     
     #[error("Invalid state transition: {from} -> {to}")]
     InvalidTransition { from: String, to: String },
     
     // ...
   }
   ```

4. **模块组织**
   - 文件：`crates/novel-domain/src/lib.rs`
   ```rust
   pub mod entities;
   pub mod error;
   pub mod invariants;
   ```

5. **单元测试**
   - 文件：`crates/novel-domain/src/entities/novel.rs` 内测试
   - 验证：创建实体、状态转移、不变量检查

**验证清单**：
- [ ] `cargo build` 编译通过
- [ ] `cargo test` 单元测试通过
- [ ] `cargo clippy` 无 warning
- [ ] 实体结构文档完整

**完成标志**：
- 所有实体已定义
- 单元测试覆盖 > 80%
- 无 cargo warning

**提交说明**：
`新增：Phase 3.0 Task 1 - 核心领域实体定义（Novel、Chapter、World、Character）`

---

### Task 3.2：数据库持久化层（Storage Layer）

**前置条件**：Task 3.1 完成  
**依赖**：Task 3.1  
**工期**：3 天  
**优先级**：P0  

**目标**：
- ✓ 建立 SQLite 数据库与迁移框架
- ✓ 实现 Repository 接口与 CRUD 操作
- ✓ 集成测试验证数据库操作

**具体工作**：

1. **新建 crate**
   ```bash
   cd crates
   cargo new novel-storage-sqlite --lib
   ```
   编辑 `Cargo.toml`：
   ```toml
   [dependencies]
   sqlx = { version = "0.7", features = ["runtime-tokio", "sqlite", "migrate", "json"] }
   tokio = { version = "1", features = ["full"] }
   novel-domain = { path = "../novel-domain" }
   serde_json = "1"
   uuid = { version = "1", features = ["v4", "serde"] }
   thiserror = "1"
   ```

2. **迁移脚本**
   - 目录：`crates/novel-storage-sqlite/migrations/`
   - 文件：`001_initial_schema.sql`
   ```sql
   CREATE TABLE IF NOT EXISTS novels (
     id TEXT PRIMARY KEY,
     title TEXT NOT NULL,
     author TEXT,
     description TEXT,
     status TEXT NOT NULL DEFAULT 'Created',
     created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
     updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
     UNIQUE(title)
   );
   
   CREATE TABLE IF NOT EXISTS chapters (
     id TEXT PRIMARY KEY,
     novel_id TEXT NOT NULL REFERENCES novels(id),
     chapter_number INTEGER NOT NULL,
     title TEXT NOT NULL,
     content TEXT,
     word_count INTEGER,
     generation_state TEXT,
     chapter_status TEXT,
     created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
     updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
     UNIQUE(novel_id, chapter_number)
   );
   
   -- 类似地创建 worlds、characters、runs 等表
   CREATE TABLE IF NOT EXISTS worlds (
     id TEXT PRIMARY KEY,
     novel_id TEXT NOT NULL REFERENCES novels(id),
     name TEXT NOT NULL,
     world_type TEXT,
     description TEXT,
     key_attributes JSON,
     created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
   );
   
   CREATE TABLE IF NOT EXISTS characters (
     id TEXT PRIMARY KEY,
     novel_id TEXT NOT NULL REFERENCES novels(id),
     name TEXT NOT NULL,
     role TEXT,
     personality TEXT,
     relationships JSON,
     arc TEXT,
     created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
   );
   
   CREATE INDEX idx_novels_author ON novels(author);
   CREATE INDEX idx_chapters_novel_id ON chapters(novel_id);
   ```

3. **Repository 接口定义**
   - 文件：`crates/novel-storage-sqlite/src/repos/mod.rs`
   ```rust
   pub trait NovelRepository: Send + Sync {
     async fn save(&self, novel: Novel) -> Result<()>;
     async fn get(&self, id: &str) -> Result<Option<Novel>>;
     async fn list(&self, limit: i64, offset: i64) -> Result<Vec<Novel>>;
     async fn delete(&self, id: &str) -> Result<()>;
   }
   ```

4. **SQLite 实现**
   - 文件：`crates/novel-storage-sqlite/src/repos/novel_sqlite.rs`
   - 实现 CRUD 操作，使用 SQLx 查询宏

5. **连接池初始化**
   - 文件：`crates/novel-storage-sqlite/src/db.rs`
   ```rust
   pub async fn init_db(database_url: &str) -> Result<SqlitePool> {
     sqlx::migrate!()
       .run(&pool)
       .await?;
     Ok(pool)
   }
   ```

6. **集成测试**
   - 文件：`crates/novel-storage-sqlite/tests/integration_tests.rs`
   - 测试：CRUD 操作、迁移、数据完整性

**验证清单**：
- [ ] `cargo build` 编译通过
- [ ] SQLx CLI 验证迁移
- [ ] 集成测试通过
- [ ] 迁移可重入

**完成标志**：
- 数据库表已创建
- CRUD 操作完整
- 集成测试覆盖 > 80%

**提交说明**：
`新增：Phase 3.0 Task 2 - 数据库持久化层（SQLite + SQLx + 迁移框架）`

---

### Task 3.3：应用层用例与事务边界（Application Layer）

**前置条件**：Task 3.1、3.2 完成  
**依赖**：Task 3.1、3.2  
**工期**：2 天  
**优先级**：P0  

**目标**：
- ✓ 定义业务用例与应用服务
- ✓ 实现 DTO 与错误映射
- ✓ 建立清晰的事务边界

**具体工作**：

1. **新建 crate**
   ```bash
   cd crates
   cargo new novel-application --lib
   ```

2. **用例定义**
   - 文件：`crates/novel-application/src/use_cases/mod.rs`
   - 实现：`CreateNovelUseCase`、`GetChapterUseCase` 等

3. **DTO 定义**
   - 文件：`crates/novel-application/src/dto.rs`
   ```rust
   pub struct CreateNovelInput {
     pub title: String,
     pub author: String,
     pub description: Option<String>,
   }
   
   pub struct NovelDTO {
     pub id: String,
     pub title: String,
     // ...
   }
   ```

4. **错误处理**
   - 文件：`crates/novel-application/src/error.rs`
   - 从 Domain 和 Storage 错误转换为应用错误

5. **服务实现**
   - 文件：`crates/novel-application/src/services/novel_service.rs`
   - 业务逻辑与编排

6. **单元测试**
   - Mock Repository，验证用例逻辑

**验证清单**：
- [ ] `cargo build` 编译通过
- [ ] 用例测试通过
- [ ] 类型检查通过

**完成标志**：
- 主要用例已实现
- 单元测试覆盖 > 70%

**提交说明**：
`新增：Phase 3.0 Task 3 - 应用层用例与事务边界`

---

### Task 3.4：LLM Provider 与结构化输出（LLM Layer）

**前置条件**：Task 3.1 完成  
**依赖**：Task 3.1  
**工期**：3 天  
**优先级**：P0  

**目标**：
- ✓ 实现 LLM Provider 抽象
- ✓ 支持 OpenAI 和其他模型端点
- ✓ 实现流式响应与结构化输出

**具体工作**：

1. **新建 crate**
   ```bash
   cd crates
   cargo new novel-llm --lib
   ```
   依赖：`reqwest`、`tokio`、`serde`、`tokio-util`

2. **Provider 接口**
   - 文件：`crates/novel-llm/src/lib.rs`
   ```rust
   pub trait LlmProvider: Send + Sync {
     async fn invoke_text(&self, req: TextRequest) -> Result<TextResponse>;
     async fn invoke_structured(&self, req: StructuredRequest) -> Result<StructuredResponse>;
   }
   ```

3. **OpenAI 实现**
   - 文件：`crates/novel-llm/src/providers/openai.rs`
   - 调用 `/v1/chat/completions` 端点
   - 支持流式响应

4. **错误处理与重试**
   - 文件：`crates/novel-llm/src/error.rs`
   - 实现指数退避重试

5. **单元测试**
   - Mock HTTP 响应
   - 验证解析与错误处理

**验证清单**：
- [ ] `cargo build` 编译通过
- [ ] 单元测试通过
- [ ] 流式响应测试通过

**完成标志**：
- Provider 接口完整
- OpenAI 实现可用
- 测试覆盖 > 70%

**提交说明**：
`新增：Phase 3.0 Task 4 - LLM Provider 与结构化输出`

---

### Task 3.5：Prompt Registry 与结构化定义（Prompt Layer）

**前置条件**：Task 3.1 完成  
**依赖**：Task 3.1  
**工期**：2 天  
**优先级**：P0  

**目标**：
- ✓ 建立 Prompt 注册机制
- ✓ 实现版本管理与模板渲染
- ✓ 支持结构化输出 Schema

**具体工作**：

1. **新建 crate**
   ```bash
   cd crates
   cargo new novel-prompt --lib
   ```
   依赖：`serde_json`、`handlebars`

2. **Prompt Asset 定义**
   - 文件：`crates/novel-prompt/src/asset.rs`

3. **注册表实现**
   - 文件：`crates/novel-prompt/src/registry.rs`
   - 从 JSON 加载或内存维护

4. **模板渲染**
   - 集成 `handlebars` 库
   - 变量替换与验证

5. **Schema 验证**
   - 验证 LLM 输出是否符合 schema
   - JSON 修复机制

**验证清单**：
- [ ] `cargo build` 编译通过
- [ ] 模板渲染测试通过
- [ ] Schema 验证测试通过

**完成标志**：
- Prompt 注册表可用
- 模板渲染完整
- 测试覆盖 > 70%

**提交说明**：
`新增：Phase 3.0 Task 5 - Prompt Registry 与结构化定义`

---

### Task 3.6：RAG 与知识库检索（Context Layer）

**前置条件**：Task 3.2 完成  
**依赖**：Task 3.2  
**工期**：3 天  
**优先级**：P1  

**目标**：
- ✓ 实现文档分块与 FTS5 全文检索
- ✓ 建立向量检索基础设施
- ✓ 完成上下文组装逻辑

**具体工作**：

1. **新建 crate**
   ```bash
   cd crates
   cargo new novel-context --lib
   ```

2. **文档分块**
   - 文件：`crates/novel-context/src/chunking.rs`

3. **FTS5 集成**
   - 迁移脚本创建虚表
   - 实现全文搜索

4. **向量存储 trait**
   - 预留接口，暂用内存实现

5. **上下文组装**
   - 召回、排序、裁剪

**验证清单**：
- [ ] `cargo build` 编译通过
- [ ] 搜索测试通过
- [ ] 端到端召回测试通过

**完成标志**：
- 全文检索可用
- 上下文组装完整
- 测试覆盖 > 70%

**提交说明**：
`新增：Phase 3.0 Task 6 - RAG 与知识库检索（FTS5）`

---

### Task 3.7：工作流引擎骨架（Workflow Layer）

**前置条件**：Task 3.1、3.2 完成  
**依赖**：Task 3.1、3.2  
**工期**：4 天  
**优先级**：P0  

**目标**：
- ✓ 实现自动导演工作流核心框架
- ✓ 实现 Step 注册表与执行器
- ✓ 建立后台 Worker 与事件日志

**具体工作**：

1. **新建 crate**
   ```bash
   cd crates
   cargo new novel-workflow --lib
   ```

2. **工作流定义**
   - 文件：`crates/novel-workflow/src/definition.rs`

3. **Step 注册表与工厂**
   - 文件：`crates/novel-workflow/src/step_registry.rs`

4. **Run 与执行状态**
   - 文件：`crates/novel-workflow/src/run.rs`
   - 状态机实现

5. **Worker 与异步执行**
   - 文件：`crates/novel-workflow/src/worker.rs`
   - Tokio Worker 拉取并执行任务

6. **事件日志**
   - 持久化记录步骤输入/输出

7. **集成测试**
   - 端到端工作流执行

**验证清单**：
- [ ] `cargo build` 编译通过
- [ ] 工作流执行测试通过
- [ ] 事件日志记录正确

**完成标志**：
- 工作流引擎基本可用
- Worker 能正常运行
- 测试覆盖 > 70%

**提交说明**：
`新增：Phase 3.0 Task 7 - 工作流引擎骨架与 Worker`

---

### Task 3.8：Tauri 命令层与前端集成（Tauri Layer）

**前置条件**：Task 3.1-3.7 完成  
**依赖**：Task 3.1-3.7  
**工期**：3 天  
**优先级**：P0  

**目标**：
- ✓ 建立 Tauri 命令与应用用例的映射
- ✓ 实现前端 Transport 适配层
- ✓ 完成流式响应支持

**具体工作**：

1. **Tauri 命令定义**
   - 目录：`src-tauri/src/commands/`
   - 文件：`src-tauri/src/commands/novel.rs`
   ```rust
   #[tauri::command]
   async fn create_novel(
     state: tauri::State<'_, AppState>,
     title: String,
   ) -> Result<NovelDTO> {
     // 调用应用用例
   }
   ```

2. **错误映射**
   - 文件：`src-tauri/src/error.rs`
   - 将错误转换为 Tauri 格式

3. **前端 Transport 适配**
   - 文件：`client/src/platform/desktopTransport.ts`
   ```typescript
   export const desktopTransport = {
     async createNovel(title: string) {
       return invoke('create_novel', { title });
     },
   };
   ```

4. **流式响应**
   - 使用 Tauri Channel
   - LLM 生成结果分批推送

5. **生命周期管理**
   - 应用启动初始化
   - 应用关闭清理资源

6. **集成测试**
   - 端到端命令调用

**验证清单**：
- [ ] 编译通过
- [ ] TypeScript 类型检查通过
- [ ] 端到端测试通过

**完成标志**：
- 主要命令已实现
- 流式响应可用
- 集成测试通过

**提交说明**：
`新增：Phase 3.0 Task 8 - Tauri 命令层与前端集成`

---

### Task 3.9：平台层与系统集成（Platform Layer）

**前置条件**：Task 3.8 完成  
**依赖**：Task 3.8  
**工期**：2 天  
**优先级**：P1  

**目标**：
- ✓ 实现密钥管理（Keychain）
- ✓ 建立日志与诊断体系
- ✓ 支持导入导出功能

**具体工作**：

1. **新建 crate**
   ```bash
   cd crates
   cargo new novel-platform --lib
   ```

2. **密钥管理**
   - 文件：`crates/novel-platform/src/keychain.rs`
   - macOS/Windows/Linux 适配

3. **日志体系**
   - 集成 `tracing`
   - 配置日志级别

4. **文件 I/O**
   - 导入导出功能
   - 备份机制

**验证清单**：
- [ ] `cargo build` 编译通过
- [ ] 各平台兼容性测试

**完成标志**：
- 密钥管理可用
- 日志体系就位
- 导入导出功能完整

**提交说明**：
`新增：Phase 3.0 Task 9 - 平台层与系统集成（Keychain、日志、导入导出）`

---

### Task 3.10：测试基础设施与集成验证

**前置条件**：Task 3.1-3.9 完成  
**依赖**：Task 3.1-3.9  
**工期**：4 天  
**优先级**：P0  

**目标**：
- ✓ 建立完整的测试框架
- ✓ 验证端到端工作流
- ✓ 建立性能基准

**具体工作**：

1. **新建 crate**
   ```bash
   cd crates
   cargo new novel-testkit --lib
   ```

2. **Mock 工具**
   - Mock LLM Provider
   - Fake Clock
   - 测试数据工厂

3. **单元测试**
   - 各 crate 内部测试
   - 目标覆盖 > 70%

4. **集成测试**
   - 跨 crate 集成
   - 使用内存 SQLite

5. **端到端测试**
   - Tauri 应用启动
   - 完整命令链调用

6. **性能基准**
   - 章节生成速度
   - 数据库查询性能

7. **代码覆盖率**
   - 使用 `cargo tarpaulin` 或 `cargo llvm-cov`

**验证清单**：
- [ ] 所有测试通过
- [ ] 代码覆盖率 > 70%
- [ ] 性能基准建立

**完成标志**：
- 完整的测试框架就位
- 所有关键路径通过端到端测试
- 性能基准已记录

**提交说明**：
`新增：Phase 3.0 Task 10 - 测试基础设施与集成验证`

---

## 执行流程与检查点

### 周期性检查点

**每日**（每项任务）：
- [ ] 代码编译通过
- [ ] 本地单元测试通过
- [ ] 无 cargo warning

**每任务完成**：
- [ ] 提交代码并更新 git 历史
- [ ] 补充相关文档
- [ ] 运行集成测试

**每周**（周末总结）：
- [ ] 审查本周成果
- [ ] 更新进度文档
- [ ] 规划下周重点

**阶段完成**（Task 3.10）：
- [ ] 所有测试通过
- [ ] 文档完整
- [ ] 性能基准建立
- [ ] 准备向 beta 分支合并

---

## 预期交付物

### 代码交付
- [ ] 10 个 Rust crate（domain、storage、application 等）
- [ ] 完整的数据库迁移脚本
- [ ] Tauri 命令与前端适配
- [ ] 测试框架与测试用例

### 文档交付
- [ ] 架构设计说明（已有）
- [ ] API 文档与 Rust doc 注释
- [ ] 数据库 schema 说明
- [ ] 测试计划与结果报告
- [ ] 性能基准数据

### 验证交付
- [ ] 编译通过：`cargo build --release` 无 warning
- [ ] 测试通过：所有单元与集成测试通过
- [ ] 覆盖率 > 70%
- [ ] 性能达标：章节生成 < 30s

---

## 风险与变更管理

### 已知风险

| 风险 | 影响 | 应对 |
|-----|------|------|
| 编译时间长 | 迭代慢 | 并行构建、增量编译 |
| SQLite 并发瓶颈 | 性能问题 | 连接池、WAL 模式 |
| Tauri 兼容性 | 平台支持不足 | 定期测试各平台 |

### 变更流程

如需调整任务范围、工期或优先级：
1. 在本文档中更新相应项目
2. 在 GitHub issue 或讨论中说明理由
3. 更新此清单的"变更记录"部分

---

## 变更记录

| 日期 | 内容 | 原因 |
|-----|------|------|
| 2026-10-20 | 初版发布 | 基于 Phase 2.2 完成 |

---

## 联系与反馈

- 技术文档：见 `docs/design/2026-10-20-phase-3.0-design.md`
- 问题追踪：GitHub Issues
- 讨论：GitHub Discussions
