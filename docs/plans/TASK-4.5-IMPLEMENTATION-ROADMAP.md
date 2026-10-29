# Task 4.5 - 文档与部署 实现路线图

**版本**: 1.0 | **状态**: 🚀 **计划中**  
**计划**: 2026-11-19 ~ 2026-11-23 | **工作量**: 1-2 天

---

## 一、核心目标

### 1.1 功能目标

- ✅ Docker 容器化
- ✅ Docker Compose 编排
- ✅ CI/CD 流程
- ✅ API 文档 (OpenAPI/Swagger)
- ✅ 部署指南

### 1.2 质量目标

| 指标 | 目标 |
|-----|------|
| **Dockerfile** | 1 个 |
| **Docker Compose** | 1 个 |
| **CI/CD 工作流** | 3 个 |
| **测试用例** | 8+ 个 |
| **代码行数** | ~190 行 |
| **代码覆盖率** | > 85% |

---

## 二、部署系统设计

### 2.1 3 个主要组件

#### 1. Docker 镜像 (Dockerfile)
```dockerfile
FROM rust:1.70 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y \
    libssl3 ca-certificates sqlite3
COPY --from=builder /app/target/release/novel-http /usr/local/bin/
EXPOSE 8080
CMD ["novel-http"]
```

**特点**:
- 多阶段构建
- 最小化镜像大小
- 包含必要依赖

**代码行数**: ~30 行

#### 2. Docker Compose 编排 (docker-compose.yml)
```yaml
version: '3.8'

services:
  api:
    build: .
    ports:
      - "8080:8080"
    environment:
      DATABASE_URL: sqlite:///data/app.db
      LOG_LEVEL: info
      JWT_SECRET: ${JWT_SECRET}
    volumes:
      - ./data:/app/data

  prometheus:
    image: prom/prometheus:latest
    ports:
      - "9090:9090"
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml

  grafana:
    image: grafana/grafana:latest
    ports:
      - "3000:3000"
    environment:
      GF_SECURITY_ADMIN_PASSWORD: admin
```

**代码行数**: ~40 行

#### 3. CI/CD 工作流 (GitHub Actions)
```yaml
name: Test and Build

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test --all
      - run: cargo fmt -- --check
      - run: cargo clippy -- -D warnings

  build:
    runs-on: ubuntu-latest
    needs: test
    steps:
      - uses: actions/checkout@v3
      - run: docker build -t novel-http:${{ github.sha }} .
      - run: docker push novel-http:${{ github.sha }}
```

**代码行数**: ~50 行

### 2.2 OpenAPI 文档

```rust
// 使用 utoipa 生成 OpenAPI 文档
#[utoipa::path(
    post,
    path = "/api/novels",
    request_body = CreateNovelRequest,
    responses(
        (status = 201, description = "Novel created", body = NovelResponse),
        (status = 400, description = "Invalid input"),
    )
)]
pub async fn create_novel(req: CreateNovelRequest) -> Result<Json<NovelResponse>> {
    // ...
}
```

**代码行数**: ~80 行 (注解和生成)

---

## 三、实现计划

### Day 1 (2026-11-19)

**目标**: Docker 容器化

- [ ] Dockerfile 编写 - 30 分钟
- [ ] docker-compose.yml 编写 - 30 分钟
- [ ] 本地测试 - 1 小时
- [ ] 容器测试 (3 个) - 1 小时

**代码行数**: ~70 行 | **测试**: 3 个 ✓

---

### Day 2 (2026-11-20)

**目标**: CI/CD 和 API 文档

- [ ] GitHub Actions 工作流 - 1 小时
- [ ] OpenAPI 文档生成 - 1 小时
- [ ] Swagger UI 集成 - 30 分钟
- [ ] 文档测试 (5 个) - 1 小时

**代码行数**: ~120 行 | **测试**: 5 个 ✓

---

## 四、部署清单

### 4.1 前置检查
```
□ Rust 工具链 1.70+
□ Docker 和 Docker Compose
□ Git 配置完成
□ 密钥和配置准备好
```

### 4.2 构建验证
```
□ cargo build --release 成功
□ docker build 成功
□ docker-compose up 成功
□ 服务健康检查通过
```

### 4.3 运行时检查
```
□ HTTP 端点可访问
□ 数据库连接正常
□ Prometheus 指标收集
□ 日志输出正常
□ 健康检查端点 (/health) 响应 200
```

---

## 五、部署指南

### 5.1 本地部署
```bash
# 构建镜像
docker build -t novel-http:latest .

# 启动服务
docker-compose up -d

# 检查日志
docker-compose logs -f api
```

### 5.2 生产部署
```bash
# 登录仓库
docker login

# 推送镜像
docker tag novel-http:latest myregistry/novel-http:v1.0.0
docker push myregistry/novel-http:v1.0.0

# 在生产环境启动
docker-compose -f docker-compose.prod.yml up -d
```

### 5.3 健康检查
```bash
curl http://localhost:8080/health

# 预期响应
{
  "status": "ok",
  "version": "1.0.0",
  "timestamp": "2026-11-20T10:00:00Z"
}
```

---

## 六、完成标准

```
✅ Dockerfile 完整且优化
✅ docker-compose.yml 可运行
✅ 3 个 CI/CD 工作流配置完成
✅ 8+ 个部署测试通过
✅ API 文档自动生成
✅ Swagger UI 可访问
✅ 部署指南完整
✅ 健康检查端点工作
```

**启动**: 2026-11-19 | **完成**: 2026-11-23

