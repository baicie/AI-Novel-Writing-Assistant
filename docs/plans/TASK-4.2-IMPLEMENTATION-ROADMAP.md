# Task 4.2 - 数据库集成 实现路线图

**版本**: 1.0  
**状态**: 🚀 **计划中**  
**计划开始**: 2026-10-29  
**预计完成**: 2026-11-02  
**工作量**: 4-5 天

---

## 一、Task 4.2 核心目标

### 1.1 功能目标

实现 **3 个 Repository**，建立完整的 **数据访问层**，支持：

- ✅ Novel 数据持久化 (创建、读、更新、删除)
- ✅ Chapter 数据管理
- ✅ GenerationTask 追踪
- ✅ 事务支持和一致性
- ✅ 查询优化和索引

### 1.2 质量目标

| 指标 | 目标 |
|-----|------|
| **Repository 数** | 3 个 |
| **测试用例** | 28+ 个 |
| **代码行数** | ~400 行 |
| **代码覆盖率** | > 85% |
| **测试通过率** | 100% |
| **查询性能** | < 50ms (p95) |

### 1.3 数据库选择

| 数据库 | 版本 | 用途 |
|--------|------|------|
| SQLite | 3.44+ | 本地存储 (Phase 4) |
| PostgreSQL | 14+ | 生产部署 (Phase 5+) |

---

## 二、3 个 Repository 详细规划

### 2.1 NovelRepository

**职责**: 管理故事数据的生命周期

**CRUD 操作**:

```rust
pub async fn create(&mut self, req: CreateNovelRequest) -> Result<Novel> {
    // 1. 验证输入
    // 2. 生成 UUID
    // 3. 插入数据库
    // 4. 返回完整对象
}

pub async fn find_by_id(&self, id: Uuid) -> Result<Option<Novel>> {
    // 1. 查询单条记录
    // 2. 反序列化
    // 3. 返回结果
}

pub async fn update(&mut self, id: Uuid, req: UpdateNovelRequest) -> Result<Novel> {
    // 1. 验证记录存在
    // 2. 部分更新支持
    // 3. 更新数据库
    // 4. 返回更新后的对象
}

pub async fn delete(&mut self, id: Uuid) -> Result<()> {
    // 1. 验证存在
    // 2. 标记删除或真删
    // 3. 返回结果
}

pub async fn list(&self, limit: i32, offset: i32) -> Result<Vec<Novel>> {
    // 1. 分页查询
    // 2. 按创建时间排序
    // 3. 返回列表
}
```

**测试计划**:
- create: 4 个测试 (正常、重复、无效、边界)
- find_by_id: 3 个测试 (存在、不存在、权限)
- update: 3 个测试 (全更新、部分更新、并发)
- delete: 2 个测试 (正常、不存在)
- list: 2 个测试 (分页、排序)

**总计**: 14 个测试

---

### 2.2 ChapterRepository

**职责**: 管理章节数据

**CRUD 操作**:

```rust
pub async fn create(&mut self, novel_id: Uuid, req: CreateChapterRequest) -> Result<Chapter> {
    // 1. 验证故事存在
    // 2. 创建章节记录
    // 3. 更新故事的章节计数
    // 4. 返回章节对象
}

pub async fn find_by_id(&self, id: Uuid) -> Result<Option<Chapter>> {
    // 按章节 ID 查询
}

pub async fn find_by_novel(&self, novel_id: Uuid) -> Result<Vec<Chapter>> {
    // 1. 查询该故事的所有章节
    // 2. 按序号排序
    // 3. 返回列表
}

pub async fn update(&mut self, id: Uuid, req: UpdateChapterRequest) -> Result<Chapter> {
    // 1. 验证存在
    // 2. 更新内容/状态
    // 3. 返回更新后对象
}

pub async fn update_content(&mut self, id: Uuid, content: String) -> Result<()> {
    // 只更新内容，保持其他字段
}

pub async fn update_status(&mut self, id: Uuid, status: ChapterStatus) -> Result<()> {
    // 只更新状态 (draft, review, published)
}

pub async fn delete(&mut self, id: Uuid) -> Result<()> {
    // 删除章节
}
```

**测试计划**:
- create: 3 个测试
- find_by_id: 2 个测试
- find_by_novel: 3 个测试
- update: 2 个测试
- update_content: 2 个测试
- update_status: 2 个测试
- delete: 2 个测试

**总计**: 14+ 个测试

---

### 2.3 GenerationTaskRepository

**职责**: 追踪生成任务的进度和状态

**CRUD 操作**:

```rust
pub async fn create(&mut self, req: CreateGenerationTaskRequest) -> Result<GenerationTask> {
    // 1. 创建任务记录
    // 2. 初始化状态为 "pending"
    // 3. 记录开始时间
    // 4. 返回任务对象
}

pub async fn find_by_id(&self, id: Uuid) -> Result<Option<GenerationTask>> {
    // 查询任务详情
}

pub async fn find_by_chapter(&self, chapter_id: Uuid) -> Result<Vec<GenerationTask>> {
    // 查询章节的所有生成任务
}

pub async fn update_status(&mut self, id: Uuid, status: TaskStatus) -> Result<()> {
    // 更新任务状态
    // status: pending, running, completed, failed, cancelled
}

pub async fn update_progress(&mut self, id: Uuid, token_count: i32) -> Result<()> {
    // 更新已生成的 token 数
}

pub async fn save_result(&mut self, id: Uuid, content: String) -> Result<()> {
    // 保存生成的内容
}

pub async fn save_error(&mut self, id: Uuid, error_msg: String) -> Result<()> {
    // 记录错误信息
}

pub async fn list_active(&self) -> Result<Vec<GenerationTask>> {
    // 查询所有进行中的任务
}
```

**测试计划**:
- create: 3 个测试
- find_by_id: 2 个测试
- find_by_chapter: 2 个测试
- update_status: 3 个测试
- update_progress: 2 个测试
- save_result: 2 个测试
- save_error: 2 个测试
- list_active: 2 个测试

**总计**: 18+ 个测试

---

## 三、数据库设计

### 3.1 Schema 定义

```sql
-- 故事表
CREATE TABLE novels (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    synopsis TEXT,
    target_length INTEGER,
    status TEXT DEFAULT 'draft',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_status (status),
    INDEX idx_created (created_at)
);

-- 章节表
CREATE TABLE chapters (
    id TEXT PRIMARY KEY,
    novel_id TEXT NOT NULL,
    chapter_number INTEGER NOT NULL,
    title TEXT,
    content TEXT,
    status TEXT DEFAULT 'draft',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (novel_id) REFERENCES novels(id),
    UNIQUE (novel_id, chapter_number),
    INDEX idx_novel (novel_id),
    INDEX idx_status (status)
);

-- 生成任务表
CREATE TABLE generation_tasks (
    id TEXT PRIMARY KEY,
    chapter_id TEXT NOT NULL,
    status TEXT DEFAULT 'pending',
    token_count INTEGER DEFAULT 0,
    max_tokens INTEGER,
    content TEXT,
    error_message TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    started_at TIMESTAMP,
    completed_at TIMESTAMP,
    FOREIGN KEY (chapter_id) REFERENCES chapters(id),
    INDEX idx_chapter (chapter_id),
    INDEX idx_status (status),
    INDEX idx_created (created_at)
);
```

### 3.2 迁移计划

```
migrations/
├── 001_create_novels.sql
├── 002_create_chapters.sql
├── 003_create_generation_tasks.sql
└── 004_create_indexes.sql
```

---

## 四、代码结构

### 4.1 新增文件结构

```
crates/novel-storage-sqlite/
├── src/
│   ├── repositories/           # Repository 层
│   │   ├── mod.rs
│   │   ├── novel_repository.rs
│   │   ├── chapter_repository.rs
│   │   └── generation_task_repository.rs
│   │
│   ├── models/                 # 数据模型
│   │   ├── mod.rs
│   │   ├── novel.rs
│   │   ├── chapter.rs
│   │   └── generation_task.rs
│   │
│   ├── db/                     # 数据库连接和迁移
│   │   ├── mod.rs
│   │   ├── connection.rs
│   │   ├── migration.rs
│   │   └── query_builder.rs
│   │
│   ├── migrations/             # SQL 迁移文件
│   │   ├── 001_create_novels.sql
│   │   ├── 002_create_chapters.sql
│   │   └── 003_create_generation_tasks.sql
│   │
│   ├── error.rs                # 错误类型
│   └── lib.rs
│
└── tests/                      # 集成测试
    ├── novel_repository_test.rs
    ├── chapter_repository_test.rs
    └── generation_task_repository_test.rs
```

### 4.2 核心模块实现

#### 模块 1: repositories/novel_repository.rs

```rust
pub struct NovelRepository {
    pool: SqlitePool,
}

impl NovelRepository {
    pub async fn create(&mut self, req: CreateNovelRequest) -> Result<Novel> {
        // ~40 行
    }
    // ... 其他方法 ~80 行
}
```

**代码行数**: ~120 行

#### 模块 2: repositories/chapter_repository.rs

```rust
pub struct ChapterRepository {
    pool: SqlitePool,
}

impl ChapterRepository {
    pub async fn create(&mut self, novel_id: Uuid, req: CreateChapterRequest) -> Result<Chapter> {
        // ~40 行
    }
    // ... 其他方法 ~100 行
}
```

**代码行数**: ~140 行

#### 模块 3: repositories/generation_task_repository.rs

```rust
pub struct GenerationTaskRepository {
    pool: SqlitePool,
}

impl GenerationTaskRepository {
    pub async fn create(&mut self, req: CreateGenerationTaskRequest) -> Result<GenerationTask> {
        // ~40 行
    }
    // ... 其他方法 ~100 行
}
```

**代码行数**: ~140 行

---

## 五、详细实现计划 (Day by Day)

### Day 1 (2026-10-29 周二)

**目标**: 建立数据库连接和迁移框架

**任务列表**:

- [ ] 09:00 - 晨会 (15 min)

- [ ] 09:15 - 创建 SQLx 连接和迁移 (1.5 hours)
  - [ ] src/db/connection.rs (连接池管理)
  - [ ] src/db/migration.rs (迁移执行)
  - [ ] 创建 migrations/ 目录
  - [ ] 编写 001_create_novels.sql
  - [ ] 编写 002_create_chapters.sql
  - [ ] 编写 003_create_generation_tasks.sql

- [ ] 10:45 - 创建数据模型 (1 hour)
  - [ ] src/models/novel.rs
  - [ ] src/models/chapter.rs
  - [ ] src/models/generation_task.rs

- [ ] 11:45 - 休息 (15 min)

- [ ] 12:00 - 数据库连接测试 (1 hour)
  - [ ] 测试连接池创建
  - [ ] 测试迁移执行
  - [ ] 测试表创建

- [ ] 13:00 - 午休 (1 hour)

- [ ] 14:00 - 创建 NovelRepository 框架 (1 hour)
  - [ ] src/repositories/novel_repository.rs 基础结构
  - [ ] 实现 create 方法

- [ ] 15:00 - NovelRepository 测试 (1 hour)
  - [ ] 创建单元测试文件
  - [ ] 测试 create 方法 (3 个测试)

- [ ] 16:00 - 提交 Day 1 (30 min)

**Day 1 代码行数预计**: ~150 行
**Day 1 测试用例预计**: 3 个 ✓

---

### Day 2 (2026-10-30 周三)

**目标**: 完成 NovelRepository 和 ChapterRepository

**任务列表**:

- [ ] 09:00 - 晨会 (15 min)

- [ ] 09:15 - 完成 NovelRepository (1.5 hours)
  - [ ] 实现 find_by_id, update, delete, list
  - [ ] 编写剩余 11 个测试

- [ ] 10:45 - 创建 ChapterRepository (1.5 hours)
  - [ ] src/repositories/chapter_repository.rs
  - [ ] 实现所有 CRUD 方法

- [ ] 12:15 - 午休 (1 hour)

- [ ] 13:15 - ChapterRepository 测试 (1.5 hours)
  - [ ] 编写 14+ 个测试

- [ ] 14:45 - 代码审查和验证 (1 hour)
  - [ ] 运行所有测试
  - [ ] 检查覆盖率

- [ ] 15:45 - 提交 Day 2 (15 min)

**Day 2 代码行数预计**: ~150 行
**Day 2 测试用例预计**: 25 个 ✓

---

### Day 3-4 (2026-10-31 ~ 2026-11-01)

**目标**: 完成 GenerationTaskRepository 和集成测试

**任务列表**:

- [ ] **Day 3 09:00 - 晨会 (15 min)**

- [ ] **Day 3 09:15 - GenerationTaskRepository 实现 (2 hours)**
  - [ ] 实现所有 8 个方法
  - [ ] 编写 18+ 个测试

- [ ] **Day 3 11:30 - 集成测试 (1.5 hours)**
  - [ ] 多 Repository 协作测试
  - [ ] 事务测试

- [ ] **Day 3 13:30 - 午休 + 代码审查 (1.5 hours)**

- [ ] **Day 3 15:00 - 性能优化 (1 hour)**
  - [ ] 索引验证
  - [ ] 查询优化
  - [ ] 基准测试

- [ ] **Day 4 - 最终验证**
  - [ ] 所有 28+ 测试通过
  - [ ] 覆盖率 > 85%
  - [ ] 文档完成
  - [ ] 准备合并

---

## 六、测试计划详表

### 6.1 单元测试分布 (28+ 个)

| Repository | 测试数 |
|-----------|--------|
| NovelRepository | 14 |
| ChapterRepository | 14+ |
| GenerationTaskRepository | 18+ |
| **总计** | **46+** |

（注：超过 28+ 的目标，表示充分覆盖）

### 6.2 测试覆盖范围

```
Repository 层测试:
  ✓ CRUD 操作完整性
  ✓ 错误处理
  ✓ 边界值处理
  ✓ 事务支持
  ✓ 并发安全

集成测试:
  ✓ 跨 Repository 操作
  ✓ 外键关系验证
  ✓ 级联删除
  ✓ 数据一致性
```

---

## 七、质量保证检查清单

### 7.1 代码质量

```
□ cargo fmt           ✓ 代码格式化通过
□ cargo clippy        ✓ 无警告
□ cargo test --all    ✓ 所有测试通过
□ cargo tarpaulin     ✓ 覆盖率 > 85%
□ sqlx verify         ✓ SQL 编译检查
```

### 7.2 数据库验证

```
□ 迁移执行成功
□ 表结构正确
□ 索引创建完成
□ 外键关系正确
□ 约束生效
```

### 7.3 性能验证

```
□ 查询性能 < 50ms (p95)
□ 插入性能 < 30ms (p95)
□ 更新性能 < 30ms (p95)
□ 连接池正常
□ 事务完整性
```

---

## 八、风险管理

| 风险 | 概率 | 影响 | 应对 |
|-----|------|------|------|
| 并发冲突 | 中 | 高 | 完整的并发测试 |
| 数据一致性 | 低 | 高 | 事务验证 |
| 性能瓶颈 | 中 | 中 | 索引优化 |

---

## 九、完成标准

**Task 4.2 完成标准**:

```
✅ 3 个 Repository 实现完成
✅ 46+ 个单元测试全部通过
✅ 代码覆盖率 > 85%
✅ 查询性能 < 50ms
✅ 数据库迁移验证通过
✅ API 文档完整
```

---

**准备就绪**: 🚀 **100% 准备**  
**启动时间**: 2026-10-29 09:00 AM  
**预期完成**: 2026-11-02 16:00 PM  
**下一阶段**: Task 4.3 (2026-11-05)

