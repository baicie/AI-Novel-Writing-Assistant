# 桌面本地创作内核架构

## 概述

本地创作内核是 Tauri 桌面应用里与 Node 服务并排开发的 Rust 内核，由 `crates/` 下的多个 crate 分层组成，覆盖最小的单章生成闭环：小说与章节读写、运行与恢复、系统凭据保存模型密钥。

它不是桌面应用的产品后端。自动导演、章节生产、Prompt、RAG、任务中心等全部产品能力，由安装包内置的 Node 服务（sidecar）提供，运行合同见 [Tauri 桌面壳与 Node 本地服务 sidecar](./tauri-node-sidecar.md)。桌面壳目前给内核接的是 `UnconfiguredProvider`，本地生成还没有接入真实模型。

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
- **命令清单**：见下文“Tauri 命令集合”。壳层的启动快照、日志目录和重启命令属于 Node sidecar 管理，不经过内核。
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

命令注册在 `src-tauri/src/lib.rs`。每个命令都要同时出现在 `src-tauri/build.rs` 的命令清单和 `src-tauri/capabilities/default.json` 的权限里，否则前端调用会被权限层拒绝。

| 命令 | 用途 |
|------|------|
| `list_novels` / `create_novel` | 小说列表与新建 |
| `list_chapters` / `create_chapter` / `get_chapter` / `save_chapter` | 章节读写，保存时校验版本 |
| `open_run` | 打开一次单章生成运行 |
| `approve_run` / `cancel_run` / `resume_run` | 确认、停止、恢复运行 |
| `cancel_generation` | 中止正在进行的模型调用 |
| `replay_run` / `subscribe_run` | 补读与订阅运行事件 |
| `get_model_setup` / `save_model_setup` / `has_api_key` | 模型配置与系统凭据 |

### 错误处理

内核命令返回 `Result<T, AppError>`。`AppError` 序列化为 `{ code, message, retryable, details }`，`code` 取值为 `VALIDATION`、`CONFLICT`、`NOT_FOUND`、`CANCELLED`、`MODEL_RATE_LIMIT`、`STORAGE_CORRUPT`、`INTEGRITY_ERROR`，前端类型见 `client/src/platform/desktopTransport.ts`。

---

## 与既有 Node 服务的关系

### 不替换，并排

- Tauri 本地内核 **不替换** 既有 Express + Prisma Node 服务。
- Tauri 安装包内置 Node 运行时和编译后的 server，由 Rust 壳层在本机随机端口启动；桌面界面的全部产品功能都走这条 HTTP 链路。
- **迁移路径**：按模块把对应 HTTP 接口逐个迁到 Rust；迁移完成前，Node 服务仍是产品主链路。

### 当前调用路由

```
Desktop UI
    │
    ├─→ 产品功能（自动导演、章节生产、设置、任务中心……）
    │   HTTP → 安装包内置的 Node 服务（127.0.0.1:<每次启动分配的端口>）
    │
    ├─→ 本地创作页：list_novels、list_chapters、open_run……
    │   Tauri IPC → Rust 内核
    │
    └─→ 启动页：get_desktop_bootstrap_snapshot、open_desktop_logs_directory、restart_desktop_app
        Tauri IPC → Rust 壳层（管理 Node 服务）
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
| `src-tauri/src/node_backend/` | 内置 Node 服务的启动、健康检查与启动快照 |
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

