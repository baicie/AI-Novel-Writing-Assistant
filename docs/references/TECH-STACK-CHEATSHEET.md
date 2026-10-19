# 技术栈速查手册 - AI Novel Writing Assistant

**版本**: 1.0  
**更新日期**: 2026-10-20  
**用途**: 快速参考所有技术栈和配置

---

## 一、编程语言和工具链

### Rust

```
版本: 1.75+ (稳定)
安装: https://rustup.rs/

验证编译器:
  rustc --version
  # 期望: rustc 1.75.0 或更高

安装工具:
  rustup component add rustfmt  # 代码格式化
  rustup component add clippy   # 代码检查
  cargo install cargo-tarpaulin # 覆盖工具
  cargo install cargo-audit     # 依赖审计
```

### 工具链命令速查

```bash
# 编译
cargo build              # 调试编译
cargo build --release   # 发布编译

# 测试
cargo test              # 运行所有测试
cargo test --lib       # 仅库测试
cargo test --doc       # 仅文档测试
cargo test -- --nocapture  # 显示输出

# 检查
cargo fmt              # 格式化代码
cargo clippy           # 代码检查
cargo audit            # 安全审计
cargo check            # 编译检查

# 文档
cargo doc --open       # 生成并打开文档

# 性能
cargo flamegraph        # 火焰图分析
cargo bench            # 基准测试
```

---

## 二、依赖栈

### 版本定义 (Cargo.toml)

#### Web 框架
```toml
actix-web = "4.4"          # 异步 Web 框架
actix-rt = "2.9"           # 运行时
tokio = { version = "1.35", features = ["full"] }  # 异步运行时
```

**选择理由**: actix-web 是 Rust 生态中性能最优的框架，支持异步、中间件完善

#### 数据库
```toml
sqlx = { 
  version = "0.7",
  features = ["runtime-tokio-native-tls", "sqlite", "postgres"]
}
```

**选择理由**: 编译时 SQL 检查，零开销，支持多数据库

#### 序列化
```toml
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

**选择理由**: 标准 JSON 序列化库，性能最优

#### 认证
```toml
jsonwebtoken = "9.2"       # JWT 处理
bcrypt = "0.15"            # 密码哈希
```

**选择理由**: 行业标准，安全算法，维护活跃

#### 监控
```toml
prometheus = "0.13"        # 指标收集
tracing = "0.1"            # 结构化日志
tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt", "json"] }
```

**选择理由**: 可观测性标准，支持分布式追踪

#### 时间和 UUID
```toml
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1.6", features = ["v4", "serde"] }
```

**选择理由**: 标准库，广泛使用，维护活跃

### 开发依赖

```toml
[dev-dependencies]
tokio-test = "0.4"         # 异步测试
mockall = "0.11"           # Mock 库
tempfile = "3.8"           # 临时文件
```

---

## 三、项目结构速查

### 源代码布局

```
src/
├── lib.rs              # 库入口，导出所有公有模块
├── dto/                # 数据传输对象层
│   ├── mod.rs
│   ├── novel_dto.rs
│   ├── chapter_dto.rs
│   ├── run_dto.rs
│   └── error_dto.rs
├── models/             # 领域模型层
│   ├── mod.rs
│   ├── novel.rs
│   ├── chapter.rs
│   └── run.rs
├── services/           # 服务层
│   ├── mod.rs
│   ├── novel_service.rs
│   ├── chapter_service.rs
│   └── run_service.rs
└── use_cases/          # UseCase 业务逻辑层
    ├── mod.rs
    ├── create_novel.rs
    ├── update_chapter.rs
    └── execute_run.rs

tests/                 # 测试代码
├── integration_tests.rs
└── e2e_tests.rs
```

### 分层职责

```
Layer 5: HTTP Handler (将实现在 Phase 4.1)
  ↓ HTTP 请求处理，参数解析，返回序列化
  
Layer 4: UseCase (Phase 3 已实现)
  ↓ 业务逻辑协调，事务管理
  
Layer 3: Service (Phase 3 已实现)
  ↓ 业务操作，数据验证
  
Layer 2: Domain Model (Phase 3 已实现)
  ↓ 领域对象，业务规则
  
Layer 1: DTO (Phase 3 已实现)
  ↓ 纯数据对象，无业务逻辑
```

---

## 四、编译命令速查

### 基本编译

```bash
# 检查编译（快速）
cargo check

# 编译调试版本（包含 debug 符号，可调试）
cargo build

# 编译发布版本（优化，可用于生产）
cargo build --release

# 强制重新编译
cargo clean && cargo build

# 只编译特定包
cargo build -p novel-server
```

### 发布优化

```bash
# 添加到 Cargo.toml [profile.release]
[profile.release]
opt-level = 3           # 最大优化
lto = true              # 链接时优化
codegen-units = 1       # 代码生成单元
strip = true            # 移除符号表
```

---

## 五、测试命令速查

### 运行测试

```bash
# 运行所有测试
cargo test

# 仅运行单元测试
cargo test --lib

# 仅运行集成测试
cargo test --test '*'

# 运行特定测试
cargo test test_name

# 显示测试输出
cargo test -- --nocapture

# 单线程运行（便于调试）
cargo test -- --test-threads=1

# 生成覆盖报告
cargo tarpaulin --out Html
```

### 性能测试

```bash
# 基准测试
cargo bench

# 火焰图分析（需要 cargo-flamegraph）
cargo flamegraph

# 内存分析
valgrind ./target/release/novel-server
```

---

## 六、代码质量命令速查

### 格式化和检查

```bash
# 查看将要做的格式化改动
cargo fmt -- --check

# 格式化所有代码
cargo fmt

# 运行 clippy 检查
cargo clippy

# 自动修复
cargo clippy --fix

# 检查文档测试
cargo test --doc

# 安全审计
cargo audit
```

### 文档生成

```bash
# 生成文档
cargo doc

# 生成文档并打开
cargo doc --open

# 包含私有项目的文档
cargo doc --document-private-items

# 生成 README.md
cargo doc --no-deps
```

---

## 七、开发工作流速查

### 每日工作流

```bash
# 1. 开始工作
git checkout -b task/4.x-feature-name

# 2. 编写代码
# 编辑文件...

# 3. 格式化
cargo fmt

# 4. 本地测试
cargo test

# 5. 代码检查
cargo clippy -- -D warnings

# 6. 提交
git add .
git commit -m "描述"

# 7. 推送
git push origin task/4.x-feature-name
```

### 调试工作流

```bash
# 在 VS Code 或 Cursor 中调试
# 需要安装 CodeLLDB 扩展

# 使用 println! 调试
println!("Debug: {:?}", variable);

# 使用 dbg! 宏
let result = dbg!(expensive_computation());

# 使用 log 库
log::debug!("Debug message");
```

---

## 八、数据库命令速查

### SQLx CLI

```bash
# 安装 SQLx CLI
cargo install sqlx-cli --no-default-features --features sqlite,postgres

# 创建数据库
sqlx database create

# 删除数据库
sqlx database drop

# 创建迁移
sqlx migrate add -r add_table_name

# 运行迁移
sqlx migrate run

# 回滚迁移
sqlx migrate revert

# 查看迁移状态
sqlx migrate --status

# 验证 SQL 查询
sqlx prepare
```

### SQLite 命令

```bash
# 打开数据库
sqlite3 data/novel.db

# 列出表
.tables

# 显示表结构
.schema novels

# 运行查询
SELECT * FROM novels LIMIT 10;

# 导出 CSV
.mode csv
.output data.csv
SELECT * FROM novels;
.output stdout

# 退出
.quit
```

### PostgreSQL 命令

```bash
# 连接数据库
psql -U user -d database_name

# 列出表
\dt

# 显示表结构
\d novels

# 列出所有数据库
\l

# 连接到数据库
\c database_name

# 执行 SQL 文件
\i file.sql

# 退出
\q
```

---

## 九、Docker 命令速查

### Docker 基础

```bash
# 构建镜像
docker build -t novel-server:latest .

# 运行容器
docker run -p 8080:8080 novel-server:latest

# 查看容器日志
docker logs container_id

# 停止容器
docker stop container_id

# 删除容器
docker rm container_id

# 查看镜像
docker images

# 删除镜像
docker rmi image_id
```

### Docker Compose

```bash
# 启动所有服务
docker-compose up

# 后台启动
docker-compose up -d

# 查看状态
docker-compose ps

# 查看日志
docker-compose logs -f

# 停止服务
docker-compose down

# 重建镜像
docker-compose build --no-cache

# 运行单个服务
docker-compose up app
```

---

## 十、环境变量速查

### 开发环境 (.env)

```bash
# 数据库
DATABASE_URL=sqlite://./data/novel.db
# 或 PostgreSQL
# DATABASE_URL=postgresql://user:password@localhost/novel

# 日志级别
RUST_LOG=info,actix_web=debug

# 服务器
SERVER_HOST=0.0.0.0
SERVER_PORT=8080

# JWT
JWT_SECRET=your-secret-key-change-in-production
JWT_EXPIRY=3600

# Prometheus
PROMETHEUS_PORT=9090
```

### 生产环境

```bash
# 使用强加密密钥
JWT_SECRET=$(openssl rand -base64 32)

# 使用 PostgreSQL
DATABASE_URL=postgresql://prod_user:strong_password@prod_db_host/novel_db

# 调整日志级别
RUST_LOG=warn

# 启用性能优化
RUST_BACKTRACE=1
```

---

## 十一、性能优化速查

### 编译优化

```toml
# Cargo.toml 配置
[profile.release]
opt-level = 3           # 最大优化
lto = true              # 链接时优化
codegen-units = 1       # 单一编码单元
strip = true            # 移除调试符号
split-debuginfo = "packed"  # 紧凑调试信息
```

### 运行时优化

```bash
# 启用性能统计
RUST_LOG=info cargo run --release

# 使用火焰图分析热点
cargo flamegraph --freq 99

# 内存分析
valgrind --leak-check=full ./target/release/novel-server

# CPU 分析
perf record -F 99 -p $(pgrep novel-server) -- sleep 60
perf report
```

### 缓存优化

```rust
// 实现缓存
let cache = MemoryCache::new();
let result = cache.get("key")
    .or_else(|| {
        let value = expensive_computation();
        cache.set("key", value.clone(), Duration::from_secs(300));
        value
    });
```

---

## 十二、调试技巧速查

### 日志调试

```rust
// 添加日志
log::info!("Processing novel: {}", id);
log::debug!("Novel data: {:?}", novel);
log::error!("Error: {}", error);

// 使用 tracing
use tracing::{info, debug, error, span, Level};

let span = span!(Level::DEBUG, "process_novel", %id);
let _enter = span.enter();
```

### 性能分析

```bash
# 编译时间分析
cargo build -Z timings

# 依赖大小分析
cargo bloat --release

# 编译调试信息大小
cargo build --release -v
```

### 错误调试

```rust
// 使用 Result 链
result
    .map_err(|e| {
        log::error!("Error: {}", e);
        e
    })?

// 使用 ? 操作符
let value = risky_operation()?;
```

---

## 十三、CI/CD 命令速查

### GitHub Actions 本地测试

```bash
# 安装 act 工具
brew install act

# 列出工作流
act -l

# 运行工作流
act push

# 运行特定工作流
act push -j test

# 查看详细信息
act push -v
```

### 自动化测试

```bash
# 运行完整测试套件
cargo test --all

# 生成覆盖报告
cargo tarpaulin --out Html --output-dir coverage

# 运行 linting
cargo fmt --check && cargo clippy -- -D warnings
```

---

## 十四、常见命令组合

### 完整构建流程

```bash
# 1. 清理
cargo clean

# 2. 更新依赖
cargo update

# 3. 检查代码
cargo fmt && cargo clippy

# 4. 编译
cargo build --release

# 5. 测试
cargo test

# 6. 文档
cargo doc

# 完全一键脚本
./scripts/full-build.sh
```

### 完整开发流程

```bash
# 1. 创建功能分支
git checkout -b feature/new-feature

# 2. 开发和测试
cargo fmt && cargo clippy && cargo test

# 3. 提交
git add . && git commit -m "Add new feature"

# 4. 推送
git push origin feature/new-feature

# 5. 创建 PR
gh pr create
```

---

## 十五、故障排除速查

### 常见问题

```bash
# 问题: 编译错误 - 找不到类型
解决: cargo update && cargo clean && cargo build

# 问题: 测试失败 - 数据库连接
解决: sqlx database create && sqlx migrate run

# 问题: 性能问题 - 响应缓慢
解决: cargo flamegraph 分析 && 添加缓存

# 问题: 内存泄漏
解决: valgrind --leak-check=full ./target/release/app

# 问题: 文件权限
解决: chmod +x script.sh && ./script.sh
```

### 有用的诊断命令

```bash
# 检查 Rust 安装
rustc --version && cargo --version

# 检查工具链
rustup show

# 检查依赖
cargo tree

# 检查项目配置
cargo metadata

# 生成依赖图
cargo tree --graph > deps.txt
```

---

## 十六、生产部署速查

### 发布前检查

```bash
# 1. 运行完整测试
cargo test --all --release

# 2. 代码检查
cargo clippy -- -D warnings

# 3. 安全审计
cargo audit

# 4. 生成文档
cargo doc --release

# 5. 构建发布版本
cargo build --release

# 6. 检查文件大小
ls -lh target/release/novel-server
```

### 容器部署

```bash
# 1. 构建镜像
docker build -t novel-server:1.0.0 .

# 2. 测试镜像
docker run --rm novel-server:1.0.0

# 3. 标记版本
docker tag novel-server:1.0.0 registry.example.com/novel-server:1.0.0

# 4. 推送到仓库
docker push registry.example.com/novel-server:1.0.0

# 5. 部署
docker-compose -f docker-compose.prod.yml up -d
```

---

## 十七、快速参考表

### 工具对照表

| 功能 | 命令 | 说明 |
|-----|------|------|
| 编译 | `cargo build` | 编译项目 |
| 测试 | `cargo test` | 运行测试 |
| 检查 | `cargo check` | 快速检查 |
| 格式 | `cargo fmt` | 格式化代码 |
| 检查 | `cargo clippy` | 代码检查 |
| 文档 | `cargo doc --open` | 生成文档 |
| 审计 | `cargo audit` | 安全审计 |
| 清理 | `cargo clean` | 清理构建 |

### 依赖版本表

| 库 | 版本 | 用途 |
|----|------|------|
| actix-web | 4.4 | Web 框架 |
| tokio | 1.35 | 异步运行时 |
| sqlx | 0.7 | 数据库访问 |
| serde | 1.0 | 序列化 |
| jsonwebtoken | 9.2 | JWT |
| prometheus | 0.13 | 监控 |
| tracing | 0.1 | 日志 |

---

## 总结

这份手册提供了：

✅ 完整的命令速查  
✅ 常见工作流  
✅ 故障排除指南  
✅ 性能优化技巧  
✅ 生产部署清单  

**最常用命令**:
```bash
cargo fmt && cargo clippy && cargo test
```

**紧急恢复**:
```bash
cargo clean && cargo build && cargo test
```

---

**手册版本**: 1.0  
**最后更新**: 2026-10-20  
**保存位置**: `docs/references/TECH-STACK-CHEATSHEET.md`

使用此手册可快速查找所有技术栈相关的命令和配置！
