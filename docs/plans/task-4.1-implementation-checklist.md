# Task 4.1 - 实现清单与初始框架

**日期**: 2026-10-20  
**阶段**: Phase 4.0 - 实现与集成  
**任务**: Task 4.1 - Web 框架集成  
**当前步骤**: 实现准备与初始框架设计

---

## Part 1: 实现清单

### 1.1 项目配置阶段清单

#### Cargo.toml 配置
- [ ] 添加 actix-web 依赖 (4.4+)
- [ ] 添加 actix-rt 依赖 (2.9+)
- [ ] 添加 tokio 依赖 (完整功能)
- [ ] 添加 serde & serde_json
- [ ] 添加 tracing & tracing-subscriber
- [ ] 添加 dotenv
- [ ] 添加 uuid & chrono
- [ ] 验证所有依赖兼容性
- [ ] 运行 cargo check

**预期**: ~20 行依赖配置

#### 项目结构创建
- [ ] 创建 server/src/ 目录
- [ ] 创建 server/src/http/ 模块
- [ ] 创建 server/src/http/middleware/ 子模块
- [ ] 创建 server/src/http/handlers/ 子模块
- [ ] 创建 server/src/http/mod.rs
- [ ] 创建 server/src/http/app.rs
- [ ] 创建 server/src/http/routes.rs
- [ ] 创建 server/src/config.rs
- [ ] 创建 server/src/main.rs
- [ ] 创建 .env 文件模板
- [ ] 验证项目结构

**预期**: 15-20 个新文件

### 1.2 中间件实现清单

#### 日志中间件
- [ ] 实现 HttpMetrics struct
- [ ] 实现 Transform trait
- [ ] 实现 Service trait
- [ ] 记录请求方法和路径
- [ ] 记录响应时间
- [ ] 单元测试日志中间件
- [ ] 性能验证

**预期**: ~80 行代码 + 5 个测试

#### 错误处理中间件
- [ ] 实现 ErrorHandler struct
- [ ] 实现错误捕获
- [ ] 实现错误日志
- [ ] 验证错误响应格式
- [ ] 单元测试错误处理
- [ ] 边界情况测试

**预期**: ~70 行代码 + 4 个测试

#### CORS 中间件 (可选)
- [ ] 配置允许的源
- [ ] 配置允许的方法
- [ ] 配置允许的头
- [ ] 测试 CORS 功求

**预期**: ~40 行代码

### 1.3 路由配置清单

- [ ] 定义所有路由 (13 个)
- [ ] 映射到处理器
- [ ] 验证路由冲突
- [ ] 测试路由匹配

**预期**: ~25 行代码 + 8 个路由测试

### 1.4 处理器实现清单

#### 小说处理器 (6 个端点)
- [ ] POST /novels - 创建小说
  - [ ] 请求验证
  - [ ] UseCase 调用
  - [ ] 响应序列化
  - [ ] 错误处理
  - [ ] 单元测试
  - [ ] 集成测试
  
- [ ] GET /novels - 列出小说
  - [ ] 分页支持
  - [ ] 过滤支持
  - [ ] 响应序列化
  - [ ] 集成测试
  
- [ ] GET /novels/{id} - 获取单个小说
  - [ ] ID 验证
  - [ ] 404 处理
  - [ ] 集成测试
  
- [ ] PUT /novels/{id} - 更新小说
  - [ ] 请求验证
  - [ ] UseCase 调用
  - [ ] 乐观锁检查
  - [ ] 集成测试
  
- [ ] DELETE /novels/{id} - 删除小说
  - [ ] 级联删除检查
  - [ ] 204 响应
  - [ ] 集成测试
  
- [ ] POST /novels/{id}/experience - 设置经验等级
  - [ ] 参数验证
  - [ ] UseCase 调用
  - [ ] 集成测试

**预期**: ~110 行代码 + 12 个测试

#### 章节处理器 (5 个端点)
- [ ] POST /novels/{novel_id}/chapters - 创建章节
- [ ] GET /novels/{novel_id}/chapters - 列出章节
- [ ] GET /chapters/{id} - 获取章节
- [ ] PUT /chapters/{id} - 保存章节
- [ ] POST /chapters/{id}/promote - 晋升章节

**预期**: ~130 行代码 + 10 个测试

#### 运行处理器 (2 个端点)
- [ ] POST /runs - 启动运行
- [ ] POST /runs/{id}/approve - 批准运行

**预期**: ~60 行代码 + 4 个测试

### 1.5 应用程序集成清单

- [ ] 实现 main.rs
- [ ] 配置日志初始化
- [ ] 配置应用启动
- [ ] 配置路由挂载
- [ ] 配置中间件堆栈
- [ ] 测试应用启动
- [ ] 验证所有中间件工作

**预期**: ~40 行代码 + 3 个测试

### 1.6 集成测试清单

#### HTTP 集成测试
- [ ] 创建 tests/http_integration_tests.rs
- [ ] 测试所有小说端点 (6 个)
  - [ ] POST /novels
  - [ ] GET /novels
  - [ ] GET /novels/{id}
  - [ ] PUT /novels/{id}
  - [ ] DELETE /novels/{id}
  - [ ] POST /novels/{id}/experience
  
- [ ] 测试所有章节端点 (5 个)
- [ ] 测试所有运行端点 (2 个)
- [ ] 测试错误响应 (5+ 个)
- [ ] 测试边界情况 (5+ 个)

**预期**: 15+ 集成测试

#### 端到端测试
- [ ] 创建小说 → 验证返回
- [ ] 创建章节 → 验证返回
- [ ] 启动运行 → 验证返回
- [ ] 批准运行 → 验证返回
- [ ] 完整工作流测试

**预期**: 5+ E2E 测试

### 1.7 文档清单

- [ ] 编写 API 端点文档 (所有 13 个)
- [ ] 编写请求/响应示例
- [ ] 编写错误响应文档
- [ ] 编写使用指南
- [ ] 编写集成指南
- [ ] 提供代码示例

**预期**: 5-10 页 API 文档

---

## Part 2: 初始项目框架

### 2.1 项目结构框架

```
server/
├── src/
│   ├── main.rs              # 应用入口点
│   ├── config.rs            # 配置管理
│   ├── http/
│   │   ├── mod.rs           # 模块导出
│   │   ├── app.rs           # actix-web 应用配置
│   │   ├── routes.rs        # 路由定义
│   │   ├── middleware/
│   │   │   ├── mod.rs
│   │   │   ├── logging.rs   # 日志中间件
│   │   │   ├── error.rs     # 错误处理
│   │   │   └── cors.rs      # CORS 中间件
│   │   └── handlers/
│   │       ├── mod.rs
│   │       ├── novels.rs    # 小说处理器
│   │       ├── chapters.rs  # 章节处理器
│   │       └── runs.rs      # 运行处理器
│   └── lib.rs               # 库导出
├── tests/
│   ├── http_integration_tests.rs    # HTTP 测试
│   └── e2e_tests.rs                 # E2E 测试
├── Cargo.toml               # 项目配置
├── .env.example             # 环境变量示例
└── README.md                # 项目说明

crates/
├── novel-application/       # Phase 3.0 的应用层
│   └── src/                 # (已存在)
└── (其他 crate)
```

### 2.2 Cargo.toml 框架

```toml
[package]
name = "novel-server"
version = "0.1.0"
edition = "2021"

[dependencies]
# Web 框架
actix-web = "4.4"
actix-rt = "2.9"
tokio = { version = "1.35", features = ["full"] }

# 序列化
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# 日志和追踪
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt"] }

# 配置
dotenv = "0.15"

# 工具
uuid = { version = "1.6", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }

# 项目内部依赖
novel-application = { path = "../crates/novel-application" }

[dev-dependencies]
tokio = { version = "1.35", features = ["full"] }
actix-web = "4.4"

[[test]]
name = "http_integration_tests"
path = "tests/http_integration_tests.rs"

[[test]]
name = "e2e_tests"
path = "tests/e2e_tests.rs"
```

### 2.3 main.rs 框架

```rust
use actix_web::{web, App, HttpServer};
use novel_application::app::{AppConfig, AppInitializer};
use std::env;

mod config;
mod http;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
        )
        .init();

    // 从环境变量读取配置
    dotenv::dotenv().ok();
    let app_config = config::AppConfig::from_env()
        .expect("Failed to load app config");

    tracing::info!("Server configuration loaded");

    // 初始化应用上下文
    let ctx = AppInitializer::initialize_with(app_config.into())
        .await
        .expect("Failed to initialize application");

    let host = env::var("SERVER_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = env::var("SERVER_PORT").unwrap_or_else(|_| "8080".to_string());
    let addr = format!("{}:{}", host, port);

    tracing::info!("Starting HTTP server on {}", addr);

    // 启动 HTTP 服务器
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(ctx.clone()))
            .wrap(http::middleware::logging::HttpMetrics)
            .wrap(http::middleware::error::ErrorHandler)
            .configure(http::routes::configure)
    })
    .bind(&addr)?
    .run()
    .await
}
```

### 2.4 config.rs 框架

```rust
use std::env;

#[derive(Clone)]
pub struct AppConfig {
    pub server_host: String,
    pub server_port: String,
    pub log_level: String,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, String> {
        Ok(AppConfig {
            server_host: env::var("SERVER_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
            server_port: env::var("SERVER_PORT").unwrap_or_else(|_| "8080".to_string()),
            log_level: env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string()),
        })
    }
}
```

### 2.5 http/mod.rs 框架

```rust
pub mod app;
pub mod routes;
pub mod middleware;
pub mod handlers;

pub use app::create_app;
pub use routes::configure;
```

### 2.6 http/routes.rs 框架

```rust
use actix_web::web;
use super::handlers::{novels, chapters, runs};

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg
        // 小说路由
        .route("/novels", web::post().to(novels::create_novel))
        .route("/novels", web::get().to(novels::list_novels))
        .route("/novels/{id}", web::get().to(novels::get_novel))
        .route("/novels/{id}", web::put().to(novels::update_novel))
        .route("/novels/{id}", web::delete().to(novels::delete_novel))
        .route("/novels/{id}/experience", web::post().to(novels::set_creation_experience))
        
        // 章节路由
        .route("/novels/{novel_id}/chapters", web::post().to(chapters::create_chapter))
        .route("/novels/{novel_id}/chapters", web::get().to(chapters::list_chapters))
        .route("/chapters/{id}", web::get().to(chapters::get_chapter))
        .route("/chapters/{id}", web::put().to(chapters::save_chapter))
        .route("/chapters/{id}/promote", web::post().to(chapters::promote_chapter))
        
        // 运行路由
        .route("/runs", web::post().to(runs::start_run))
        .route("/runs/{id}/approve", web::post().to(runs::approve_run));
}
```

### 2.7 http/handlers/novels.rs 框架

```rust
use actix_web::{web, HttpResponse, Result};
use serde::{Deserialize, Serialize};
use novel_application::usecase::NovelUseCases;

#[derive(Serialize, Deserialize, Clone)]
pub struct CreateNovelRequest {
    pub title: String,
    pub description: Option<String>,
}

#[derive(Serialize)]
pub struct NovelResponse {
    pub id: String,
    pub title: String,
    pub created_at: String,
}

pub async fn create_novel(
    req: web::Json<CreateNovelRequest>,
) -> Result<HttpResponse> {
    // 验证请求
    if req.title.trim().is_empty() {
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Title cannot be empty"
        })));
    }

    // 调用 UseCase
    match NovelUseCases::create_novel(&req.title, req.description.as_deref()) {
        Ok(novel) => {
            Ok(HttpResponse::Created().json(NovelResponse {
                id: novel.id,
                title: novel.title,
                created_at: format!("{:?}", chrono::Utc::now()),
            }))
        }
        Err(e) => {
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": e.to_string()
            })))
        }
    }
}

pub async fn list_novels() -> Result<HttpResponse> {
    match NovelUseCases::list_novels() {
        Ok(novels) => Ok(HttpResponse::Ok().json(novels)),
        Err(e) => Ok(HttpResponse::InternalServerError().json(serde_json::json!({
            "error": e.to_string()
        }))),
    }
}

pub async fn get_novel(path: web::Path<String>) -> Result<HttpResponse> {
    let id = path.into_inner();
    match NovelUseCases::get_novel(&id) {
        Ok(novel) => Ok(HttpResponse::Ok().json(novel)),
        Err(_) => Ok(HttpResponse::NotFound().json(serde_json::json!({
            "error": "Novel not found"
        }))),
    }
}

pub async fn update_novel(
    path: web::Path<String>,
    req: web::Json<CreateNovelRequest>,
) -> Result<HttpResponse> {
    let id = path.into_inner();
    // TODO: 实现更新逻辑
    Ok(HttpResponse::Ok().json(serde_json::json!({})))
}

pub async fn delete_novel(path: web::Path<String>) -> Result<HttpResponse> {
    let id = path.into_inner();
    match NovelUseCases::delete_novel(&id) {
        Ok(_) => Ok(HttpResponse::NoContent().finish()),
        Err(_) => Ok(HttpResponse::NotFound().json(serde_json::json!({
            "error": "Novel not found"
        }))),
    }
}

pub async fn set_creation_experience(
    path: web::Path<String>,
    req: web::Json<serde_json::Value>,
) -> Result<HttpResponse> {
    let id = path.into_inner();
    // TODO: 实现设置经验等级的逻辑
    Ok(HttpResponse::Ok().json(serde_json::json!({})))
}
```

---

## Part 3: 实现优先级

### 第一优先级 (必须完成)
1. ✅ Cargo.toml 配置
2. ✅ 项目结构创建
3. ✅ main.rs 实现
4. ✅ config.rs 实现
5. ✅ http/routes.rs 实现
6. ✅ 日志中间件
7. ✅ 小说处理器 (6 个)
8. ✅ 章节处理器 (5 个)
9. ✅ 运行处理器 (2 个)
10. ✅ HTTP 集成测试 (15+)

### 第二优先级 (重要但可延后)
1. ✅ 错误处理中间件
2. ✅ CORS 中间件
3. ✅ E2E 测试 (5+)
4. ✅ API 文档

### 第三优先级 (增强功能)
1. ✅ 性能优化
2. ✅ 缓存支持
3. ✅ 速率限制

---

## Part 4: 测试策略

### 单元测试
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_novel_request_validation() {
        // 测试空标题
        let req = CreateNovelRequest {
            title: "".to_string(),
            description: None,
        };
        // 验证失败
    }

    #[test]
    fn test_create_novel_response_format() {
        // 测试响应格式
    }
}
```

### 集成测试
```rust
#[actix_web::test]
async fn test_create_novel_integration() {
    let app = test::init_service(
        App::new()
            .configure(routes::configure)
    ).await;

    let req = test::TestRequest::post()
        .uri("/novels")
        .set_json(serde_json::json!({
            "title": "Test Novel",
            "description": "A test"
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::CREATED);
}
```

### E2E 测试
```rust
#[tokio::test]
async fn test_complete_workflow() {
    // 1. 创建小说
    // 2. 验证小说存在
    // 3. 创建章节
    // 4. 验证章节存在
    // 5. 启动运行
    // 6. 批准运行
}
```

---

## Part 5: 验证检查点

### Day 1-2 验证
- [ ] Cargo build 成功
- [ ] 项目结构正确
- [ ] 依赖都可用

### Day 3-4 验证
- [ ] 中间件代码编译
- [ ] 中间件测试通过
- [ ] 日志输出正常

### Day 5-7 验证
- [ ] 所有处理器编译通过
- [ ] 每个处理器有基本实现
- [ ] 路由测试通过

### Day 8 验证
- [ ] 集成测试通过 (15+ 个)
- [ ] 所有端点可访问
- [ ] 错误处理工作

### Day 9 验证
- [ ] API 文档完整
- [ ] 使用示例可运行
- [ ] 最终编译检查通过

---

## 总结

这个**实现清单与初始框架**为 Task 4.1 的实际开发提供了：

1. ✅ **详细的实现清单** - 逐项可追踪
2. ✅ **项目结构框架** - 可直接使用
3. ✅ **代码框架示例** - 可快速启动
4. ✅ **测试策略** - 分层次验证
5. ✅ **验证检查点** - 全程质量控制

现在可以开始实际的代码实现了！

---

**状态**: 准备就绪 🚀  
**下一步**: 开始 Task 4.1 实现 (2026-10-22)  
**预计完成**: 2026-10-30
