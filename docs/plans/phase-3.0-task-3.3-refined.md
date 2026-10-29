# Task 3.3 修正实现计划

> 更新日期：2026-10-20  
> 基于：项目代码现状分析  
> 状态：重新规划中

## 现有架构评估

### 当前应用层架构
```
HTTP Routes
  ↓
Runtime (IOC 容器)
  ├─ DirectorService (运行管理)
  ├─ SetupService (小说和世界观管理)
  ├─ TaskCenterService (任务管理)
  ├─ EventHub (事件流)
  └─ Worker (工作流执行引擎)
  ↓
Repository Ports (6 个 trait)
  ├─ NovelRepository
  ├─ WorkflowRepository
  ├─ WorldRepository
  ├─ KnowledgeRepository
  ├─ LookupRepository
  └─ TaskCenterRepository
  ↓
SQLite Storage + Domain 层
```

### 代码统计
```
directive.rs:    522 行 - 运行生命周期管理
worker.rs:       786 行 - 工作流执行引擎
setup.rs:        133 行 - 小说和世界观设置
runtime.rs:       76 行 - 依赖注入配置
dto.rs:          451 行 - 数据传输对象
error.rs:         91 行 - 错误定义
knowledge.rs:     78 行 - 上下文组装
hub.rs:           29 行 - 事件总线
---
总计：2,166 行（不含港口定义）
```

## Task 3.3 修正范围

### 原始目标 vs. 实际情况

| 原始目标 | 现有情况 | 需要做什么 |
|---------|---------|---------|
| 创建小说 UseCase | SetupService + DirectorService | ✅ 已实现 |
| DTO 定义 | dto.rs 完整定义 | ✅ 已完成 |
| 事务管理 | Repository 层原子性 | 🔄 需要补强 |
| 错误映射 | error.rs 定义清晰 | ✅ 已完成 |
| 业务验证 | 散落在各服务中 | 🔄 需要提取 |

### 修正后的 Task 3.3 目标

#### 目标 1：应用层架构优化（2 天）
- [ ] 提取并整理现有业务逻辑
- [ ] 创建清晰的 UseCase 封装层
- [ ] 统一的错误处理和映射
- [ ] 完整的集成测试

#### 目标 2：事务边界明确化（1 天）
- [ ] 定义 Repository 级事务
- [ ] 应用层事务协调
- [ ] 错误回滚处理

#### 目标 3：文档和测试（1 天）
- [ ] 架构文档更新
- [ ] 集成测试补全
- [ ] 性能基准测试

## 具体实现步骤

### 第一天上午：代码整理和抽象（3-4 小时）

#### Step 1: 创建 UseCase 基类
```rust
// crates/novel-application/src/usecase.rs

pub trait UseCase: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
}

pub trait NovelUseCases {
    async fn create_novel(&self, input: CreateNovelInput) -> Result<NovelDetail, AppError>;
    async fn list_novels(&self, query: ListNovelsQuery) -> Result<ListNovelsResult, AppError>;
    async fn get_novel(&self, id: &str) -> Result<NovelDetail, AppError>;
    async fn update_novel(&self, id: &str, fields: UpdateNovelFields) -> Result<NovelDetail, AppError>;
    async fn delete_novel(&self, id: &str) -> Result<(), AppError>;
}

pub trait ChapterUseCases {
    async fn create_chapter(&self, novel_id: &str, input: NewChapter) -> Result<ChapterSnapshot, AppError>;
    async fn save_chapter(&self, input: SaveChapterRecord) -> Result<ChapterSnapshot, AppError>;
    async fn promote_chapter(&self, chapter_id: &str, candidate_id: &str) -> Result<PromoteOutcome, AppError>;
}

pub trait RunUseCases {
    async fn start_run(&self, input: OpenRunInput) -> Result<RunSummary, AppError>;
    async fn approve_run(&self, run_id: &str, command_id: &str) -> Result<RunSummary, AppError>;
}
```

#### Step 2: 从 DirectorService 中提取小说 UseCase
```rust
// crates/novel-application/src/services/novel_usecases.rs

impl NovelUseCases for DirectorService {
    async fn create_novel(&self, input: CreateNovelInput) -> Result<NovelDetail, AppError> {
        // 验证输入
        self.validate_create_novel_input(&input)?;
        
        // 调用 Repository
        let now_ms = self.clock.now_ms();
        let novel_id = self.ids.new_id();
        
        let novel = self.novels.create_novel_with_setup(input).await?;
        Ok(novel)
    }
    
    async fn list_novels(&self, query: ListNovelsQuery) -> Result<ListNovelsResult, AppError> {
        // 现有实现迁移
        self.novels.list_novels_paginated(&query).await
    }
    
    // ... 其他方法
}
```

#### Step 3: 从 SetupService 中提取世界观 UseCase
```rust
// crates/novel-application/src/services/world_usecases.rs

impl WorldUseCases for SetupService {
    async fn create_world(&self, input: CreateWorldInput) -> Result<World, AppError> {
        // 现有实现已很完整，只需包装
        self.create_world(
            &input.name,
            &input.description,
            &input.overview_summary,
            input.source_template_id,
            input.owner_novel_id,
        ).await
    }
}
```

### 第一天下午：事务管理和错误处理（3-4 小时）

#### Step 4: 定义事务管理器
```rust
// crates/novel-application/src/transaction.rs

pub trait TransactionManager {
    async fn with_transaction<F, T>(&self, f: F) -> Result<T, AppError>
    where
        F: FnOnce() -> BoxFuture<'static, Result<T, AppError>> + Send,
        T: Send,
    ;
}

pub struct NoTransactionManager;

impl TransactionManager for NoTransactionManager {
    async fn with_transaction<F, T>(&self, f: F) -> Result<T, AppError> {
        f().await // SQLite 会自动处理事务
    }
}
```

#### Step 5: 统一错误映射
```rust
// 在 error.rs 中补充映射

impl From<PortError> for AppError {
    fn from(err: PortError) -> Self {
        match err {
            PortError::NotFound(msg) => AppError::not_found(msg),
            PortError::Conflict(msg) => AppError::conflict(msg),
            PortError::Internal(msg) => AppError::internal(msg),
        }
    }
}
```

### 第二天上午：测试和验证（3-4 小时）

#### Step 6: 创建集成测试
```rust
// tests/integration_tests.rs

#[tokio::test]
async fn test_create_novel_use_case() {
    let runtime = setup_test_runtime().await;
    
    let input = CreateNovelInput {
        title: "Test Novel".to_string(),
        // ... 其他字段
    };
    
    let result = runtime.create_novel(input).await;
    assert!(result.is_ok());
    
    let novel = result.unwrap();
    assert_eq!(novel.title, "Test Novel");
}
```

#### Step 7: 性能基准测试
```bash
cargo bench --package novel-application --lib usecase
```

### 第二天下午：文档和最终检查（2-3 小时）

#### Step 8: 更新架构文档
- [ ] UseCase 接口文档
- [ ] 事务管理说明
- [ ] 错误处理指南

#### Step 9: 代码审查和优化
- [ ] Clippy 检查
- [ ] 编译检查
- [ ] 测试覆盖验证

## 预期交付物

### 代码文件
```
crates/novel-application/src/
├─ usecase.rs          (新) - UseCase trait 定义
├─ transaction.rs      (新) - 事务管理
├─ services/
│  ├─ novel.rs        (新) - 小说 UseCase 实现
│  ├─ chapter.rs      (新) - 章节 UseCase 实现
│  ├─ run.rs          (新) - 运行 UseCase 实现
│  └─ world.rs        (新) - 世界观 UseCase 实现
└─ (其他更新)
```

### 文档文件
```
docs/design/
├─ phase-3.0-task-3.3-refined.md (修正设计)
└─ usecase-guide.md             (UseCase 使用指南)

docs/wiki/architecture/
└─ application-layer.md        (应用层架构文档)
```

### 测试文件
```
tests/
├─ novel_usecase_tests.rs
├─ chapter_usecase_tests.rs
├─ run_usecase_tests.rs
└─ integration_tests.rs
```

## 质量目标

```
编译：✅ cargo build 通过
类型：✅ cargo clippy 无警告
测试：✅ cargo test 15+ 新测试通过
覆盖：✅ 关键路径覆盖率 > 80%
文档：✅ 所有 UseCase 接口有文档
```

## 风险和缓解

| 风险 | 概率 | 缓解方案 |
|-----|------|---------|
| 重构引入回归 | 中 | 保持现有实现，只提取接口 |
| 性能下降 | 低 | 基准测试和profiling |
| 事务死锁 | 低 | SQLite 内置锁管理，充分测试 |

## 最终状态

Task 3.3 完成后：
- ✅ 清晰的 UseCase 抽象层
- ✅ 完整的事务管理
- ✅ 统一的错误映射
- ✅ > 15 个集成测试
- ✅ 完整的架构文档
- ✅ 可进入 Task 3.4（LLM Provider）

---

**建议立即启动：第一步创建 UseCase 基类**
