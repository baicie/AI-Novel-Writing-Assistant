# Task 3.5 集成指南

本指南说明如何在实际项目中集成和使用 Task 3.5 开发的 HTTP 处理器框架和应用集成层。

---

## 目录

1. [快速开始](#快速开始)
2. [应用初始化](#应用初始化)
3. [处理器集成](#处理器集成)
4. [错误处理](#错误处理)
5. [配置管理](#配置管理)
6. [监控和调试](#监控和调试)
7. [常见问题](#常见问题)

---

## 快速开始

### 导入模块

```rust
use novel_application::{
    app::{AppConfig, AppContext, AppInitializer},
    http_handlers::HttpHandlers,
    error::{AppError, AppErrorCode},
};
```

### 初始化应用

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 从配置初始化
    let ctx = AppInitializer::initialize().await?;
    
    // 检查应用状态
    println!("{}", ctx.status());
    
    Ok(())
}
```

---

## 应用初始化

### 三种初始化方式

#### 1. 从环境变量读取配置

```rust
let ctx = AppInitializer::initialize().await?;
```

环境变量:
- `APP_HOST`: 默认 `127.0.0.1`
- `APP_PORT`: 默认 `8080`
- `DATABASE_URL`: 默认 `sqlite::memory:`
- `APP_ENV`: 默认 `development`
- `LOG_LEVEL`: 默认 `info`

#### 2. 使用预定义配置

```rust
// 开发配置
let config = AppConfig::development();
let ctx = AppInitializer::initialize_with(config).await?;

// 测试配置
let config = AppConfig::testing();
let ctx = AppInitializer::initialize_with(config).await?;

// 生产配置
let config = AppConfig::production();
let ctx = AppInitializer::initialize_with(config).await?;
```

#### 3. 自定义配置

```rust
use std::collections::HashMap;

let config = AppConfig {
    host: "0.0.0.0".to_string(),
    port: 3000,
    database_url: "postgres://localhost/novel".to_string(),
    environment: "staging".to_string(),
    log_level: "debug".to_string(),
    extra: HashMap::new(),
};

let ctx = AppInitializer::initialize_with(config).await?;
```

### 应用状态管理

```rust
// 检查运行状态
if ctx.is_running() {
    println!("Application is running");
}

// 获取完整状态报告
let status = ctx.status();
println!("{}", status);
// 输出: "Status: running | host: 127.0.0.1:8080 | ... | Requests: 0 | Errors: 0"

// 手动转换状态
let mut ctx = ctx;
ctx.mark_stopping();  // 转为停止中
ctx.mark_stopped();   // 转为已停止
ctx.mark_error();     // 转为错误状态
```

---

## 处理器集成

### 查询可用处理器

```rust
// 获取所有处理器名称列表
let handlers = HttpHandlers::list();
println!("{:?}", handlers);
// ["create_novel", "list_novels", "get_novel", ...]

// 获取处理器总数
let count = HttpHandlers::count();
assert_eq!(count, 13);

// 检查特定处理器是否存在
if HttpHandlers::has("create_novel") {
    println!("create_novel handler is available");
}

// 按类别统计
let novel_handlers = HttpHandlers::count_by_category("novel");     // 6
let chapter_handlers = HttpHandlers::count_by_category("chapter"); // 5
let run_handlers = HttpHandlers::count_by_category("run");         // 2
```

### 处理器分类

#### 小说处理器 (6)

```
create_novel         - 创建新小说
list_novels          - 列出小说
get_novel            - 获取小说详情
update_novel         - 更新小说
delete_novel         - 删除小说
set_creation_experience - 设置创作经验级别
```

#### 章节处理器 (5)

```
create_chapter       - 创建新章节
list_chapters        - 列出章节
get_chapter          - 获取章节详情
save_chapter         - 保存章节内容
promote_chapter      - 晋升章节状态
```

#### 运行处理器 (2)

```
start_run           - 启动运行（生成、审查等）
approve_run         - 批准或拒绝运行结果
```

---

## 错误处理

### 错误类型

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppErrorCode {
    Validation,      // 输入验证失败
    Conflict,        // 资源冲突
    NotFound,        // 资源未找到
    Cancelled,       // 操作取消
    ModelRateLimit,  // 模型速率限制
    StorageCorrupt,  // 存储损坏
    IntegrityError,  // 数据完整性错误
}
```

### 创建错误

```rust
// 验证错误
let error = AppError::validation("Title cannot be empty");

// 未找到错误
let error = AppError::not_found("Novel not found with ID: 123");

// 冲突错误
let error = AppError::conflict("Novel already exists");

// 完整性错误
let error = AppError::integrity("Foreign key constraint violated");
```

### 检查和处理

```rust
let error = AppError::validation("Invalid input");

// 访问错误信息
println!("Code: {:?}", error.code);
println!("Message: {}", error.message);
println!("Retryable: {}", error.retryable);

// 根据错误类型处理
match error.code {
    AppErrorCode::Validation => {
        eprintln!("Input error: {}", error.message);
    }
    AppErrorCode::NotFound => {
        eprintln!("Resource not found: {}", error.message);
    }
    AppErrorCode::Conflict => {
        eprintln!("Conflict: {}", error.message);
    }
    _ => {
        eprintln!("Error: {}", error.message);
    }
}
```

### HTTP 映射

| AppErrorCode | HTTP 状态 | 描述 |
|-------------|---------|------|
| Validation | 400 | 请求验证失败 |
| Conflict | 409 | 资源冲突 |
| NotFound | 404 | 资源不存在 |
| Cancelled | 409 | 操作被取消 |
| ModelRateLimit | 429 | 速率限制 |
| StorageCorrupt | 500 | 服务器内部错误 |
| IntegrityError | 500 | 服务器内部错误 |

---

## 配置管理

### 环境特定配置

#### 开发环境

```rust
let config = AppConfig::development();
// host: 127.0.0.1
// port: 8080
// database: sqlite::memory:
// env: development
// log_level: debug
```

#### 测试环境

```rust
let config = AppConfig::testing();
// host: 127.0.0.1
// port: 8081  (不同端口避免冲突)
// database: sqlite::memory:
// env: testing
// log_level: debug
```

#### 生产环境

```rust
let config = AppConfig::production();
// host: 0.0.0.0  (所有接口)
// port: 8080
// database: postgres://localhost/novel
// env: production
// log_level: info
```

### 配置验证

```rust
let config = AppConfig {
    host: "invalid_host".to_string(),  // 无效
    port: 8080,
    database_url: "".to_string(),       // 无效
    environment: "prod".to_string(),
    log_level: "debug".to_string(),
    extra: HashMap::new(),
};

// 验证配置
match config.validate() {
    Ok(_) => println!("Configuration is valid"),
    Err(e) => println!("Configuration error: {}", e),
}
```

### 配置摘要

```rust
let config = AppConfig::development();
let summary = config.summary();
println!("{}", summary);
// "host: 127.0.0.1, port: 8080, env: development, log_level: debug"
```

---

## 监控和调试

### 元数据跟踪

```rust
let ctx = AppInitializer::initialize().await?;

// 访问元数据
let metadata = &ctx.metadata;
println!("Application: {}", metadata.name);
println!("Version: {}", metadata.version);
println!("Requests: {}", metadata.request_count);
println!("Errors: {}", metadata.error_count);
println!("Uptime: {} seconds", metadata.uptime_seconds());
```

### 手动更新元数据

```rust
let mut ctx = AppInitializer::initialize().await?;

// 增加计数
ctx.metadata.increment_request_count();
ctx.metadata.increment_error_count();

println!("Requests: {}", ctx.metadata.request_count);
println!("Errors: {}", ctx.metadata.error_count);
```

### 应用状态报告

```rust
let ctx = AppInitializer::initialize().await?;

let status = ctx.status();
println!("{}", status);

// 输出示例:
// Status: running | host: 127.0.0.1:8080 | Uptime: 5s | Requests: 42 | Errors: 1
```

---

## 常见问题

### Q: 如何自定义处理器？

A: 处理器框架提供了基础结构。实际实现应通过 `HttpHandler` trait 扩展：

```rust
pub trait HttpHandler {
    type Request: ValidateRequest;
    type Response;

    fn handle(request: Self::Request) -> Result<Self::Response, AppError> {
        request.validate()?;
        Self::process(&request)
    }

    fn process(request: &Self::Request) -> Result<Self::Response, AppError>;
}
```

### Q: 支持哪些数据库？

A: 当前支持通过 `database_url` 配置：
- SQLite (开发/测试): `sqlite::memory:` 或 `sqlite:///path/to/db.sqlite`
- PostgreSQL (生产): `postgres://user:pass@localhost/dbname`

### Q: 如何处理初始化失败？

A:

```rust
match AppInitializer::initialize().await {
    Ok(ctx) => {
        println!("Application initialized successfully");
    }
    Err(e) => {
        eprintln!("Initialization failed: {}", e);
        eprintln!("Error code: {:?}", e.code);
        // 退出或重试
        std::process::exit(1);
    }
}
```

### Q: 元数据计数是否线程安全？

A: 当前实现不是原子的。对于多线程环境，应将元数据移动到原子类型或使用互斥锁：

```rust
use std::sync::atomic::{AtomicU64, Ordering};

// 使用 AtomicU64 替换 u64
pub request_count: AtomicU64,

// 更新方式
self.request_count.fetch_add(1, Ordering::SeqCst);
```

### Q: 应用生命周期管理

A: 完整的生命周期流程：

```
初始化中 (Initializing)
    ↓
运行中 (Running)
    ↓
停止中 (Stopping)
    ↓
已停止 (Stopped)

或在任何时刻转为错误状态 (Error)
```

### Q: 如何集成日志记录？

A: `log_level` 配置通知系统日志应该使用的级别。实际日志记录应该在 Web 框架集成层实现：

```rust
let config = AppConfig::development();
println!("Log level: {}", config.log_level);  // "debug"
// 初始化日志系统时使用此值
```

---

## 最佳实践

1. **配置验证**: 始终调用 `config.validate()` 或通过 `initialize` 路由验证
2. **错误处理**: 使用具体的 `AppErrorCode` 而不是通用错误
3. **状态检查**: 在操作前检查 `ctx.is_running()`
4. **监控**: 定期检查元数据中的请求/错误计数
5. **日志记录**: 使用 `ctx.status()` 生成调试信息

---

## 下一步

1. **Web 框架集成**: 使用 actix-web 或其他框架实现实际的 HTTP 端点
2. **数据库集成**: 实现持久化存储层
3. **认证/授权**: 添加用户认证和权限检查
4. **API 文档**: 使用 OpenAPI/Swagger 生成 API 文档
5. **性能监控**: 集成 Prometheus 或其他监控系统

---

**最后更新**: 2026-10-20  
**作者**: AI Assistant  
**版本**: 1.0
