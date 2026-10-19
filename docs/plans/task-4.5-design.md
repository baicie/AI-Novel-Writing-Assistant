# Task 4.5 - 文档与部署详细设计

**日期**: 2026-10-20  
**阶段**: Phase 4.0 - 实现与集成  
**任务**: Task 4.5 - 文档与部署  
**目标**: 完整的 API 文档、Docker 容器化和部署脚本

---

## 任务概述

### 目标

准备应用程序用于生产部署，包括完整的 API 文档、Docker 容器化、CI/CD 流程和运维文档。

---

## Task 4.5 分解

### Part 1: OpenAPI 文档生成 (1 天)

#### 1.1 依赖配置

```toml
[dependencies]
utoipa = { version = "4.0", features = ["actix-web"] }
utoipa-swagger-ui = { version = "4.0", features = ["actix-web"] }
utoipa-redoc = { version = "4.0", features = ["actix-web"] }
```

#### 1.2 OpenAPI 配置

**文件**: `src/api/docs.rs`

```rust
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;
use utoipa_redoc::Redoc;

#[derive(OpenApi)]
#[openapi(
    paths(
        handlers::novels::create_novel,
        handlers::novels::list_novels,
        handlers::novels::get_novel,
        handlers::novels::update_novel,
        handlers::novels::delete_novel,
        handlers::chapters::create_chapter,
        handlers::chapters::list_chapters,
        handlers::runs::start_run,
    ),
    components(schemas(
        CreateNovelRequest,
        NovelResponse,
        ChapterResponse,
        RunResponse,
        ErrorResponse,
    )),
    tags(
        (name = "Novels", description = "小说管理接口"),
        (name = "Chapters", description = "章节管理接口"),
        (name = "Runs", description = "运行管理接口"),
    )
)]
pub struct ApiDoc;

pub fn swagger_ui() -> SwaggerUi {
    SwaggerUi::new("/swagger-ui")
        .url("/api-docs/openapi.json", ApiDoc::openapi())
}

pub fn redoc() -> Redoc {
    Redoc::new("/api-docs/openapi.json")
        .url("/redoc")
}
```

**预期**: ~50 行代码

#### 1.3 端点装饰器

```rust
#[utoipa::path(
    post,
    path = "/novels",
    request_body = CreateNovelRequest,
    responses(
        (status = 201, description = "小说创建成功", body = NovelResponse),
        (status = 400, description = "请求参数错误", body = ErrorResponse),
        (status = 500, description = "服务器错误", body = ErrorResponse),
    ),
    tags = ["Novels"]
)]
pub async fn create_novel(req: web::Json<CreateNovelRequest>) -> Result<HttpResponse> {
    // 实现...
}
```

**预期**: 应用到所有 13 个端点

---

### Part 2: Docker 容器化 (1 天)

#### 2.1 Dockerfile

**文件**: `Dockerfile`

```dockerfile
# 构建阶段
FROM rust:1.75-alpine as builder

WORKDIR /usr/src/app

# 安装依赖
RUN apk add --no-cache \
    musl-dev \
    pkgconfig \
    openssl-dev

# 复制源代码
COPY . .

# 构建应用
RUN cargo build --release

# 运行阶段
FROM alpine:3.19

WORKDIR /app

# 安装运行时依赖
RUN apk add --no-cache \
    ca-certificates \
    tzdata

# 从构建阶段复制二进制文件
COPY --from=builder /usr/src/app/target/release/novel-server /app/

# 创建日志目录
RUN mkdir -p /app/logs

# 暴露端口
EXPOSE 8080

# 设置环境变量
ENV RUST_LOG=info
ENV SERVER_HOST=0.0.0.0

# 启动应用
CMD ["./novel-server"]
```

**特点**:
- 多阶段构建，减少镜像大小
- Alpine 基础镜像，轻量级
- 非 root 用户运行 (可选)

#### 2.2 .dockerignore

```
.git
.gitignore
target/
.env
.env.local
README.md
docs/
tests/
.github/
```

---

### Part 3: Docker Compose 配置 (0.5 天)

**文件**: `docker-compose.yml`

```yaml
version: '3.8'

services:
  app:
    build: .
    container_name: novel-server
    ports:
      - "8080:8080"
    environment:
      - DATABASE_URL=sqlite:///app/data/novel.db
      - SERVER_HOST=0.0.0.0
      - SERVER_PORT=8080
      - RUST_LOG=info
      - JWT_SECRET=${JWT_SECRET:-your-secret-key}
    volumes:
      - ./data:/app/data
      - ./logs:/app/logs
    depends_on:
      - db
    restart: unless-stopped

  db:
    image: sqlite:latest
    # 或使用 PostgreSQL
    # image: postgres:16-alpine
    container_name: novel-db
    environment:
      POSTGRES_DB: novel
      POSTGRES_USER: user
      POSTGRES_PASSWORD: password
    volumes:
      - db-data:/var/lib/postgresql/data
    restart: unless-stopped

  prometheus:
    image: prom/prometheus:latest
    container_name: prometheus
    ports:
      - "9090:9090"
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml
      - prometheus-data:/prometheus
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'
    restart: unless-stopped

  grafana:
    image: grafana/grafana:latest
    container_name: grafana
    ports:
      - "3000:3000"
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=admin
    volumes:
      - grafana-data:/var/lib/grafana
    depends_on:
      - prometheus
    restart: unless-stopped

volumes:
  db-data:
  prometheus-data:
  grafana-data:
```

**预期**: ~70 行配置

---

### Part 4: CI/CD 配置 (1 day)

#### 4.1 GitHub Actions

**文件**: `.github/workflows/ci.yml`

```yaml
name: CI

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main, develop]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - uses: dtolnay/rust-toolchain@stable
      
      - uses: Swatinem/rust-cache@v2
      
      - name: Run tests
        run: cargo test --lib
      
      - name: Run clippy
        run: cargo clippy -- -D warnings
      
      - name: Check formatting
        run: cargo fmt -- --check

  build:
    needs: test
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - uses: dtolnay/rust-toolchain@stable
      
      - name: Build Docker image
        run: docker build -t novel-server:${{ github.sha }} .
      
      - name: Push to registry (if main)
        if: github.ref == 'refs/heads/main'
        run: |
          docker tag novel-server:${{ github.sha }} novel-server:latest
          # 推送到镜像仓库
```

**预期**: ~60 行配置

#### 4.2 部署流程

```yaml
deploy:
  needs: build
  runs-on: ubuntu-latest
  if: github.ref == 'refs/heads/main'
  steps:
    - name: Deploy to production
      run: |
        # SSH 到服务器
        ssh -i ${{ secrets.DEPLOY_KEY }} user@server
        # 拉取最新镜像
        docker pull novel-server:latest
        # 停止旧容器
        docker stop novel-server || true
        # 启动新容器
        docker run -d --name novel-server novel-server:latest
```

---

### Part 5: 部署指南 (0.5 天)

**文件**: `docs/DEPLOYMENT.md`

```markdown
# 部署指南

## 前置条件

- Docker 和 Docker Compose
- 4GB 内存
- 20GB 磁盘空间
- 网络访问

## 快速开始

### 1. 克隆仓库

```bash
git clone <repo-url>
cd novel-writing-assistant
```

### 2. 配置环境

```bash
cp .env.example .env
# 编辑 .env，设置必要的环境变量
```

### 3. 启动应用

```bash
docker-compose up -d
```

### 4. 验证

```bash
# 检查日志
docker-compose logs -f app

# 测试 API
curl http://localhost:8080/health
curl http://localhost:8080/swagger-ui
```

## 本地部署

### 1. 安装 Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### 2. 安装依赖

```bash
# macOS
brew install openssl pkg-config

# Linux (Ubuntu)
sudo apt-get install libssl-dev pkg-config
```

### 3. 构建和运行

```bash
cargo build --release
./target/release/novel-server
```

## 云部署 (AWS EC2)

### 1. 启动 EC2 实例

```bash
# t3.medium 实例
aws ec2 run-instances \
  --image-id ami-xxxxx \
  --instance-type t3.medium \
  --key-name my-key
```

### 2. SSH 连接

```bash
ssh -i my-key.pem ec2-user@instance-ip
```

### 3. 安装 Docker

```bash
sudo yum update
sudo yum install docker
sudo systemctl start docker
```

### 4. 部署应用

```bash
git clone <repo-url>
cd novel-writing-assistant
docker-compose up -d
```

## 监控和维护

### 查看日志

```bash
docker-compose logs app
docker-compose logs -f --tail=100 app
```

### 数据库备份

```bash
docker-compose exec db pg_dump -U user novel > backup.sql
```

### 性能监控

访问 Grafana: http://localhost:3000 (admin/admin)
访问 Prometheus: http://localhost:9090

## 故障排除

### 应用不启动

```bash
docker-compose logs app
# 检查错误信息
# 验证 DATABASE_URL
# 验证 JWT_SECRET
```

### 数据库连接失败

```bash
docker-compose exec app ping db
# 检查网络连接
```

### 性能问题

- 检查 CPU 和内存使用
- 查看 Prometheus 指标
- 优化数据库查询
```

---

### Part 6: 运维文档 (0.5 天)

**文件**: `docs/OPERATIONS.md`

```markdown
# 运维手册

## 常见任务

### 重启应用

```bash
docker-compose restart app
```

### 更新应用

```bash
git pull origin main
docker-compose build
docker-compose up -d
```

### 清理数据

```bash
docker-compose exec db rm -rf /app/data
docker-compose restart
```

## 监控告警

### 设置告警规则

```yaml
# prometheus-alerts.yml
groups:
  - name: app
    rules:
      - alert: HighErrorRate
        expr: rate(http_requests_total{status=~"5.."}[5m]) > 0.05
        for: 5m
        annotations:
          summary: "High error rate"

      - alert: HighLatency
        expr: histogram_quantile(0.99, http_request_duration_seconds) > 1
        for: 5m
        annotations:
          summary: "High p99 latency"

      - alert: LowAvailability
        expr: up{job="novel-server"} == 0
        for: 1m
        annotations:
          summary: "Service down"
```

## 容量规划

- 单个实例容纳 1000 并发连接
- 每个实例 ~500MB 内存占用
- 水平扩展: 使用 Kubernetes 或负载均衡器

## 灾难恢复

- 定期备份数据库
- 配置故障自动转移
- 保持最近 3 个快照
```

---

### Part 7: 快速开始指南 (0.5 天)

**文件**: `docs/QUICKSTART.md`

```markdown
# 快速开始

## 5 分钟内运行应用

### 方式 1: Docker Compose (推荐)

```bash
git clone <repo-url>
cd novel-writing-assistant
docker-compose up
```

访问应用: http://localhost:8080

### 方式 2: 本地开发

```bash
git clone <repo-url>
cd novel-writing-assistant
cargo build
cargo run
```

### 首次使用

1. 打开 http://localhost:8080/swagger-ui
2. 注册账户或登录
3. 创建小说
4. 创建章节
5. 启动运行

## API 示例

### 创建小说

```bash
curl -X POST http://localhost:8080/novels \
  -H "Content-Type: application/json" \
  -d '{
    "title": "我的小说",
    "description": "这是一部关于..."
  }'
```

### 获取小说列表

```bash
curl http://localhost:8080/novels
```

## 需要帮助?

- API 文档: http://localhost:8080/swagger-ui
- GitHub: https://github.com/...
- 问题追踪: https://github.com/.../issues
```

---

### Part 8: 性能调优指南 (0.5 天)

**文件**: `docs/PERFORMANCE.md`

```markdown
# 性能调优指南

## 基准指标

- 响应时间 P99: 100ms
- 吞吐量: 1000 req/s
- 错误率: < 0.1%

## 调优清单

### 数据库

- [ ] 添加索引
- [ ] 启用查询缓存
- [ ] 配置连接池
- [ ] 使用 EXPLAIN 分析查询

### 应用

- [ ] 启用缓存
- [ ] 使用异步处理
- [ ] 优化中间件
- [ ] 减少对象分配

### 部署

- [ ] 选择合适的服务器
- [ ] 配置 CPU 亲和性
- [ ] 启用 gzip 压缩
- [ ] 使用 CDN

## 监控指标

关键指标:
- HTTP 请求速率
- 响应时间分布
- 错误率
- 数据库查询时间
- 缓存命中率
```

---

### Part 9: 测试 (0.5 天)

- [ ] 端点可访问
- [ ] Swagger UI 工作
- [ ] Prometheus 暴露指标
- [ ] Grafana 显示数据
- [ ] Docker 镜像构建成功
- [ ] Docker Compose 启动无错误

---

## 完整时间表

```
Day 1: OpenAPI 文档
  • 依赖配置
  • 端点装饰器
  • Swagger UI 配置

Day 2: Docker 容器化
  • Dockerfile 编写
  • .dockerignore 配置
  • 镜像构建测试

Day 3: Docker Compose
  • 服务配置
  • 数据卷管理
  • 网络配置

Day 4: CI/CD
  • GitHub Actions 配置
  • 构建流程
  • 部署流程

Day 5: 文档
  • 部署指南
  • 运维手册
  • 快速开始
  • 性能调优

Day 6: 测试和验证
  • 本地验证
  • Docker 验证
  • 文档完整性检查
```

**总计**: 4-6 工作天

---

## 成功标准

- [ ] API 文档完整
- [ ] Docker 镜像构建成功
- [ ] Docker Compose 启动成功
- [ ] CI/CD 流程工作
- [ ] 所有文档完成
- [ ] 部署到测试环境成功

---

**预计开始**: 2026-11-27  
**预计完成**: 2026-12-03
