# Phase 4.0 实现启动指南

**文档日期**: 2026-10-20  
**目标**: 为 Phase 4.0 的实现提供完整的启动准备  
**状态**: 实现前完整准备

---

## 一、实现前检查清单

### 1.1 环境准备

- [ ] Rust 1.75+ 已安装
  ```bash
  rustc --version  # 验证版本
  cargo --version
  ```

- [ ] 必要工具已安装
  ```bash
  # 代码格式化
  rustfmt --version
  
  # Linter
  cargo clippy --version
  
  # 测试工具
  cargo test --version
  ```

- [ ] Git 工作流已准备
  ```bash
  git branch -a  # 确认 feature/rust-tasks-3.0 存在
  git status     # 确认工作树干净
  ```

### 1.2 依赖验证

关键依赖已在 Cargo.toml 中定义：

```toml
[dependencies]
# Web 框架
actix-web = "4.4"
actix-rt = "2.9"
tokio = { version = "1.35", features = ["full"] }

# 序列化
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# 数据库
sqlx = { version = "0.7", features = ["runtime-tokio-native-tls", "sqlite", "postgres"] }

# 认证
jsonwebtoken = "9.2"
bcrypt = "0.15"

# 监控
prometheus = "0.13"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt", "json"] }

# 其他
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1.6", features = ["v4", "serde"] }
```

- [ ] 依赖版本确认
- [ ] 兼容性检查完成
- [ ] 离线模式测试完成

### 1.3 架构理解

确保理解以下架构层次：

```
Layer 5: HTTP Handlers (actix-web)
  ↓ HTTP 请求处理
Layer 4: UseCase (业务逻辑)
  ↓ 调用服务
Layer 3: Service (服务层)
  ↓ 调用仓库
Layer 2: Domain Model (领域模型)
  ↓ 使用 DTO
Layer 1: DTO (数据传输)
```

- [ ] 5 层架构理解清晰
- [ ] 职责分离明确
- [ ] 集成点确认

---

## 二、Task 4.1 启动指南

### 2.1 项目初始化

```bash
# 1. 创建新分支
git checkout -b task/4.1-web-framework

# 2. 创建项目结构
cargo new novel-server --bin

# 3. 进入目录
cd novel-server

# 4. 检查编译
cargo build
```

### 2.2 主要任务分解

**Part 1: 项目配置** (Day 1)
```
□ src/main.rs - 应用入口
□ src/config/mod.rs - 配置模块
□ src/config/server.rs - 服务器配置
□ Cargo.toml - 依赖定义
□ .env.example - 环境模板
```

**Part 2: 中间件实现** (Day 1-2)
```
□ src/middleware/mod.rs - 中间件模块
□ src/middleware/logging.rs - 日志中间件
□ src/middleware/error.rs - 错误处理
□ src/middleware/cors.rs - CORS 支持
```

**Part 3: 处理器框架** (Day 2-3)
```
□ src/handlers/mod.rs - 处理器模块
□ src/handlers/novels.rs - 小说端点
□ src/handlers/chapters.rs - 章节端点
□ src/handlers/runs.rs - 运行端点
```

**Part 4: 应用组装** (Day 3)
```
□ src/app.rs - 应用配置
□ src/router.rs - 路由定义
□ 端点注册和测试
```

**Part 5: 测试** (Day 4-5)
```
□ tests/integration_tests.rs
□ 端点集成测试
□ HTTP 客户端测试
□ E2E 工作流测试
```

### 2.3 代码框架

**main.rs 框架**:
```rust
mod config;
mod middleware;
mod handlers;
mod app;
mod router;

use actix_web::{web, App, HttpServer};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // 1. 初始化日志
    
    // 2. 加载配置
    
    // 3. 创建应用
    
    // 4. 启动服务器
    
    HttpServer::new(|| {
        App::new()
            // 添加中间件
            // 添加路由
    })
    .bind("0.0.0.0:8080")?
    .run()
    .await
}
```

### 2.4 验证检查点

Day 1 结束:
- [ ] 项目编译成功
- [ ] 配置加载正常
- [ ] 日志中间件工作

Day 3 结束:
- [ ] 13 个路由注册
- [ ] 处理器框架完成
- [ ] 编译无警告

Day 5 结束:
- [ ] 20+ 集成测试通过
- [ ] 5+ E2E 测试通过
- [ ] 文档完整

---

## 三、Task 4.2 启动指南

### 3.1 数据库初始化

```bash
# 1. 创建新分支
git checkout -b task/4.2-database

# 2. 初始化数据库
sqlx database create

# 3. 运行迁移
sqlx migrate run
```

### 3.2 Schema 准备

在 `sqlx-data.json` 中验证以下表结构：

```sql
-- 1. 小说表
CREATE TABLE novels (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  description TEXT,
  created_by TEXT NOT NULL,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- 2. 章节表
CREATE TABLE chapters (
  id TEXT PRIMARY KEY,
  novel_id TEXT NOT NULL,
  title TEXT NOT NULL,
  content TEXT,
  order_index INTEGER,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (novel_id) REFERENCES novels(id)
);

-- 3. 运行表
CREATE TABLE runs (
  id TEXT PRIMARY KEY,
  novel_id TEXT NOT NULL,
  input_parameters JSON,
  output_result JSON,
  status TEXT,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (novel_id) REFERENCES novels(id)
);

-- 4. 用户表
CREATE TABLE users (
  id TEXT PRIMARY KEY,
  email TEXT UNIQUE NOT NULL,
  password_hash TEXT NOT NULL,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- 索引
CREATE INDEX idx_novels_created_by ON novels(created_by);
CREATE INDEX idx_chapters_novel_id ON chapters(novel_id);
CREATE INDEX idx_runs_novel_id ON runs(novel_id);
```

### 3.3 Repository 实现

主要类需要实现：

```rust
pub struct NovelRepository {
    pool: SqlitePool,
}

impl NovelRepository {
    pub async fn create(&self, novel: &Novel) -> Result<Novel>;
    pub async fn get_by_id(&self, id: &str) -> Result<Option<Novel>>;
    pub async fn list(&self) -> Result<Vec<Novel>>;
    pub async fn update(&self, novel: &Novel) -> Result<Novel>;
    pub async fn delete(&self, id: &str) -> Result<()>;
}
```

- [ ] NovelRepository 完成
- [ ] ChapterRepository 完成
- [ ] RunRepository 完成
- [ ] 30+ Repository 测试通过

### 3.4 验证检查点

Day 5 结束:
- [ ] 数据库创建成功
- [ ] 迁移脚本执行成功
- [ ] 连接池配置正确
- [ ] Repository 实现完整

Day 8-10 结束:
- [ ] 30+ Repository 测试通过
- [ ] CRUD 操作验证
- [ ] 数据一致性检查通过

---

## 四、Task 4.3 启动指南

### 4.1 JWT 实现

创建 `src/auth/jwt_config.rs`:

```rust
pub struct Claims {
    pub sub: String,      // 用户ID
    pub email: String,
    pub exp: i64,         // 过期时间
    pub iat: i64,         // 签发时间
    pub role: String,     // 角色
}

pub struct JwtConfig {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    token_expiry: Duration,
}

impl JwtConfig {
    pub fn generate_token(&self, ...) -> Result<String>;
    pub fn verify_token(&self, token: &str) -> Result<Claims>;
}
```

### 4.2 认证端点

实现以下端点：

```
POST /auth/login       - 用户登录
POST /auth/register    - 用户注册
POST /auth/refresh     - 刷新 token
GET  /auth/me          - 获取当前用户
```

### 4.3 中间件实现

```rust
pub struct AuthMiddleware;

impl AuthMiddleware {
    // 从请求头提取 JWT token
    // 验证 token 有效性
    // 将 claims 存入请求扩展
}
```

### 4.4 权限检查

```rust
pub fn require_role(role: &str) -> impl Fn(HttpRequest) -> Result<()>;
pub fn require_auth() -> impl Fn(HttpRequest) -> Result<()>;
```

- [ ] JWT 生成和验证通过
- [ ] 登录/注册端点工作
- [ ] 中间件集成正常
- [ ] 20+ 认证测试通过

---

## 五、Task 4.4 启动指南

### 5.1 Prometheus 集成

```bash
# 1. 添加依赖
cargo add prometheus

# 2. 初始化指标
let metrics = Metrics::new()?;

# 3. 暴露 /metrics 端点
```

### 5.2 关键指标

```
http_requests_total           - 请求总数
http_request_duration_seconds - 请求延迟
http_request_size_bytes       - 请求大小
http_response_size_bytes      - 响应大小
active_connections            - 活跃连接
database_query_duration       - 数据库查询时间
```

### 5.3 缓存实现

```rust
pub struct MemoryCache<T: Clone> {
    data: Arc<RwLock<HashMap<String, CacheEntry<T>>>>,
}

impl<T> MemoryCache<T> {
    pub fn get(&self, key: &str) -> Option<T>;
    pub fn set(&self, key: String, value: T, ttl: Duration);
    pub fn invalidate(&self, key: &str);
}
```

### 5.4 性能优化

- [ ] 查询缓存实现 (缓存命中 > 50%)
- [ ] 数据库索引优化
- [ ] N+1 查询消除
- [ ] P99 延迟 < 100ms

---

## 六、Task 4.5 启动指南

### 6.1 Docker 配置

**Dockerfile**:
```dockerfile
FROM rust:1.75-alpine as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM alpine:3.19
COPY --from=builder /app/target/release/novel-server /app/
EXPOSE 8080
CMD ["./novel-server"]
```

### 6.2 Docker Compose

```yaml
version: '3.8'
services:
  app:
    build: .
    ports:
      - "8080:8080"
    environment:
      - DATABASE_URL=sqlite:///app/data/novel.db
      - RUST_LOG=info
  
  prometheus:
    image: prom/prometheus
    ports:
      - "9090:9090"
  
  grafana:
    image: grafana/grafana
    ports:
      - "3000:3000"
```

### 6.3 CI/CD 流程

**.github/workflows/ci.yml**:
```yaml
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test --lib
      - run: cargo clippy -- -D warnings
```

- [ ] Docker 镜像构建成功
- [ ] docker-compose 启动成功
- [ ] CI/CD 流程工作
- [ ] 所有文档完整

---

## 七、实现的关键原则

### 7.1 质量第一

```
□ 编译时检查: 0 编译错误，0 警告
□ 类型安全: 充分利用 Rust 类型系统
□ 测试覆盖: 每个功能都有测试
□ 文档完整: 每个模块都有文档注释
```

### 7.2 增量实现

```
□ 每个 Task 都有明确的验证检查点
□ 每天都有可验证的进度
□ 每周都完成一个完整的任务
□ 渐进式集成，减少风险
```

### 7.3 代码标准

```
□ 使用 rustfmt 格式化代码
□ 使用 clippy 检查代码质量
□ 遵循 Rust API 规范
□ 充分的错误处理
```

### 7.4 测试策略

```
□ 单元测试: 每个函数都有测试
□ 集成测试: 模块间交互测试
□ E2E 测试: 完整工作流测试
□ 性能测试: 关键路径性能验证
```

---

## 八、日常工作流程

### 8.1 每日开始

```bash
# 1. 拉取最新代码
git pull origin feature/rust-tasks-3.0

# 2. 创建任务分支
git checkout -b task/4.x-<task-name>

# 3. 确保编译通过
cargo build
```

### 8.2 每日结束

```bash
# 1. 运行所有测试
cargo test

# 2. 检查代码质量
cargo clippy -- -D warnings

# 3. 格式化代码
cargo fmt

# 4. 提交工作
git add .
git commit -m "进度: Task 4.x - <详细说明>"
```

### 8.3 每周检查

```bash
# 1. 拉取并合并主线
git pull origin feature/rust-tasks-3.0

# 2. 运行完整测试套件
cargo test --all

# 3. 生成测试报告
cargo tarpaulin --out Html
```

---

## 九、常见问题和解决方案

### Q1: 编译失败 - 找不到依赖

```
解决: cargo update && cargo build
```

### Q2: 测试失败 - 数据库连接错误

```
解决: 
1. 检查 DATABASE_URL 环境变量
2. 确保数据库已创建
3. 运行迁移脚本
```

### Q3: 中间件冲突 - 路由不匹配

```
解决:
1. 检查中间件顺序
2. 验证路由前缀
3. 查看错误日志
```

### Q4: 性能问题 - 响应缓慢

```
解决:
1. 检查 Prometheus 指标
2. 分析数据库查询
3. 使用 flamegraph 分析
```

---

## 十、交付物检查清单

### Phase 4.0 完成时：

- [ ] Task 4.1: 13 个端点 + 20+ 测试
- [ ] Task 4.2: Repository 层 + 30+ 测试
- [ ] Task 4.3: JWT + 认证 + 20+ 测试
- [ ] Task 4.4: Prometheus + 缓存
- [ ] Task 4.5: Docker + CI/CD + 文档
- [ ] 总代码: ~2,440 行
- [ ] 总测试: ~65 个
- [ ] 文档完整: 100%

### 最终验证：

- [ ] 编译错误: 0
- [ ] 测试通过: 100%
- [ ] 代码覆盖: > 85%
- [ ] API 文档: 完整
- [ ] 部署文档: 完整

---

## 十一、时间表确认

```
Week 1 (10-22): Task 4.1 Part 1-3 ← 开始这里
Week 2 (10-29): Task 4.1 完成 + Task 4.2 开始
Week 3 (11-05): Task 4.2 完成 + Task 4.3 开始
Week 4 (11-12): Task 4.3 完成 + Task 4.4 开始
Week 5 (11-19): Task 4.4 完成 + Task 4.5 开始
Week 6 (11-26): Task 4.5 完成
```

---

## 十二、成功标准

### 最小成功标准 (MVP)
- ✓ 所有端点实现
- ✓ 所有测试通过
- ✓ 0 编译错误
- ✓ 基本文档完成

### 理想成功标准
- ✓ 高代码覆盖 (> 85%)
- ✓ 优秀性能指标
- ✓ 完整的 API 文档
- ✓ 完善的运维文档

---

## 总结

这份启动指南为 Phase 4.0 的实现提供了：

✅ 完整的环境检查清单  
✅ 每个 Task 的详细启动步骤  
✅ 关键代码框架和示例  
✅ 日常工作流程  
✅ 质量保证标准  
✅ 时间表确认  

现在，系统已完全就绪！

**开始日期**: 2026-10-22  
**准备状态**: ✅ 完全就绪

🚀 **让我们开始实现吧！**

---

**文档完成日期**: 2026-10-20  
**下一步**: 执行 Phase 4.0 Task 4.1
