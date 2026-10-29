# Phase 3.0 Task 3.5 - 客户端集成规划

> 预计开始：2026-10-21  
> 预计完成：2-3 小时  
> 状态：📋 待启动

---

## 📌 任务目标

将 Task 3.4 完成的 HTTP API 层与 Task 3.3 的 UseCase 业务逻辑集成到一个可运行的 Rust 应用程序中，实现端到端的流程验证。

---

## 🎯 核心目标

### 主要目标
- ✅ 实现完整的 HTTP 路由处理器
- ✅ 集成 DirectorService 与 HTTP 层
- ✅ 编写完整的集成测试
- ✅ 验证端到端流程

### 次要目标
- ✅ 完整的错误处理链
- ✅ 请求验证管道
- ✅ 响应序列化规范
- ✅ 文档完善

---

## 📊 当前进度

### Phase 3.0 累积成果

```
Task 3.1 ✅ (架构设计)         - 基础架构和模块组织
Task 3.2 ✅ (错误系统)         - 统一的错误处理
Task 3.3 ✅ (UseCase 层)       - 982 行 + 52 个测试
Task 3.4 ✅ (HTTP 层)          - 1,046 行 + 39 个测试
Task 3.5 ◯ (客户端集成)       - 待启动
───────────────────────────────────────
累计：2,028 行代码 + 91 个测试
```

---

## 📂 客户端集成架构

### 层次结构

```
┌─────────────────────────────────────┐
│ HTTP Handlers (新)                  │
│  ├─ create_novel_handler            │
│  ├─ get_novel_handler               │
│  ├─ list_novels_handler             │
│  ├─ ... (13 个处理器)               │
└─────────────────┬───────────────────┘
                  │
┌─────────────────▼───────────────────┐
│ HTTP Layer (✅ Task 3.4 完成)       │
│  ├─ http_request.rs (验证)         │
│  ├─ http_response.rs (序列化)      │
│  ├─ http_error.rs (错误映射)       │
│  └─ http_routes.rs (路由定义)      │
└─────────────────┬───────────────────┘
                  │
┌─────────────────▼───────────────────┐
│ UseCase Layer (✅ Task 3.3 完成)    │
│  ├─ services_novel.rs              │
│  ├─ services_chapter.rs            │
│  └─ services_run.rs                │
└─────────────────┬───────────────────┘
                  │
┌─────────────────▼───────────────────┐
│ DirectorService + Ports             │
│  ├─ Business Logic                 │
│  ├─ Data Persistence               │
│  └─ Events                         │
└─────────────────────────────────────┘
```

---

## 🏗️ 实现计划

### 阶段 1: 处理器实现（45 分钟）

**目标**：为所有 13 个 REST 端点实现 HTTP 处理器

**文件**：`crates/novel-application/src/http_handlers.rs` (新建)

**工作内容**：

#### 小说处理器 (6 个)

```rust
// 创建小说
pub async fn create_novel_handler(
    service: &DirectorService,
    req: CreateNovelRequest,
) -> Result<NovelDetailResponse, AppError>

// 列表查询
pub async fn list_novels_handler(
    service: &DirectorService,
    req: ListNovelsRequest,
) -> Result<ListNovelsResponse, AppError>

// 获取详情、更新、删除、设置体验
// ... (类似实现)
```

#### 章节处理器 (5 个)

```rust
pub async fn create_chapter_handler(...)
pub async fn list_chapters_handler(...)
pub async fn get_chapter_handler(...)
pub async fn save_chapter_handler(...)
pub async fn promote_chapter_handler(...)
```

#### 运行处理器 (2 个)

```rust
pub async fn start_run_handler(...)
pub async fn approve_run_handler(...)
```

**验证方式**：
- 编译通过
- 所有处理器都能被调用
- 类型安全

---

### 阶段 2: 应用集成（30 分钟）

**目标**：创建可运行的应用程序主体

**文件**：`crates/novel-application/src/app.rs` (新建)

**工作内容**：

```rust
/// 应用程序上下文
pub struct AppContext {
    pub service: Arc<DirectorService>,
}

/// 应用程序初始化
pub async fn initialize_app() -> Result<AppContext, AppError> {
    // 初始化 DirectorService
    // 初始化数据库（如适用）
    // 返回应用上下文
}

/// 应用程序配置
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub database_url: String,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, AppError> {
        // 从环境变量读取配置
    }
}
```

**特性**：
- 应用程序生命周期管理
- 依赖注入
- 配置管理

---

### 阶段 3: 集成测试（45 分钟）

**目标**：编写完整的端到端集成测试

**文件**：`crates/novel-application/src/integration_tests.rs` (新建/扩展)

**测试场景**：

#### 小说工作流测试
```rust
#[tokio::test]
async fn test_novel_creation_and_retrieval() {
    // 1. 创建小说
    let novel = create_novel_handler(...).await?;
    
    // 2. 获取小说详情
    let retrieved = get_novel_handler(...).await?;
    
    // 3. 验证数据一致性
    assert_eq!(novel.id, retrieved.id);
}

#[tokio::test]
async fn test_novel_list_with_filtering() {
    // 创建多个小说
    // 查询列表
    // 验证分页和排序
}

#[tokio::test]
async fn test_novel_update_and_delete() {
    // 创建 → 更新 → 删除 → 验证
}
```

#### 章节工作流测试
```rust
#[tokio::test]
async fn test_chapter_creation_in_novel() {
    // 创建小说
    // 创建章节
    // 验证章节归属小说
}

#[tokio::test]
async fn test_chapter_save_and_retrieve() {
    // 保存章节内容
    // 检索并验证
}

#[tokio::test]
async fn test_chapter_promotion() {
    // 创建候选版本
    // 提升为正式版本
    // 验证转换
}
```

#### 运行工作流测试
```rust
#[tokio::test]
async fn test_run_creation_and_approval() {
    // 启动运行
    // 批准运行
    // 验证状态转换
}
```

#### 错误处理测试
```rust
#[tokio::test]
async fn test_validation_error_handling() {
    // 发送无效请求
    // 验证返回 400 Bad Request
}

#[tokio::test]
async fn test_not_found_error_handling() {
    // 查询不存在的资源
    // 验证返回 404 Not Found
}

#[tokio::test]
async fn test_conflict_error_handling() {
    // 触发冲突条件
    // 验证返回 409 Conflict
}
```

**测试数量**：20+ 个集成测试

---

### 阶段 4: 文档和验证（20 分钟）

**目标**：文档完善和最终验证

**工作内容**：

1. **API 使用文档**
   - 请求/响应示例
   - 错误代码列表
   - 工作流指南

2. **集成指南**
   - 处理器注册流程
   - 错误处理最佳实践
   - 测试运行指南

3. **最终验证**
   - 编译检查
   - 所有测试通过
   - 代码审查

---

## 📁 新增文件清单

```
crates/novel-application/src/
├─ http_handlers.rs (预计 400-500 行)
│  ├─ create_novel_handler (60 行)
│  ├─ list_novels_handler (50 行)
│  ├─ get_novel_handler (50 行)
│  ├─ update_novel_handler (60 行)
│  ├─ delete_novel_handler (40 行)
│  ├─ set_creation_experience_handler (50 行)
│  ├─ create_chapter_handler (50 行)
│  ├─ list_chapters_handler (50 行)
│  ├─ get_chapter_handler (50 行)
│  ├─ save_chapter_handler (60 行)
│  ├─ promote_chapter_handler (50 行)
│  ├─ start_run_handler (50 行)
│  └─ approve_run_handler (50 行)
├─ app.rs (预计 100-150 行)
│  ├─ AppConfig
│  ├─ AppContext
│  └─ initialize_app()
└─ lib.rs (修改，添加模块声明)

tests/
└─ integration_tests.rs (预计 800-1000 行)
   ├─ 小说工作流测试 (6 个)
   ├─ 章节工作流测试 (6 个)
   ├─ 运行工作流测试 (3 个)
   ├─ 错误处理测试 (5+ 个)
   └─ 完整流程测试 (3+ 个)
```

---

## 🧪 测试覆盖

### 单元测试 (既有)
- Task 3.3：52 个 UseCase 测试
- Task 3.4：39 个 HTTP 层测试
- **小计**：91 个

### 集成测试 (Task 3.5 新增)
- 小说工作流：6 个
- 章节工作流：6 个
- 运行工作流：3 个
- 错误处理：5 个
- 完整流程：3 个
- **小计**：23+ 个

### 总计
- **114+ 个测试**
- **100% 通过率**

---

## 📊 关键指标

| 指标 | 目标 | 备注 |
|------|------|------|
| 代码行数 | 500-700 行 | 处理器 + 应用 + 集成测试 |
| 测试数 | 23+ 个 | 全覆盖所有端点 |
| 编译 | 零错误 | 类型安全 |
| 测试通过率 | 100% | 无失败 |
| 文档完整度 | 100% | 所有方法注释 |

---

## ⚠️ 潜在挑战

### 1. 异步处理
**问题**：处理器需要异步调用 UseCase
**解决**：使用 `tokio::test` 和异步 Rust 模式

### 2. 依赖注入
**问题**：DirectorService 的初始化和传递
**解决**：创建 AppContext，使用 Arc<> 共享

### 3. 数据库连接
**问题**：集成测试需要真实或 mock 数据库
**解决**：使用 test fixtures 或内存数据库

### 4. 错误处理链
**问题**：确保所有错误都正确映射
**解决**：完整的错误处理测试

---

## 🎯 成功标准

任务完成时应满足：

- ✅ 13 个 HTTP 端点都有完整处理器
- ✅ 所有处理器都能正确调用 UseCase
- ✅ 请求验证和响应转换完整
- ✅ 23+ 个集成测试全部通过
- ✅ 编译零错误、零致命警告
- ✅ 所有代码都有文档注释
- ✅ 端到端流程可验证

---

## 📝 提交计划

每个阶段完成后提交一次：

```
1. 阶段 1 完成 → "新增：Task 3.5 第一步 - HTTP 处理器实现"
2. 阶段 2 完成 → "新增：Task 3.5 第二步 - 应用程序集成"
3. 阶段 3 完成 → "测试：Task 3.5 第三步 - 端到端集成测试"
4. 阶段 4 完成 → "报告：Task 3.5 - 最终完成报告"
```

---

## 🔗 任务链

```
Phase 3.0 - Rust 后端架构化
├─ Task 3.1 ✅ (架构设计) - 2026-10-16
├─ Task 3.2 ✅ (错误系统) - 2026-10-17
├─ Task 3.3 ✅ (UseCase 层) - 2026-10-18-19
├─ Task 3.4 ✅ (HTTP 层) - 2026-10-20
├─ Task 3.5 ◯ (客户端集成) - 2026-10-21 ← 当前
└─ Phase 3.0 完成 - 2026-10-23
```

---

## 📚 相关文档

### 输入文档
- [Task 3.3 最终报告](phase-3.0-task-3.3-final-report.md) - UseCase 实现
- [Task 3.4 最终报告](phase-3.0-task-3.4-final-report.md) - HTTP 层设计

### 输出文档（待生成）
- HTTP 处理器规范
- 应用集成指南
- 集成测试报告

---

## ⏱️ 时间预估

```
总耗时：2-3 小时

阶段分布：
├─ 第一步（处理器）：45 分钟
├─ 第二步（应用）：30 分钟
├─ 第三步（测试）：45 分钟
└─ 第四步（验证）：20 分钟
```

---

**状态**：📋 规划完成，准备开始实现

**下一步**：启动 Task 3.5 第一步 - HTTP 处理器实现

**预计完成**：2026-10-21（明日）

---

## 📌 快速参考

### HTTP 处理器模板

```rust
pub async fn handler_name(
    service: &DirectorService,
    req: RequestType,
) -> Result<ResponseType, AppError> {
    // 1. 验证请求
    req.validate()
        .map_err(|e| AppError::validation(&e))?;
    
    // 2. 转换为 UseCase 输入
    let input = ConvertToUseCaseInput::from(req);
    
    // 3. 调用 UseCase
    let result = service.method(&input).await?;
    
    // 4. 转换为 HTTP 响应
    Ok(ResponseType::from(result))
}
```

### 错误映射模板

```rust
// 使用 map_error_to_http 进行错误映射
match handler_result {
    Ok(response) => Ok(response),
    Err(app_error) => {
        let http_error = map_error_to_http(app_error);
        Err(/* HTTP 状态和响应 */)
    }
}
```

### 集成测试模板

```rust
#[tokio::test]
async fn test_workflow() {
    // 1. 初始化应用
    let ctx = initialize_app().await.unwrap();
    
    // 2. 执行操作
    let result = handler(&ctx.service, request).await;
    
    // 3. 验证结果
    assert!(result.is_ok());
    assert_eq!(result.unwrap().field, expected_value);
}
```

