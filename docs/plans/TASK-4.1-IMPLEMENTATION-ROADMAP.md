# Task 4.1 - Web 框架集成 实现路线图

**版本**: 1.0  
**状态**: 🚀 **准备启动**  
**计划开始**: 2026-10-22  
**预计完成**: 2026-10-26  
**工作量**: 5-6 天

---

## 一、Task 4.1 核心目标

### 1.1 功能目标

实现 **13 个 HTTP 端点**，建立完整的 **Web API 框架**，支持：

- ✅ 故事创建和管理
- ✅ 章节规划和执行
- ✅ 流式响应处理
- ✅ 错误恢复和重试
- ✅ 中断和恢复流程

### 1.2 质量目标

| 指标 | 目标 |
|-----|------|
| **端点数** | 13 个 |
| **测试用例** | 35+ 个 |
| **代码行数** | ~600 行 |
| **代码覆盖率** | > 85% |
| **测试通过率** | 100% |
| **编译错误** | 0 |
| **性能基准** | < 100ms (p95) |

### 1.3 技术栈

| 组件 | 版本 | 状态 |
|-----|------|------|
| Axum | 0.7.9 | 已集成 |
| Tokio | 1.53 | 已集成 |
| serde_json | 1.0.154 | 已集成 |
| sqlx | 0.8.6 | 已集成 |
| reqwest | 0.12.28 | 已集成 |

---

## 二、13 个 HTTP 端点详细规划

### 2.1 故事管理 (3 个端点)

#### 端点 1: 创建故事
```
POST /api/novels
请求: { title, synopsis, target_length }
响应: { novel_id, status, created_at }
测试: 3 个
```

**实现步骤**:
1. 接收请求参数验证
2. 调用 novel-application 层创建故事
3. 返回生成的故事 ID
4. 单元测试 (有效输入、无效输入、重复创建)

#### 端点 2: 获取故事详情
```
GET /api/novels/:novel_id
响应: { id, title, synopsis, status, chapters, metadata }
测试: 4 个
```

**实现步骤**:
1. 参数校验和安全检查
2. 查询 novel-platform 层获取故事信息
3. 返回完整故事信息
4. 单元测试 (存在、不存在、权限检查、缓存)

#### 端点 3: 更新故事
```
PUT /api/novels/:novel_id
请求: { title?, synopsis?, target_length? }
响应: { novel_id, updated_at, status }
测试: 3 个
```

**实现步骤**:
1. 参数验证
2. 部分更新支持
3. 返回更新后的信息
4. 单元测试 (更新各字段、非法值、并发)

---

### 2.2 章节规划 (4 个端点)

#### 端点 4: 创建章节计划
```
POST /api/novels/:novel_id/chapters/plan
请求: { chapter_count, chapter_length }
响应: { plan_id, chapters: [...], status }
测试: 4 个
```

**实现步骤**:
1. 验证故事存在和状态
2. 调用 novel-workflow 层规划章节
3. 返回完整的规划信息
4. 单元测试 (有效规划、边界值、并发)

#### 端点 5: 获取章节计划
```
GET /api/novels/:novel_id/chapters/plan
响应: { plan_id, chapters: [...], metadata }
测试: 3 个
```

**实现步骤**:
1. 获取章节规划
2. 包含进度和元数据
3. 返回格式化的规划数据
4. 单元测试 (存在、不存在、多版本)

#### 端点 6: 获取单个章节
```
GET /api/novels/:novel_id/chapters/:chapter_id
响应: { id, title, content, status, metadata }
测试: 4 个
```

**实现步骤**:
1. 查询章节详情
2. 包含内容和元数据
3. 返回完整章节信息
4. 单元测试 (存在、删除、草稿、发布)

#### 端点 7: 更新章节
```
PUT /api/novels/:novel_id/chapters/:chapter_id
请求: { content?, title?, status? }
响应: { chapter_id, updated_at, status }
测试: 3 个
```

**实现步骤**:
1. 验证章节存在性
2. 支持部分更新
3. 返回更新状态
4. 单元测试 (更新内容、更新状态、锁定检查)

---

### 2.3 生成和流式响应 (3 个端点)

#### 端点 8: 开始章节生成 (流式)
```
POST /api/novels/:novel_id/chapters/generate
请求: { chapter_id, prompt, temperature? }
响应: Server-Sent Events 流
       event: token
       data: { token, position }
测试: 5 个
```

**实现步骤**:
1. 验证并准备生成参数
2. 调用 novel-llm 层开始生成
3. 设置 SSE 响应头
4. 流式输出每个 token
5. 单元测试 (流式输出、中断、错误处理、超时)

#### 端点 9: 获取生成进度
```
GET /api/novels/:novel_id/chapters/generate/:task_id
响应: { task_id, status, progress, current_token_count }
测试: 4 个
```

**实现步骤**:
1. 查询生成任务状态
2. 返回进度信息
3. 支持轮询
4. 单元测试 (进行中、完成、失败、不存在)

#### 端点 10: 停止生成
```
POST /api/novels/:novel_id/chapters/generate/:task_id/stop
响应: { task_id, stopped_at, partial_content }
测试: 3 个
```

**实现步骤**:
1. 查找运行中的任务
2. 触发优雅停止
3. 返回已生成的内容
4. 单元测试 (正常停止、不存在、已完成)

---

### 2.4 错误恢复 (3 个端点)

#### 端点 11: 获取恢复点
```
GET /api/novels/:novel_id/recovery-points
响应: { points: [...], latest: {...} }
测试: 3 个
```

**实现步骤**:
1. 查询所有恢复点
2. 标记最新的恢复点
3. 返回可恢复的位置列表
4. 单元测试 (多个恢复点、没有恢复点、权限)

#### 端点 12: 从恢复点恢复
```
POST /api/novels/:novel_id/recovery-points/:point_id/resume
请求: { resume_from_step }
响应: { task_id, status, next_steps }
测试: 4 个
```

**实现步骤**:
1. 验证恢复点存在
2. 加载之前的状态
3. 启动恢复流程
4. 返回新的任务 ID
5. 单元测试 (正常恢复、无效点、权限检查)

#### 端点 13: 获取错误日志
```
GET /api/novels/:novel_id/errors
请求: { limit?, offset? }
响应: { errors: [...], total_count }
测试: 3 个
```

**实现步骤**:
1. 分页查询错误日志
2. 按时间排序
3. 包含错误详情
4. 单元测试 (分页、过滤、权限)

---

## 三、代码结构和组织

### 3.1 新增文件结构

```
crates/novel-http/
├── src/
│   ├── routes/                    # HTTP 路由层
│   │   ├── mod.rs                 # 路由导出
│   │   ├── novels.rs              # 故事管理 (3 个端点)
│   │   ├── chapters.rs            # 章节管理 (4 个端点)
│   │   ├── generation.rs          # 生成流 (3 个端点)
│   │   └── recovery.rs            # 恢复流程 (3 个端点)
│   │
│   ├── handlers/                  # 请求处理器
│   │   ├── mod.rs
│   │   ├── novel_handlers.rs      # 故事处理
│   │   ├── chapter_handlers.rs    # 章节处理
│   │   ├── generation_handlers.rs # 生成处理
│   │   └── recovery_handlers.rs   # 恢复处理
│   │
│   ├── response/                  # 响应格式
│   │   ├── mod.rs
│   │   ├── success_response.rs    # 成功响应
│   │   ├── error_response.rs      # 错误响应
│   │   └── stream_response.rs     # 流式响应
│   │
│   ├── request/                   # 请求格式
│   │   ├── mod.rs
│   │   ├── novel_requests.rs      # 故事请求
│   │   ├── chapter_requests.rs    # 章节请求
│   │   └── generation_requests.rs # 生成请求
│   │
│   ├── error.rs                   # HTTP 错误类型
│   ├── middleware.rs              # 中间件 (认证、日志)
│   └── lib.rs                     # 库根
│
└── tests/                         # 集成测试
    ├── novel_integration_test.rs
    ├── chapter_integration_test.rs
    ├── generation_integration_test.rs
    └── recovery_integration_test.rs
```

### 3.2 核心模块设计

#### 模块 1: routes/mod.rs (路由注册)
```rust
// 路由聚合和注册
pub fn mount_routes(app: &mut Router) {
    app.route("/api/novels", post(novels::create))
       .route("/api/novels/:id", get(novels::get))
       .route("/api/novels/:id", put(novels::update))
       // ... 其他路由
}
```

**代码行数**: ~80 行

#### 模块 2: handlers/novel_handlers.rs (故事处理)
```rust
// 故事相关的请求处理逻辑
pub async fn create_novel(req: CreateNovelRequest) -> Result<Json<NovelResponse>> {
    // 验证
    // 创建
    // 返回
}

pub async fn get_novel(id: Uuid) -> Result<Json<NovelResponse>> {
    // 查询
    // 返回
}

pub async fn update_novel(id: Uuid, req: UpdateNovelRequest) -> Result<Json<NovelResponse>> {
    // 验证
    // 更新
    // 返回
}
```

**代码行数**: ~120 行

#### 模块 3: handlers/generation_handlers.rs (流式生成)
```rust
// 流式生成处理
pub async fn generate_chapter(
    novel_id: Uuid,
    req: GenerateRequest,
) -> Result<impl IntoResponse> {
    // 准备 SSE 流
    // 调用 LLM
    // 返回 SSE 响应
}

pub async fn stop_generation(task_id: Uuid) -> Result<Json<StoppedResponse>> {
    // 查找任务
    // 停止生成
    // 返回已生成的内容
}
```

**代码行数**: ~150 行

#### 模块 4: response/stream_response.rs (SSE 流处理)
```rust
// 服务端事件流
pub struct EventStream {
    receiver: mpsc::Receiver<StreamEvent>,
}

impl EventStream {
    pub fn new() -> (Self, mpsc::Sender<StreamEvent>) {
        // 创建通道
    }

    pub async fn send_token(&mut self, token: String) {
        // 发送 token 事件
    }
}
```

**代码行数**: ~100 行

---

## 四、详细实现计划 (Day by Day)

### Day 1 (2026-10-22 周二)

**目标**: 建立 HTTP 框架和路由基础

**任务列表**:

- [ ] 09:00 - 项目启动会议 (15 min)
  - [ ] 回顾 Task 4.1 设计
  - [ ] 明确代码风格和规范
  - [ ] 确认工具和环境

- [ ] 09:15 - 创建 Phase 4.0 实现分支 (15 min)
  ```bash
  git checkout -b phase-4.0-implementation
  git push -u origin phase-4.0-implementation
  ```

- [ ] 09:30 - 创建 novel-http 的基础结构 (1 hour)
  - [ ] 创建 src/routes/mod.rs (路由聚合)
  - [ ] 创建 src/handlers/mod.rs (处理器聚合)
  - [ ] 创建 src/response/mod.rs (响应格式)
  - [ ] 创建 src/request/mod.rs (请求格式)

- [ ] 10:30 - 编写故事管理路由和处理器 (2 hours)
  - [ ] src/routes/novels.rs (路由定义)
  - [ ] src/handlers/novel_handlers.rs (处理逻辑)
  - [ ] 实现 POST /api/novels (创建)
  - [ ] 实现 GET /api/novels/:id (查询)
  - [ ] 实现 PUT /api/novels/:id (更新)

- [ ] 12:30 - 午休 (1 hour)

- [ ] 13:30 - 编写故事管理的单元测试 (1.5 hours)
  - [ ] 创建单元测试文件
  - [ ] 编写 3 个创建测试 (有效、无效、重复)
  - [ ] 编写 4 个查询测试 (存在、不存在、权限、缓存)
  - [ ] 编写 3 个更新测试 (字段、并发、验证)

- [ ] 15:00 - 代码审查和测试运行 (1 hour)
  ```bash
  cargo fmt
  cargo clippy -- -D warnings
  cargo test --lib routes::novels --lib handlers::novel
  cargo tarpaulin
  ```

- [ ] 16:00 - 提交第一天成果 (30 min)
  - [ ] 代码审查
  - [ ] 提交到分支
  - [ ] 准备下一天计划

**Day 1 代码行数预计**: ~200 行
**Day 1 测试用例预计**: 10 个 ✓

---

### Day 2 (2026-10-23 周三)

**目标**: 完成章节管理端点

**任务列表**:

- [ ] 09:00 - 晨会 (15 min)
  - [ ] 回顾 Day 1 进度
  - [ ] 确认 Day 2 计划

- [ ] 09:15 - 创建章节管理路由和处理器 (2 hours)
  - [ ] src/routes/chapters.rs
  - [ ] src/handlers/chapter_handlers.rs
  - [ ] 实现 POST /api/novels/:id/chapters/plan
  - [ ] 实现 GET /api/novels/:id/chapters/plan
  - [ ] 实现 GET /api/novels/:id/chapters/:cid
  - [ ] 实现 PUT /api/novels/:id/chapters/:cid

- [ ] 11:15 - 休息 (15 min)

- [ ] 11:30 - 章节管理的单元测试 (1.5 hours)
  - [ ] 创建 4 个规划测试
  - [ ] 创建 3 个查询测试
  - [ ] 创建 4 个单章测试
  - [ ] 创建 3 个更新测试

- [ ] 13:00 - 午休 (1 hour)

- [ ] 14:00 - 代码审查和验证 (1.5 hours)
  - [ ] 运行所有测试
  - [ ] 检查覆盖率
  - [ ] 性能基准测试
  - [ ] 修复任何问题

- [ ] 15:30 - 文档和提交 (30 min)
  - [ ] 更新模块文档
  - [ ] 提交到分支

**Day 2 代码行数预计**: ~200 行
**Day 2 测试用例预计**: 14 个 ✓

---

### Day 3 (2026-10-24 周四)

**目标**: 实现流式生成端点

**任务列表**:

- [ ] 09:00 - 晨会 (15 min)

- [ ] 09:15 - 创建生成路由和处理器 (2 hours)
  - [ ] src/routes/generation.rs
  - [ ] src/handlers/generation_handlers.rs
  - [ ] src/response/stream_response.rs
  - [ ] 实现 SSE 流处理
  - [ ] 实现 POST /api/novels/:id/chapters/generate
  - [ ] 实现 GET /api/novels/:id/chapters/generate/:task_id
  - [ ] 实现 POST /api/.../generate/:task_id/stop

- [ ] 11:15 - 休息 (15 min)

- [ ] 11:30 - 流式响应测试 (1.5 hours)
  - [ ] 创建 5 个生成流测试 (流输出、中断、超时)
  - [ ] 创建 4 个进度查询测试
  - [ ] 创建 3 个停止测试

- [ ] 13:00 - 午休 (1 hour)

- [ ] 14:00 - 集成测试和性能验证 (1.5 hours)
  - [ ] 端到端流式测试
  - [ ] 性能基准 (< 100ms p95)
  - [ ] 并发测试 (多个生成同时运行)

- [ ] 15:30 - 提交和验证 (30 min)

**Day 3 代码行数预计**: ~200 行
**Day 3 测试用例预计**: 12 个 ✓

---

### Day 4-5 (2026-10-25~10-26 周五~周六)

**目标**: 完成恢复流程和最终验证

**任务列表**:

- [ ] **周五 09:00 - 晨会 (15 min)**

- [ ] **周五 09:15 - 恢复流程实现 (2 hours)**
  - [ ] src/routes/recovery.rs
  - [ ] src/handlers/recovery_handlers.rs
  - [ ] 实现 3 个恢复端点

- [ ] **周五 11:15 - 恢复测试 (1.5 hours)**
  - [ ] 创建 3 + 4 + 3 = 10 个恢复测试

- [ ] **周五 13:00 - 午休 (1 hour)**

- [ ] **周五 14:00 - 整体验证 (1.5 hours)**
  - [ ] 完整测试套件运行
  - [ ] 覆盖率检查 (> 85%)
  - [ ] 编译检查和代码风格

- [ ] **周五 15:30 - 提交 Day 4 (30 min)**

- [ ] **周六 09:00 - 最终集成测试 (2 hours)**
  - [ ] 所有 13 个端点的集成测试
  - [ ] 并发和压力测试
  - [ ] 错误处理验证

- [ ] **周六 11:15 - 文档完善 (1 hour)**
  - [ ] 更新 API 文档
  - [ ] 编写集成指南
  - [ ] 更新 README

- [ ] **周六 12:30 - 最终验证 (30 min)**
  - [ ] 所有检查清单通过
  - [ ] 代码审查完成
  - [ ] 准备发布

---

## 五、测试计划详表

### 5.1 单元测试分布 (35+ 个)

| 模块 | 测试数 | 覆盖情况 |
|-----|--------|---------|
| 故事管理 | 10 | 3 + 4 + 3 |
| 章节管理 | 14 | 4 + 3 + 4 + 3 |
| 流式生成 | 12 | 5 + 4 + 3 |
| 恢复流程 | 10 | 3 + 4 + 3 |
| **总计** | **36** | ✓ |

### 5.2 测试类型

| 类型 | 数量 | 描述 |
|-----|------|------|
| 功能测试 | 25 | 正常业务流程 |
| 错误处理 | 8 | 异常和边界情况 |
| 并发测试 | 2 | 多任务同时运行 |
| 性能测试 | 1 | 基准和上限 |

### 5.3 测试覆盖率目标

```
覆盖率: > 85%
  • 路由层: 95% (关键路径)
  • 处理器: 90% (业务逻辑)
  • 响应格式: 85% (序列化)
```

---

## 六、质量保证检查清单

### 6.1 代码质量

```
□ cargo fmt           ✓ 代码格式化通过
□ cargo clippy        ✓ 无警告
□ cargo test --all    ✓ 所有测试通过
□ cargo tarpaulin     ✓ 覆盖率 > 85%
□ cargo doc           ✓ 文档生成成功
```

### 6.2 功能验证

```
□ 13 个端点全部实现
□ 35+ 个单元测试全部通过
□ 流式响应正确处理
□ 错误恢复流程验证
□ 并发测试通过
```

### 6.3 性能验证

```
□ 端点响应时间 < 100ms (p95)
□ 内存使用合理
□ 连接池正常
□ 流式传输稳定
```

### 6.4 文档完整性

```
□ API 文档完整
□ 集成指南更新
□ 代码注释清晰
□ README 更新
```

---

## 七、依赖和工具

### 7.1 Rust 工具链

```bash
rustc --version       # Rust 1.70+
cargo --version       # Cargo 1.70+
```

### 7.2 开发工具

```bash
cargo install cargo-watch     # 文件监视
cargo install cargo-tarpaulin # 覆盖率
cargo install cargo-criterion # 基准测试
```

### 7.3 测试命令

```bash
# 单元测试
cargo test --lib --all

# 集成测试
cargo test --test '*' --all

# 覆盖率
cargo tarpaulin --out Html

# 基准测试
cargo criterion --message-format quiet

# 完整检查
cargo build && cargo test && cargo fmt && cargo clippy
```

---

## 八、风险管理

### 8.1 可能的风险

| 风险 | 概率 | 影响 | 应对 |
|-----|------|------|------|
| 依赖版本冲突 | 中 | 中 | 提前集成测试 |
| 流式响应超时 | 低 | 高 | 完整的压力测试 |
| 并发问题 | 低 | 高 | 多线程测试覆盖 |
| 时间管理 | 中 | 中 | 灵活的冲刺规划 |

### 8.2 应急方案

```
如果 Day 1 进度慢:
  → 压缩 Day 2 的非关键功能
  → 延伸到周末完成

如果测试覆盖率不足:
  → 增加临界路径的测试
  → 使用 tarpaulin 报告指导

如果性能未达标:
  → 基准测试定位瓶颈
  → 异步优化和缓存
```

---

## 九、周检查点和里程碑

### 9.1 里程碑

| 时间 | 里程碑 | 验收标准 |
|-----|--------|---------|
| 2026-10-22 | 框架建立 | 3 个故事端点 ✓ |
| 2026-10-23 | 章节完成 | 7 个章节端点 ✓ |
| 2026-10-24 | 流式通过 | 10 个生成端点 ✓ |
| 2026-10-25 | 恢复完成 | 13 个端点全部 ✓ |
| 2026-10-26 | 最终验收 | 35+ 测试通过 ✓ |

### 9.2 周五 (2026-10-25) 检查点

**目标**: 13 个端点全部可运行，35+ 测试通过

```
检查清单:
  □ cargo build 无错误
  □ cargo test --all 通过
  □ 所有 13 个端点可调用
  □ SSE 流式响应正常
  □ 恢复流程验证完成
  □ 代码覆盖率 > 85%
  □ 文档完整性检查
```

---

## 十、下一阶段 (Task 4.2)

完成 Task 4.1 后，将立即开始：

**Task 4.2 - 数据库集成** (2026-10-29 启动)

- 3 个 Repository 实现
- 28+ 个测试
- ~400 行代码
- 预计 4-5 天完成

---

## 总结

**Phase 4.0 Task 4.1 - Web 框架集成**

✨ **目标明确**: 13 个端点 + 35+ 测试 + 6 天完成

✨ **计划详细**: Day-by-day 任务分解

✨ **质量保证**: 完整的测试计划和检查清单

✨ **风险管理**: 识别和应对计划

✨ **准备就绪**: 所有工具和流程已准备好

---

**准备就绪**: 🚀 **100% 准备**  
**启动时间**: 2026-10-22 09:00 AM  
**预期完成**: 2026-10-26 16:00 PM  
**下一阶段**: Task 4.2 (2026-10-29)

