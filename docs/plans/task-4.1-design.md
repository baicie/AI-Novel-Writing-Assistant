# Task 4.1 - Web 框架集成详细设计

**日期**: 2026-10-20  
**阶段**: Phase 4.0 - 实现与集成  
**任务**: Task 4.1 - Web 框架集成  
**目标**: 选择和集成 actix-web，实现所有 13 个 REST 端点

---

## 任务概述

### 目标

将 Phase 3.0 验证的架构框架与实际的 Web 框架集成，实现完整的 HTTP REST API。

### 为什么是 actix-web？

| 框架 | 性能 | 异步支持 | 中间件 | 学习曲线 |
|-----|------|---------|--------|---------|
| **actix-web** | ⭐⭐⭐⭐⭐ | ✅ 原生 | ✅ 完善 | 中 |
| tokio-tungstenite | ⭐⭐⭐⭐ | ✅ 原生 | ❌ 基础 | 低 |
| Rocket | ⭐⭐⭐ | ✅ 原生 | ✅ 完善 | 中 |
| Axum | ⭐⭐⭐⭐ | ✅ 原生 | ✅ 完善 | 高 |

**选择理由**:
- 性能最好 (Rust 生态中最快)
- 异步支持成熟
- 中间件生态完善
- 与 Phase 3.0 架构兼容

---

## 任务分解

### Part 1: 项目配置与基础设置 (1-2 天)

#### 1.1 Cargo 依赖配置

**文件**: `Cargo.toml` (增加依赖)

```toml
[dependencies]
# Web 框架
actix-web = "4.4"
actix-rt = "2.9"

# 异步运行时
tokio = { version = "1.35", features = ["full"] }

# JSON 序列化
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# 日志
tracing = "0.1"
tracing-subscriber = "0.3"

# 配置
dotenv = "0.15"

# 工具
uuid = { version = "1.6", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
```

**预期**: ~20 行配置

#### 1.2 项目结构创建

```
server/
├── src/
│   ├── main.rs              # 应用入口
│   ├── http/
│   │   ├── mod.rs           # HTTP 模块入口
│   │   ├── app.rs           # actix-web 应用配置
│   │   ├── routes.rs        # 路由注册
│   │   ├── middleware/
│   │   │   ├── mod.rs
│   │   │   ├── logging.rs   # 日志中间件
│   │   │   ├── error.rs     # 错误处理中间件
│   │   │   └── cors.rs      # CORS 中间件
│   │   └── handlers/
│   │       ├── mod.rs
│   │       ├── novels.rs    # 小说端点
│   │       ├── chapters.rs  # 章节端点
│   │       └── runs.rs      # 运行端点
│   └── config.rs            # 应用配置
├── Cargo.toml
└── .env                      # 环境变量

总计: ~15-20 个新文件
```

**预期**: 新增 15-20 个文件框架

#### 1.3 基础应用程序

**文件**: `src/main.rs`

```rust
use actix_web::{web, App, HttpServer};
use tracing_subscriber;

mod http;
mod config;

use novel_application::app::{AppConfig, AppInitializer};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt::init();

    // 从环境变量读取配置
    let app_config = AppConfig::from_env()?;
    
    // 初始化应用
    let ctx = AppInitializer::initialize_with(app_config)
        .await
        .expect("Failed to initialize app");

    tracing::info!("Starting server on {}:{}", ctx.config.host, ctx.config.port);

    // 启动 HTTP 服务器
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(ctx.clone()))
            .wrap(http::middleware::logging::HttpMetrics)
            .wrap(http::middleware::error::ErrorHandler)
            .configure(http::routes::configure)
    })
    .bind(format!("{}:{}", ctx.config.host, ctx.config.port))?
    .run()
    .await
}
```

**预期**: ~40 行代码

### Part 2: 中间件实现 (1-2 天)

#### 2.1 日志中间件

**文件**: `src/http/middleware/logging.rs`

```rust
use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpMessage,
};
use futures::future::LocalBoxFuture;
use std::time::Instant;

pub struct HttpMetrics;

impl<S, B> Transform<S, ServiceRequest> for HttpMetrics
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = HttpMetricsMiddleware<S>;
    type InitError = ();
    type Future = std::future::Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        std::future::ready(Ok(HttpMetricsMiddleware { service }))
    }
}

pub struct HttpMetricsMiddleware<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for HttpMetricsMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let start = Instant::now();
        let method = req.method().to_string();
        let path = req.path().to_string();

        let fut = self.service.call(req);

        Box::pin(async move {
            let res = fut.await?;
            let elapsed = start.elapsed();
            
            tracing::info!(
                "{} {} - {}ms",
                method,
                path,
                elapsed.as_millis()
            );

            Ok(res)
        })
    }
}
```

**预期**: ~80 行代码

#### 2.2 错误处理中间件

**文件**: `src/http/middleware/error.rs`

```rust
use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpResponse,
};
use futures::future::LocalBoxFuture;

pub struct ErrorHandler;

impl<S, B> Transform<S, ServiceRequest> for ErrorHandler
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = ErrorHandlerMiddleware<S>;
    type InitError = ();
    type Future = std::future::Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        std::future::ready(Ok(ErrorHandlerMiddleware { service }))
    }
}

pub struct ErrorHandlerMiddleware<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for ErrorHandlerMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let fut = self.service.call(req);

        Box::pin(async move {
            match fut.await {
                Ok(res) => Ok(res),
                Err(err) => {
                    tracing::error!("Request error: {:?}", err);
                    Err(err)
                }
            }
        })
    }
}
```

**预期**: ~70 行代码

### Part 3: 路由和处理器 (2-3 天)

#### 3.1 路由配置

**文件**: `src/http/routes.rs`

```rust
use actix_web::web;
use super::handlers::{novels, chapters, runs};

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg
        // 小说路由 (6 个)
        .route("/novels", web::post().to(novels::create_novel))
        .route("/novels", web::get().to(novels::list_novels))
        .route("/novels/{id}", web::get().to(novels::get_novel))
        .route("/novels/{id}", web::put().to(novels::update_novel))
        .route("/novels/{id}", web::delete().to(novels::delete_novel))
        .route("/novels/{id}/experience", web::post().to(novels::set_creation_experience))
        
        // 章节路由 (5 个)
        .route("/novels/{novel_id}/chapters", web::post().to(chapters::create_chapter))
        .route("/novels/{novel_id}/chapters", web::get().to(chapters::list_chapters))
        .route("/chapters/{id}", web::get().to(chapters::get_chapter))
        .route("/chapters/{id}", web::put().to(chapters::save_chapter))
        .route("/chapters/{id}/promote", web::post().to(chapters::promote_chapter))
        
        // 运行路由 (2 个)
        .route("/runs", web::post().to(runs::start_run))
        .route("/runs/{id}/approve", web::post().to(runs::approve_run));
}
```

**预期**: ~25 行代码

#### 3.2 小说处理器

**文件**: `src/http/handlers/novels.rs`

```rust
use actix_web::{web, HttpResponse, Result};
use serde::{Deserialize, Serialize};
use novel_application::{usecase::NovelUseCases, dto::CreateNovelInput};

#[derive(Serialize, Deserialize)]
pub struct CreateNovelRequest {
    pub title: String,
    pub description: String,
    // ... 其他字段
}

pub async fn create_novel(
    req: web::Json<CreateNovelRequest>,
) -> Result<HttpResponse> {
    // 1. 验证请求
    if req.title.is_empty() {
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Title cannot be empty"
        })));
    }

    // 2. 转换为 UseCase 输入
    let input = CreateNovelInput {
        title: req.title.clone(),
        description: req.description.clone(),
        // ...
    };

    // 3. 调用 UseCase
    match NovelUseCases::create(&input) {
        Ok(novel) => Ok(HttpResponse::Created().json(novel)),
        Err(e) => Ok(HttpResponse::InternalServerError().json(serde_json::json!({
            "error": e.message
        }))),
    }
}

pub async fn list_novels() -> Result<HttpResponse> {
    match NovelUseCases::list() {
        Ok(novels) => Ok(HttpResponse::Ok().json(novels)),
        Err(e) => Ok(HttpResponse::InternalServerError().json(serde_json::json!({
            "error": e.message
        }))),
    }
}

pub async fn get_novel(path: web::Path<String>) -> Result<HttpResponse> {
    let id = path.into_inner();
    match NovelUseCases::get(&id) {
        Ok(novel) => Ok(HttpResponse::Ok().json(novel)),
        Err(e) => Ok(HttpResponse::NotFound().json(serde_json::json!({
            "error": e.message
        }))),
    }
}

pub async fn update_novel(
    path: web::Path<String>,
    req: web::Json<UpdateNovelRequest>,
) -> Result<HttpResponse> {
    let id = path.into_inner();
    // 实现更新逻辑
    Ok(HttpResponse::Ok().json(serde_json::json!({})))
}

pub async fn delete_novel(path: web::Path<String>) -> Result<HttpResponse> {
    let id = path.into_inner();
    match NovelUseCases::delete(&id) {
        Ok(_) => Ok(HttpResponse::NoContent().finish()),
        Err(e) => Ok(HttpResponse::NotFound().json(serde_json::json!({
            "error": e.message
        }))),
    }
}

pub async fn set_creation_experience(
    path: web::Path<String>,
    req: web::Json<SetExperienceRequest>,
) -> Result<HttpResponse> {
    let id = path.into_inner();
    // 实现设置经验等级的逻辑
    Ok(HttpResponse::Ok().json(serde_json::json!({})))
}
```

**预期**: ~110 行代码

#### 3.3 章节处理器

**文件**: `src/http/handlers/chapters.rs`

类似于小说处理器，包含 5 个端点：
- create_chapter
- list_chapters
- get_chapter
- save_chapter
- promote_chapter

**预期**: ~130 行代码

#### 3.4 运行处理器

**文件**: `src/http/handlers/runs.rs`

包含 2 个端点：
- start_run
- approve_run

**预期**: ~60 行代码

### Part 4: 集成测试 (1-2 天)

#### 4.1 HTTP 客户端测试

**文件**: `tests/http_integration_tests.rs`

```rust
use actix_web::{test, web, App};
use novel_application::http;

#[actix_web::test]
async fn test_create_novel() {
    let app = test::init_service(
        App::new()
            .configure(http::routes::configure)
    ).await;

    let req = test::TestRequest::post()
        .uri("/novels")
        .set_json(serde_json::json!({
            "title": "Test Novel",
            "description": "A test novel"
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::CREATED);
}

#[actix_web::test]
async fn test_list_novels() {
    let app = test::init_service(
        App::new()
            .configure(http::routes::configure)
    ).await;

    let req = test::TestRequest::get()
        .uri("/novels")
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::OK);
}

// 更多测试...
```

**预期**: 15+ 个 HTTP 集成测试

#### 4.2 端到端测试

**文件**: `tests/e2e_tests.rs`

测试完整的工作流：
1. 创建小说 → 验证返回
2. 列出小说 → 验证包含创建的小说
3. 创建章节 → 验证返回
4. 启动运行 → 验证返回
5. 批准运行 → 验证返回

**预期**: 5+ 个 E2E 测试

### Part 5: 文档 (1 天)

#### 5.1 API 端点文档

**文件**: `docs/API.md`

```markdown
# API 文档

## 小说端点

### POST /novels
创建新小说

**请求**:
```json
{
  "title": "新小说",
  "description": "描述"
}
```

**响应** (201 Created):
```json
{
  "id": "novel-123",
  "title": "新小说",
  "created_at": "2026-10-20T01:00:00Z"
}
```

### GET /novels
列出所有小说

**响应** (200 OK):
```json
[
  {
    "id": "novel-123",
    "title": "小说1",
    "created_at": "2026-10-20T01:00:00Z"
  }
]
```

// 更多端点...
```

**预期**: 5-10 页详细的 API 文档

---

## 实现计划

### 时间表

```
Day 1-2: Part 1 (项目配置)
  • 配置 Cargo 依赖
  • 创建项目结构
  • 实现基础应用程序

Day 3-4: Part 2 (中间件)
  • 实现日志中间件
  • 实现错误处理中间件
  • 测试中间件

Day 5-7: Part 3 (路由和处理器)
  • 配置路由
  • 实现小说处理器 (6 个)
  • 实现章节处理器 (5 个)
  • 实现运行处理器 (2 个)

Day 8: Part 4 (集成测试)
  • 编写 HTTP 集成测试
  • 编写 E2E 测试
  • 验证所有端点

Day 9: Part 5 (文档)
  • 编写 API 文档
  • 编写集成指南
  • 整理使用示例
```

**总计**: 6-8 工作天

---

## 成功标准

### 代码质量

- [ ] 0 个编译错误
- [ ] 所有 13 个端点实现
- [ ] 代码通过 Rust 审查
- [ ] 没有 unsafe 代码
- [ ] 遵循项目约定

### 测试覆盖

- [ ] 15+ HTTP 集成测试
- [ ] 5+ E2E 测试
- [ ] 所有端点测试通过
- [ ] 错误情况测试覆盖
- [ ] 性能测试基准建立

### 文档

- [ ] API 文档完整
- [ ] 请求/响应示例完整
- [ ] 错误响应文档
- [ ] 使用示例提供

### 验证

- [ ] 应用能启动
- [ ] 所有路由可访问
- [ ] 响应格式正确
- [ ] 错误处理正确
- [ ] 中间件工作正常

---

## 风险评估

| 风险 | 概率 | 影响 | 缓解 |
|-----|------|------|------|
| actix-web 学习曲线 | 中 | 中 | 参考官方文档，参考示例 |
| 与 Phase 3.0 集成 | 低 | 高 | 充分的集成测试 |
| 性能问题 | 低 | 中 | 性能测试，优化 |
| 部署问题 | 低 | 中 | Docker 化准备 |

---

## 与 Phase 3.0 的集成

### 如何连接

```
Phase 3.0 架构        Phase 4.1 实现
────────────────────────────────────
UseCase 层            actix-web 处理器
        ↓                     ↓
AppError              HTTP 响应
        ↓                     ↓
HTTP 处理器框架       路由和中间件
        ↓                     ↓
AppContext            服务器配置
```

### 关键集成点

1. **请求入口**: HTTP 请求 → actix-web 路由 → 处理器
2. **业务逻辑**: 处理器调用 UseCase → 返回结果
3. **错误处理**: AppError → HTTP 错误响应
4. **配置管理**: AppConfig → 服务器配置

---

## 下一步 (Phase 4.2)

Task 4.1 完成后，Task 4.2 将：
- 设计数据库 schema
- 实现 SQLx 集成
- 实现 Repository 层
- 连接 HTTP 端点到数据库

---

**文档版本**: 1.0-design  
**状态**: 设计完成，准备实现  
**预计实现开始**: 2026-10-22
