# Phase 3.0 Task 3.3：应用层用例与事务边界

> 任务代号：Task 3.3  
> 标题：应用层用例与事务边界设计  
> 预计工期：2 天  
> 优先级：高（Task 3.2 的直接后续）  
> 状态：设计中

## 任务背景

### 前置完成
- ✅ Task 3.1：Domain 层实体（7 个实体，13 测试通过）
- ✅ Task 3.2：持久化层（6 Repository trait，4 集成测试通过）

### 当前状态
- Domain 层：业务规则已定义，实体完整
- Ports 层：Repository trait 已定义
- Storage 层：SQLite 实现完成

### 缺失的部分（Task 3.3 要解决）
- ❌ UseCase 类（应用服务）
- ❌ DTO 对象（数据传输）
- ❌ 事务管理
- ❌ 业务验证逻辑
- ❌ 错误映射链

## 设计目标

### 主要目标
1. **应用服务层** - 连接 Domain 和外部（UI、API）
2. **事务边界** - 定义一致性单位
3. **业务验证** - Domain 规则的应用层执行
4. **错误处理** - 统一的错误映射
5. **DTO 转换** - Domain 与外部数据映射

### 次要目标
1. 减少 UI 层复杂度
2. 提供清晰的操作界面
3. 支持批量操作
4. 记录操作审计

## 核心设计

### UseCase 层架构

```
UI / HTTP Route
    ↓
UseCase (应用层)
    ├─ 输入验证
    ├─ 业务检查
    ├─ 调用 Repository
    ├─ 事务管理
    ├─ 结果映射
    └─ 输出 DTO
    ↓
Domain + Storage
```

### 事务管理模式

```
BEGIN TRANSACTION
  ├─ 验证输入 (ApplicationError)
  ├─ 检查前置条件 (DomainError)
  ├─ 执行操作 (Repository)
  ├─ 验证后置条件
  └─ COMMIT / ROLLBACK
     ↓
  返回 DTO 或 错误
```

### 错误处理链

```
DomainError
    ↓ 映射
ApplicationError
    ↓ 映射
AppError (HTTP 400/500)
    ↓
Client (JSON 错误响应)
```

## 实现范围

### 优先级 1：核心 UseCase

#### 小说管理
1. **CreateNovelUseCase**
   - 输入：CreateNovelInput (title, description, mode, etc.)
   - 处理：验证 + 创建 Novel + 初始化 Chapter
   - 输出：NovelDetail DTO
   - 事务：是

2. **GetNovelUseCase**
   - 输入：novelId
   - 处理：查询 + 组装完整信息
   - 输出：NovelDetail DTO
   - 事务：否

3. **ListNovelsUseCase**
   - 输入：ListNovelsQuery (page, limit, filters)
   - 处理：分页查询 + 过滤
   - 输出：ListNovelsResult (items + pagination)
   - 事务：否

4. **UpdateNovelUseCase**
   - 输入：novelId + UpdateNovelFields
   - 处理：验证 + 更新 + 版本检查
   - 输出：NovelDetail DTO
   - 事务：是

5. **DeleteNovelUseCase**
   - 输入：novelId
   - 处理：验证级联 + 删除
   - 输出：()
   - 事务：是

#### 章节管理
1. **CreateChapterUseCase**
   - 输入：novelId + title + sort_order
   - 处理：验证 + 创建 + 排序调整
   - 输出：ChapterSnapshot DTO
   - 事务：是

2. **SaveChapterUseCase**
   - 输入：SaveChapterRecord (novelId, chapterId, content, etc.)
   - 处理：验证 + 保存 + 版本管理
   - 输出：ChapterSnapshot DTO
   - 事务：是

3. **PromoteChapterUseCase**
   - 输入：chapterId + candidateId
   - 处理：验证规则 + 提升候选为正式版
   - 输出：PromoteOutcome (Promoted | Blocked | Missing)
   - 事务：是

#### 工作流管理
1. **StartRunUseCase**
   - 输入：OpenRunInput (novelId, contract, etc.)
   - 处理：创建运行 + 排队
   - 输出：RunSummary DTO
   - 事务：是

2. **ApproveRunUseCase**
   - 输入：runId + commandId
   - 处理：验证状态 + 更新控制 + 触发恢复
   - 输出：RunSummary DTO
   - 事务：是

### 优先级 2：DTO 定义

#### 输入 DTO
```rust
pub struct CreateNovelInput {
    pub title: String,
    pub description: String,
    pub narrative_form: String,
    pub writing_mode: String,
    pub project_mode: String,
}

pub struct SaveChapterRecord {
    pub novel_id: String,
    pub chapter_id: String,
    pub title: String,
    pub content: String,
    pub revision_id: String,
}

pub struct OpenRunInput {
    pub novel_id: String,
    pub command_id: String,
    pub contract_json: String,
    // ... 其他字段
}
```

#### 输出 DTO
```rust
pub struct NovelDetail {
    pub id: String,
    pub title: String,
    pub chapters: Vec<ChapterSnapshot>,
    pub status: String,
    pub created_at: i64,
}

pub struct ChapterSnapshot {
    pub id: String,
    pub novel_id: String,
    pub title: String,
    pub content: Option<String>,
    pub status: String,
}

pub struct RunSummary {
    pub run_id: String,
    pub novel_id: String,
    pub status: String,
    pub progress: i32,
}
```

### 优先级 3：事务管理

#### 事务模式
```rust
pub trait TransactionManager {
    async fn transaction<F, T>(&self, f: F) -> Result<T, AppError>
    where
        F: Fn() -> BoxFuture<'static, Result<T, AppError>>,
    ;
}
```

#### 事务使用
```rust
impl CreateNovelUseCase {
    async fn execute(&self, input: CreateNovelInput) -> Result<NovelDetail> {
        self.tx_manager.transaction(|| {
            Box::new(async {
                // 1. 验证
                self.validate_input(&input)?;
                
                // 2. 操作
                let novel = self.novel_repo.create_novel(
                    NewNovel { /* ... */ }
                ).await?;
                
                // 3. 返回
                self.map_to_dto(&novel).await
            })
        }).await
    }
}
```

## 实现计划

### 第一天（2026-10-21）

#### 上午（3 小时）
1. 定义 UseCase base class / trait (30 min)
2. 定义所有 DTO 结构 (1 hour)
3. 实现 CreateNovelUseCase (1.5 hours)

#### 下午（3 小时）
1. 实现 GetNovelUseCase (30 min)
2. 实现 ListNovelsUseCase (1 hour)
3. 实现 UpdateNovelUseCase (1.5 hours)

**目标：Novel 管理 UseCase 完成**

### 第二天（2026-10-22）

#### 上午（3 小时）
1. 实现 Chapter UseCase (3 个：Create、Save、Promote) (2 hours)
2. 事务管理实现 (1 hour)

#### 下午（2 小时）
1. UseCase 集成测试 (1 hour)
2. 错误映射验证 (30 min)
3. 最终提交准备 (30 min)

**目标：Task 3.3 完成，4/4 集成测试通过**

## 代码组织

### 目录结构
```
crates/novel-application/src/
├─ lib.rs
├─ ports.rs             (Repository trait - 已有)
├─ error.rs             (错误定义 - 已有)
├─ dto.rs               (DTO 定义 - 需要)
├─ usecase/
│  ├─ mod.rs
│  ├─ base.rs           (UseCase 基类)
│  ├─ novel/
│  │  ├─ create.rs
│  │  ├─ get.rs
│  │  ├─ list.rs
│  │  ├─ update.rs
│  │  ├─ delete.rs
│  │  └─ mod.rs
│  ├─ chapter/
│  │  ├─ create.rs
│  │  ├─ save.rs
│  │  ├─ promote.rs
│  │  └─ mod.rs
│  └─ workflow/
│     ├─ start.rs
│     ├─ approve.rs
│     └─ mod.rs
├─ service.rs           (应用服务 facade)
└─ transaction.rs       (事务管理)
```

### 依赖关系
```
UseCase
  ├─ Repository (via Ports)
  ├─ DTO
  ├─ Error
  └─ Transaction

Application Service
  ├─ UseCase
  ├─ Clock
  └─ Ids
```

## 质量标准

### 编译检查
- ✅ cargo build 通过
- ✅ cargo clippy 无警告
- ✅ cargo test 通过

### 测试覆盖
- ✅ 每个 UseCase 至少 2 个测试
- ✅ 事务回滚测试
- ✅ 错误路径测试
- 目标：总共 > 10 个新测试

### 文档
- ✅ UseCase 接口文档
- ✅ DTO 字段说明
- ✅ 事务管理说明

## 风险与缓解

### 风险 1：事务性能
- **风险**：SQLite 事务可能有并发限制
- **缓解**：优化事务大小，避免长事务

### 风险 2：DTO 爆炸
- **风险**：DTO 可能过多，维护负担
- **缓解**：复用基础 DTO，通过组合扩展

### 风险 3：循环依赖
- **风险**：UseCase 相互调用形成环
- **缓解**：严格的单向依赖，避免交叉调用

## 验收标准

- ✅ 所有 UseCase 编译无误
- ✅ 集成测试 > 70% 通过
- ✅ 无 clippy 警告
- ✅ DTO 映射准确
- ✅ 事务边界清晰
- ✅ 文档完整

## 后续影响

### Task 3.4 可进行条件
- ✅ UseCase 定义清晰 ✓（本 task 完成）
- ⏳ LLM Provider API 明确（Task 3.4 自己设计）

### Task 3.5 可进行条件
- ✅ Prompt 输入/输出 DTO 准备（本 task 进行中）
- ⏳ Prompt Registry 接口设计（Task 3.5 自己设计）

### Task 3.7 Workflow 依赖
- ✅ UseCase 框架（本 task）
- ⏳ 工作流编排（Task 3.7 在本框架上）

---

## 现在状态

**准备启动：即可开始 Task 3.3 实现**

前置完成清单：
- ✅ Domain 层完整
- ✅ Ports 定义清晰
- ✅ Repository 实现完成
- ✅ 编译和测试通过
- ✅ 设计文档完成

**下一步：开始 UseCase 基类和 DTO 定义**
