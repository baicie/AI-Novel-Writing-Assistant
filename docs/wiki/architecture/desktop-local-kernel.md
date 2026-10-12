# 桌面本地创作内核架构

## 概述

本地创作内核是 Tauri 桌面应用的 Rust 后端，为离线创作提供能力支撑。它由 6 个 crate 组成，分层次设计，支持系统凭据管理、章节离线查询和生成中断。

## 架构分层

```
┌─────────────────────────────────────────────┐
│  Tauri Desktop Commands Layer               │  src-tauri/src/lib.rs
│  (list_chapters, get_model_setup,           │
│   save_model_setup, cancel_generation)      │
└────────────────┬────────────────────────────┘
                 │
┌────────────────▼────────────────────────────┐
│  Application Layer                          │  novel-application/
│  (Runtime, CommandOutcome, Orchestration)  │
└────────────────┬────────────────────────────┘
                 │
     ┌───────────┼───────────────┐
     │           │               │
┌────▼──┐  ┌─────▼────┐  ┌──────▼──────┐
│Domain │  │Workflow  │  │LLM Provider │
│       │  │          │  │             │
└───────┘  └──────────┘  └─────────────┘
     │           │               │
     └───────────┼───────────────┘
                 │
         ┌───────▼────────┐
         │ Storage Layer  │
         │ (SQLite)       │
         └────────────────┘
```

### 各层职责

#### 1. **Tauri 命令层** (`src-tauri/src/lib.rs`)
- **职责**：HTTP/IPC 边界、请求反序列化、响应序列化
- **命令清单**：
  - `list_chapters(novel_id)` - 查询本书所有章节
  - `get_model_setup()` - 读取模型密钥（从系统凭据）
  - `save_model_setup(endpoint, model, api_key)` - 保存配置
  - `cancel_generation(run_id)` - 中止生成任务
  - `get_chapters_metadata()` - 章节元数据同步
  - `sync_chapter()` - 章节版本同步
- **核心约束**：
  - 不含业务逻辑，只负责参数传递
  - 所有异步操作通过 `State<AppState>` 访问
  - 错误映射到 `AppError`

#### 2. **应用层** (`novel-application/`)
- **职责**：用例编排、事务边界、错误处理
- **核心类型**：
  - `Runtime` - 创作执行引擎
  - `CommandOutcome` - 命令结果统一格式
  - `ModelSecrets` - 密钥端口（trait）
  - `PortError` - 端口层错误
- **已知实现**：
  - 密钥端口由 `OsModelSecrets` 实现（调用 `OsSecretStore`）

#### 3. **领域层** (`novel-domain/`)
- **职责**：小说、章节、保护状态规则
- **禁止项**：不能依赖 Tauri、SQLx、HTTP、系统时间
- **核心模型**：
  - `Novel` - 小说聚合根
  - `Chapter` - 章节实体
  - `ProtectionRule` - 保护规则

#### 4. **工作流层** (`novel-workflow/`)
- **职责**：根据已读产物决定下一步或恢复
- **禁止项**：不能主动领取任务、不能调用模型、工作流 crate 不能反向依赖端口
- **核心决策**：
  - 生成后接受/拒绝逻辑
  - 恢复与重试规则

#### 5. **LLM 层** (`novel-llm/`)
- **职责**：模型厂商抽象、结构化调用
- **关键类型**：
  - `UnconfiguredProvider` - 无配置厂商实例
  - 与 `ModelSecrets` 配合读取 API Key

#### 6. **存储层** (`novel-storage-sqlite/`)
- **职责**：SQLite 数据持久化
- **迁移**：`0002_model_setup.sql` 添加本地配置表
- **端口实现**：实现 `novel-application` 中定义的所有端口

---

## 系统凭据管理

### 密钥生命周期

```
用户输入 API Key
        │
        ▼
Tauri: save_model_setup()
        │
        ▼
OsSecretStore.set(provider, api_key)
        │
        ├─→ macOS: Keychain
        │
        └─→ 开发环: MemoryStore
        │
        ▼
创作时调用 cancel_generation() / open_run()
        │
        ▼
ModelSecrets::api_key() trait
        │
        ▼
OsSecretStore.get(provider)
        │
        ├─→ macOS: 从 Keychain 读
        │
        └─→ 开发环: 从内存读
        │
        ▼
LLM 调用（使用 Key）
```

### 关键约束

- **缺失时拒绝**：API Key 不存在时在应用层提前返回 `PortError::Storage`
- **隔离**：密钥不过客户端（Desktop UI），仅在 Tauri 命令层边界处理
- **重复输入避免**：第一次保存后，后续调用直接从凭据读取，无需重复输入

---

## 数据库迁移

### 0002_model_setup

```sql
ALTER TABLE provider_credential 
  ADD COLUMN model TEXT NOT NULL DEFAULT '';

ALTER TABLE provider_credential 
  ADD COLUMN allow_localhost_http INTEGER NOT NULL DEFAULT 0;

INSERT INTO schema_migration(version, applied_at) 
  VALUES ('0002', 0);
```

**影响**：
- 启用本地 HTTP 调试端点（`allow_localhost_http = 1`）
- 保存默认模型选择

**升级路径**：
- 旧库自动执行此迁移
- 新增列默认值安全，不丢失数据

---

## Tauri 命令集合

### 完整命令清单

| 命令 | 输入 | 输出 | 用途 |
|------|------|------|------|
| `list_chapters` | `novel_id: String` | `Vec<ChapterSnapshot>` | 查询章节列表 |
| `get_model_setup` | — | `Option<ModelSetupView>` | 读取密钥配置 |
| `save_model_setup` | `SaveModelSetupRequest` | `ModelSetupView` | 保存 API Key 到系统凭据 |
| `cancel_generation` | `run_id: String` | `()` | 中止生成 |
| `get_chapters_metadata` | — | `Vec<ChapterMetadata>` | 元数据同步 |
| `sync_chapter` | `chapter_id: String` | `Chapter` | 版本同步 |
| `open_run` | `OpenRunRequest` | `CommandOutcome` | 启动生成任务 |
| `commit_run` | `run_id, result` | `()` | 提交任务结果 |

### 错误处理

所有命令返回 `Result<T, AppError>`，客户端统一处理：

```rust
pub enum AppError {
    PortError(PortError),           // 端口/存储层错误
    RuntimeFault(String),            // 运行时异常
    InvalidInput(String),            // 参数验证失败
}
```

---

## 与既有 Node 服务的关系

### 不替换，并排

- Tauri 本地内核 **不替换** 既有 Express + Prisma Node 服务
- **并排运行**：Desktop UI 可同时调用本地 Rust 命令或远程 Node API
- **迁移路径**：未来可选逐步迁移业务逻辑到 Rust 层

### 当前调用路由

```
Desktop UI
    │
    ├─→ 本地命令：list_chapters, get_model_setup, cancel_generation
    │   (Tauri → Rust Kernel)
    │
    └─→ 云端 API：create_chapter, accept_chapter, ...
        (HTTP → Node Express)
```

---

## 下一步与扩展点

### 阶段 3：Desktop UI 适配（待实现）

1. **本地 vs 云创作模式选择**
   - 工作台新增模式选择器
   - 路由逻辑根据选择调用不同命令

2. **密钥管理 UI**
   - 系统凭据读写界面
   - 测试连接按钮

3. **生成进度与取消**
   - 实时进度显示
   - cancel_generation 集成

### 性能优化空间

- ✓ 已做：单线程事务、Keychain 集成
- ⚠ 待实测：包体积、内存占用、冷启动时间
- 📋 可研究：多线程池、缓存策略、增量更新

### 测试覆盖

- ✅ 40+ 单元、集成、恢复测试已通过
- 🔄 待补充：端对端 Tauri 命令测试（UI 侧）

---

## 关键文件导航

| 文件 | 用途 |
|------|------|
| `src-tauri/src/lib.rs` | Tauri 命令入口 |
| `crates/novel-application/src/` | 应用用例 |
| `crates/novel-domain/src/` | 领域模型 |
| `crates/novel-storage-sqlite/src/` | 数据库适配器 |
| `crates/novel-platform/src/` | 系统凭据实现 |
| `migrations/0002_model_setup.sql` | 迁移脚本 |

---

## 运行与调试

### 本地开发

```bash
# 编译 Rust 内核
cargo build --release

# 运行测试
cargo test --all

# Tauri 开发服务启动
cargo tauri dev
```

### 系统凭据调试

- **macOS**：Keychain 密钥保存在 `login` keychains，应用专属
- **开发环**：内存 MemoryStore（进程结束后清空）

---

## 责任边界与防守

### 应用层防守

- API Key 缺失时在 `ModelSecrets::api_key()` 返回 `Err`
- 不允许 null 或空字符串密钥通过

### 存储层防守

- 迁移脚本包含版本检查
- 旧表结构不破坏

### Tauri 边界防守

- 命令参数强类型验证（serde）
- 异步错误统一 `AppError` 映射

