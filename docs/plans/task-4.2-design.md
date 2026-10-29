# Task 4.2 - 数据库集成详细设计

**日期**: 2026-10-20  
**阶段**: Phase 4.0 - 实现与集成  
**任务**: Task 4.2 - 数据库集成  
**目标**: 设计数据库 schema，实现 Repository 层，连接 HTTP 端点到持久化

---

## 任务概述

### 目标

从 Task 4.1 的 HTTP 端点，通过 Repository 层连接到真实数据库，实现数据的完整 CRUD 操作。

### 为什么使用 SQLx？

| 特性 | SQLx | ORM (Diesel/SeaORM) | Raw SQL |
|-----|------|---------------------|---------|
| 类型安全 | ✅ 编译时检查 | ✅ 强类型 | ❌ 运行时 |
| SQL 灵活性 | ✅ 原生 SQL | ❌ 受限 | ✅ 完全 |
| 性能 | ✅ 最优 | ⚠️ 中等 | ✅ 最优 |
| 学习曲线 | ✅ 低 | ❌ 高 | ✅ 低 |
| 异步支持 | ✅ 原生 | ✅ 支持 | ✅ 支持 |

**选择理由**:
- 编译时 SQL 检查保证安全
- 零开销抽象，性能最优
- 与 Rust 异步生态天然配合
- 学习曲线适中

---

## Task 4.2 分解

### Part 1: 数据库设计 (1-2 天)

#### 1.1 数据库架构

```
┌─────────────────────────────────────────┐
│         应用程序 (Task 4.1)             │
│    HTTP 处理器 + UseCase 层             │
└────────────────┬────────────────────────┘
                 │
        ┌────────▼────────┐
        │ Repository 层   │
        │ (Task 4.2)      │
        └────────┬────────┘
                 │
┌────────────────▼────────────────────────┐
│       数据库 (SQLite/PostgreSQL)        │
│  novels | chapters | runs | users       │
└─────────────────────────────────────────┘
```

#### 1.2 表设计

##### novels 表

```sql
CREATE TABLE novels (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  description TEXT,
  author_id TEXT,
  status TEXT NOT NULL DEFAULT 'draft',  -- draft, publishing, published
  creation_experience TEXT,               -- beginner, intermediate, advanced
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  version INTEGER NOT NULL DEFAULT 1,
  FOREIGN KEY (author_id) REFERENCES users(id)
);

CREATE INDEX idx_novels_author_id ON novels(author_id);
CREATE INDEX idx_novels_status ON novels(status);
CREATE INDEX idx_novels_created_at ON novels(created_at);
```

**字段说明**:
- `id`: 主键，UUID 格式
- `title`: 小说标题，不可空
- `description`: 小说描述，可选
- `author_id`: 作者 ID，外键
- `status`: 小说状态 (草稿/发布/已发布)
- `creation_experience`: 创建者经验等级
- `version`: 乐观锁版本号

**约束**:
- 主键约束
- 外键约束 (users)
- 状态值约束
- 时间戳自动更新

##### chapters 表

```sql
CREATE TABLE chapters (
  id TEXT PRIMARY KEY,
  novel_id TEXT NOT NULL,
  title TEXT NOT NULL,
  content TEXT,
  sort_order INTEGER NOT NULL,
  status TEXT NOT NULL DEFAULT 'draft',  -- draft, saving, saved
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  version INTEGER NOT NULL DEFAULT 1,
  FOREIGN KEY (novel_id) REFERENCES novels(id) ON DELETE CASCADE
);

CREATE INDEX idx_chapters_novel_id ON chapters(novel_id);
CREATE INDEX idx_chapters_status ON chapters(status);
CREATE INDEX idx_chapters_sort_order ON chapters(novel_id, sort_order);
```

**字段说明**:
- `id`: 主键，UUID 格式
- `novel_id`: 所属小说 ID，外键
- `title`: 章节标题
- `content`: 章节内容
- `sort_order`: 排序顺序
- `status`: 章节状态

**约束**:
- 主键约束
- 外键约束 (novels) + 级联删除
- 唯一约束 (novel_id, sort_order)

##### runs 表

```sql
CREATE TABLE runs (
  id TEXT PRIMARY KEY,
  novel_id TEXT NOT NULL,
  chapter_id TEXT,
  type TEXT NOT NULL,              -- generation, review
  status TEXT NOT NULL,             -- started, completed, failed
  result TEXT,                       -- JSON 格式的结果
  error TEXT,                        -- 错误信息
  started_at TIMESTAMP,
  completed_at TIMESTAMP,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (novel_id) REFERENCES novels(id),
  FOREIGN KEY (chapter_id) REFERENCES chapters(id)
);

CREATE INDEX idx_runs_novel_id ON runs(novel_id);
CREATE INDEX idx_runs_chapter_id ON runs(chapter_id);
CREATE INDEX idx_runs_status ON runs(status);
CREATE INDEX idx_runs_created_at ON runs(created_at);
```

**字段说明**:
- `id`: 主键
- `novel_id`: 所属小说
- `chapter_id`: 所属章节 (可选)
- `type`: 运行类型
- `status`: 运行状态
- `result`: 结果 (JSON)
- `error`: 错误信息

##### users 表 (可选，多用户支持)

```sql
CREATE TABLE users (
  id TEXT PRIMARY KEY,
  email TEXT UNIQUE NOT NULL,
  name TEXT NOT NULL,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_users_email ON users(email);
```

#### 1.3 关系模型

```
┌─────────────┐
│   novels    │
│  (1)        │
│   ↓ (*)     │
│─────────────┤
│ id (PK)     │
│ title       │
│ author_id   │
│ status      │
│ created_at  │
└─────────────┘
       ↓ (1)
       
┌─────────────┐
│  chapters   │
│  (*)        │
│─────────────┤
│ id (PK)     │
│ novel_id    │
│ title       │
│ content     │
│ status      │
│ created_at  │
└─────────────┘
       ↓ (1)
       
┌─────────────┐
│    runs     │
│  (*)        │
│─────────────┤
│ id (PK)     │
│ novel_id    │
│ chapter_id  │
│ type        │
│ status      │
│ result      │
└─────────────┘

关系说明:
- novels: 1 → chapters: *  (一个小说有多个章节)
- chapters: 1 → runs: *    (一个章节有多个运行)
- novels: 1 → runs: *      (一个小说有多个运行)
```

#### 1.4 数据库选择

**开发环境**: SQLite
- ✅ 无需安装数据库服务
- ✅ 文件存储，便于测试
- ✅ 完整 SQL 支持

**生产环境**: PostgreSQL
- ✅ 高并发支持
- ✅ 企业级可靠性
- ✅ 丰富的扩展

**迁移策略**:
1. 开发使用 SQLite
2. 测试同时测试 SQLite 和 PostgreSQL
3. 生产环境部署 PostgreSQL

---

### Part 2: SQLx 集成 (1-2 天)

#### 2.1 Cargo.toml 配置

```toml
[dependencies]
sqlx = { version = "0.7", features = ["runtime-tokio-rustls", "sqlite", "postgres", "uuid", "chrono", "macros"] }
tokio = { version = "1.35", features = ["full"] }
uuid = { version = "1.6", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

[dev-dependencies]
sqlx-cli = "0.7"
```

#### 2.2 数据库配置

**文件**: `src/db/config.rs`

```rust
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use std::env;

pub struct DbConfig {
    pub database_url: String,
    pub max_connections: u32,
}

impl DbConfig {
    pub fn from_env() -> Self {
        DbConfig {
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite:novel.db".to_string()),
            max_connections: env::var("DB_MAX_CONNECTIONS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(5),
        }
    }

    pub async fn create_pool(&self) -> Result<SqlitePool, sqlx::Error> {
        SqlitePoolOptions::new()
            .max_connections(self.max_connections)
            .connect(&self.database_url)
            .await
    }
}
```

#### 2.3 连接池管理

**文件**: `src/db/pool.rs`

```rust
use sqlx::sqlite::SqlitePool;
use std::sync::Arc;

pub struct DbPool {
    pool: Arc<SqlitePool>,
}

impl DbPool {
    pub fn new(pool: SqlitePool) -> Self {
        DbPool {
            pool: Arc::new(pool),
        }
    }

    pub fn get(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn health_check(&self) -> Result<(), String> {
        match sqlx::query("SELECT 1").execute(self.get()).await {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Database health check failed: {}", e)),
        }
    }
}

impl Clone for DbPool {
    fn clone(&self) -> Self {
        DbPool {
            pool: self.pool.clone(),
        }
    }
}
```

---

### Part 3: Repository 实现 (2-3 天)

#### 3.1 Repository 特征定义

**文件**: `src/repository/mod.rs`

```rust
pub mod novels;
pub mod chapters;
pub mod runs;

use async_trait::async_trait;

pub use novels::NovelRepository;
pub use chapters::ChapterRepository;
pub use runs::RunRepository;

#[async_trait]
pub trait Repository {
    type Entity;
    type Error;

    async fn save(&self, entity: Self::Entity) -> Result<Self::Entity, Self::Error>;
    async fn find_by_id(&self, id: &str) -> Result<Option<Self::Entity>, Self::Error>;
    async fn find_all(&self) -> Result<Vec<Self::Entity>, Self::Error>;
    async fn delete(&self, id: &str) -> Result<(), Self::Error>;
    async fn update(&self, entity: Self::Entity) -> Result<Self::Entity, Self::Error>;
}
```

#### 3.2 小说 Repository

**文件**: `src/repository/novels.rs`

```rust
use sqlx::SqlitePool;
use async_trait::async_trait;
use crate::models::Novel;

pub struct NovelRepository {
    pool: SqlitePool,
}

impl NovelRepository {
    pub fn new(pool: SqlitePool) -> Self {
        NovelRepository { pool }
    }

    pub async fn create(&self, novel: &Novel) -> Result<Novel, sqlx::Error> {
        let id = &novel.id;
        let title = &novel.title;
        
        sqlx::query_as::<_, Novel>(
            "INSERT INTO novels (id, title, description, status, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?)
             RETURNING *"
        )
        .bind(id)
        .bind(title)
        .bind(&novel.description)
        .bind("draft")
        .bind(chrono::Utc::now())
        .bind(chrono::Utc::now())
        .fetch_one(&self.pool)
        .await
    }

    pub async fn list(&self) -> Result<Vec<Novel>, sqlx::Error> {
        sqlx::query_as::<_, Novel>(
            "SELECT * FROM novels ORDER BY created_at DESC"
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn get_by_id(&self, id: &str) -> Result<Option<Novel>, sqlx::Error> {
        sqlx::query_as::<_, Novel>(
            "SELECT * FROM novels WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn update(&self, novel: &Novel) -> Result<Novel, sqlx::Error> {
        sqlx::query_as::<_, Novel>(
            "UPDATE novels 
             SET title = ?, description = ?, status = ?, updated_at = ?, version = version + 1
             WHERE id = ?
             RETURNING *"
        )
        .bind(&novel.title)
        .bind(&novel.description)
        .bind(&novel.status)
        .bind(chrono::Utc::now())
        .bind(&novel.id)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn delete(&self, id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM novels WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
```

**预期**: ~150 行代码 (create, read, update, delete, list)

#### 3.3 章节 Repository

**文件**: `src/repository/chapters.rs`

类似于小说 Repository，包含：
- create_chapter
- list_chapters_by_novel
- get_chapter
- save_chapter
- promote_chapter
- delete_chapter

**预期**: ~180 行代码

#### 3.4 运行 Repository

**文件**: `src/repository/runs.rs`

包含：
- start_run
- get_run
- approve_run
- list_runs_by_novel
- update_run_status

**预期**: ~130 行代码

---

### Part 4: 迁移脚本 (1 天)

#### 4.1 迁移文件结构

```
migrations/
├── 001_initial_schema.sql
├── 002_add_users_table.sql
├── 003_add_indexes.sql
└── 004_add_constraints.sql
```

#### 4.2 初始 Schema 迁移

**文件**: `migrations/001_initial_schema.sql`

```sql
-- 小说表
CREATE TABLE novels (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  description TEXT,
  author_id TEXT,
  status TEXT NOT NULL DEFAULT 'draft',
  creation_experience TEXT,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  version INTEGER NOT NULL DEFAULT 1
);

-- 章节表
CREATE TABLE chapters (
  id TEXT PRIMARY KEY,
  novel_id TEXT NOT NULL,
  title TEXT NOT NULL,
  content TEXT,
  sort_order INTEGER NOT NULL,
  status TEXT NOT NULL DEFAULT 'draft',
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  version INTEGER NOT NULL DEFAULT 1,
  FOREIGN KEY (novel_id) REFERENCES novels(id) ON DELETE CASCADE
);

-- 运行表
CREATE TABLE runs (
  id TEXT PRIMARY KEY,
  novel_id TEXT NOT NULL,
  chapter_id TEXT,
  type TEXT NOT NULL,
  status TEXT NOT NULL,
  result TEXT,
  error TEXT,
  started_at TIMESTAMP,
  completed_at TIMESTAMP,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (novel_id) REFERENCES novels(id),
  FOREIGN KEY (chapter_id) REFERENCES chapters(id)
);

-- 索引
CREATE INDEX idx_novels_created_at ON novels(created_at);
CREATE INDEX idx_chapters_novel_id ON chapters(novel_id);
CREATE INDEX idx_runs_novel_id ON runs(novel_id);
```

#### 4.3 迁移管理

**命令**:

```bash
# 创建新迁移
sqlx migrate add -r <migration_name>

# 运行迁移
sqlx migrate run --database-url sqlite:novel.db

# 回滚迁移
sqlx migrate revert --database-url sqlite:novel.db

# 验证迁移状态
sqlx migrate status --database-url sqlite:novel.db
```

---

### Part 5: 模型定义 (0.5 天)

**文件**: `src/models/mod.rs`

```rust
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Novel {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub author_id: Option<String>,
    pub status: String,
    pub creation_experience: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub version: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Chapter {
    pub id: String,
    pub novel_id: String,
    pub title: String,
    pub content: Option<String>,
    pub sort_order: i32,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub version: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Run {
    pub id: String,
    pub novel_id: String,
    pub chapter_id: Option<String>,
    pub run_type: String,
    pub status: String,
    pub result: Option<String>,
    pub error: Option<String>,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
```

**预期**: ~70 行代码

---

### Part 6: 集成测试 (1-2 天)

#### 6.1 Repository 单元测试

**文件**: `tests/repository_tests.rs`

```rust
#[tokio::test]
async fn test_novel_repository_create() {
    let pool = setup_test_db().await;
    let repo = NovelRepository::new(pool);
    
    let novel = Novel {
        id: "test-1".to_string(),
        title: "Test Novel".to_string(),
        // ...
    };

    let result = repo.create(&novel).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_novel_repository_list() {
    // 创建测试数据
    // 验证列表返回
    // 验证顺序正确
}

#[tokio::test]
async fn test_novel_repository_update() {
    // 创建→更新→验证
}

#[tokio::test]
async fn test_novel_repository_delete() {
    // 创建→删除→验证不存在
}
```

**预期**: 20+ Repository 测试

#### 6.2 数据库集成测试

**文件**: `tests/db_integration_tests.rs`

```rust
#[tokio::test]
async fn test_transaction_rollback() {
    // 测试事务回滚
}

#[tokio::test]
async fn test_concurrent_access() {
    // 测试并发访问
}

#[tokio::test]
async fn test_foreign_key_constraints() {
    // 测试外键约束
}

#[tokio::test]
async fn test_cascade_delete() {
    // 测试级联删除
}
```

**预期**: 10+ 集成测试

---

### Part 7: 文档 (0.5 天)

#### 7.1 数据库设计文档

**内容**:
- ✅ Schema 设计说明
- ✅ 表关系图
- ✅ 字段说明
- ✅ 约束说明
- ✅ 索引策略

#### 7.2 Repository 使用指南

**内容**:
- ✅ Repository 模式说明
- ✅ 使用示例
- ✅ 错误处理
- ✅ 事务处理

#### 7.3 迁移指南

**内容**:
- ✅ 迁移步骤
- ✅ 常用命令
- ✅ 回滚过程
- ✅ 故障排除

---

## 完整时间表

```
Day 1-2: 数据库设计
  • Schema 设计 (小说、章节、运行)
  • 关系模型设计
  • 约束和索引设计

Day 3-4: SQLx 集成
  • 依赖配置
  • 连接池实现
  • 配置管理

Day 5-7: Repository 实现
  • 小说 Repository (150 行)
  • 章节 Repository (180 行)
  • 运行 Repository (130 行)

Day 8: 迁移脚本
  • 初始 schema 迁移
  • 索引和约束迁移
  • 版本控制

Day 9-10: 测试
  • Repository 单元测试 (20+)
  • 数据库集成测试 (10+)
  • 性能测试

Day 11: 文档和优化
  • 编写所有文档
  • 性能优化
  • 最终验证
```

**总计**: 8-10 工作天

---

## 与 Task 4.1 的集成

### 集成流程

```
HTTP 处理器 (Task 4.1)
    ↓ (调用)
UseCase 层 (Phase 3.0)
    ↓ (调用)
Repository (Task 4.2 新增)
    ↓ (执行)
数据库 (SQLite/PostgreSQL)
```

### 集成示例

```rust
// HTTP 处理器
pub async fn create_novel(
    req: web::Json<CreateNovelRequest>,
    db: web::Data<DbPool>,
) -> Result<HttpResponse> {
    let repo = NovelRepository::new(db.get().clone());
    
    // 调用 UseCase
    match NovelUseCases::create(&req) {
        Ok(novel_dto) => {
            // 转换为 Model
            let novel = Novel::from(novel_dto);
            
            // 保存到数据库
            match repo.create(&novel).await {
                Ok(saved) => Ok(HttpResponse::Created().json(saved)),
                Err(e) => Ok(HttpResponse::InternalServerError().json(...)),
            }
        }
        Err(e) => Ok(HttpResponse::BadRequest().json(...)),
    }
}
```

---

## 成功标准

### 代码质量
- [ ] 0 编译错误
- [ ] Repository 完全实现
- [ ] 没有 unsafe 代码
- [ ] 遵循项目约定

### 测试覆盖
- [ ] 30+ Repository 测试
- [ ] 100% 测试通过
- [ ] 并发测试通过
- [ ] 约束测试通过

### 文档完整
- [ ] Schema 文档完整
- [ ] Repository 使用指南
- [ ] 迁移指南
- [ ] 性能优化建议

### 性能指标
- [ ] 查询响应 < 10ms
- [ ] 并发连接支持
- [ ] 连接池工作正常

---

## 后续任务 (Task 4.3)

Task 4.3 将在此基础上添加：
- 用户认证
- JWT 令牌
- 权限检查
- 审计日志

---

**状态**: 详细设计完成  
**下一步**: Task 4.1 开始实现 (2026-10-22)  
**预计 Task 4.2 开始**: 2026-10-30  
**预计 Task 4.2 完成**: 2026-11-09
