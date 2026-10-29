# Phase 2.2 任务分解 - SSE 实时推送升级

**状态**: 设计完成，待实现  
**分支**: `feature/rust-tasks-3.0`  
**总工作量**: 3 天  
**起始日期**: 2026-10-20  
**预期完成**: 2026-10-22

---

## 任务 1: 改造 replay 端点为 SSE 流（1 天）

**目标**: 将当前的 `GET /api/runs/:rid/events?afterSeq=N` 升级为支持 SSE 流式响应

**工作文件**:
- `crates/novel-http/src/routes/runs.rs` - 修改 replay 端点

**任务清单**:

- [ ] 1.1 添加 `stream` 参数到 ReplayQuery
  ```rust
  pub struct ReplayQuery {
      #[serde(default)]
      after_seq: i64,
      #[serde(default)]
      stream: bool,  // 新增
  }
  ```

- [ ] 1.2 改造 replay 函数支持两种模式
  ```rust
  pub async fn replay(...) -> HttpResult<impl IntoResponse> {
      if query.stream {
          sse_stream(...).await  // SSE 流
      } else {
          json_replay(...).await  // JSON 数组（兼容）
      }
  }
  ```

- [ ] 1.3 实现 sse_stream 函数
  - 使用 async_stream 宏生成流
  - 先发送历史事件
  - 进入长轮询模式（500ms 间隔）
  - 返回 Sse<...> 响应

- [ ] 1.4 实现 json_replay 函数（从现有逻辑提取）
  - 调用 director.replay()
  - 返回 JSON 数组

- [ ] 1.5 验证编译
  ```bash
  cargo check -p novel-http
  cargo build -p novel-http
  ```

**验收标准**:
- ✓ `cargo check` 通过
- ✓ `cargo build` 通过
- ✓ 可通过 `?stream=true` 参数切换模式

**估计时间**: 1 天

---

## 任务 2: 实现事件流内存缓存（0.5 天）

**目标**: 添加 DashMap 缓存以减少数据库查询

**工作文件**:
- `crates/novel-application/src/director.rs` - 添加 EventCache
- `crates/novel-application/src/lib.rs` - 导出 EventCache

**任务清单**:

- [ ] 2.1 定义 EventCache 结构
  ```rust
  pub struct EventCache {
      active_runs: DashMap<String, Vec<(i64, GenerationEvent)>>,
  }
  
  impl EventCache {
      pub fn new() -> Self { ... }
      pub fn record_event(...) { ... }
      pub fn get_events(...) -> Vec<GenerationEvent> { ... }
      pub fn clear_run(...) { ... }
  }
  ```

- [ ] 2.2 在 DirectorService 中集成缓存
  ```rust
  pub struct DirectorService {
      // ...
      cache: Arc<EventCache>,  // 新增
  }
  ```

- [ ] 2.3 修改 replay 方法优先查询缓存
  ```rust
  pub async fn replay(...) -> Result<Vec<GenerationEvent>, AppError> {
      // 1. 优先查缓存
      let cached = self.cache.get_events(run_id, after_seq);
      if !cached.is_empty() {
          return Ok(cached);  // 缓存命中
      }
      
      // 2. 缓存未命中，查数据库
      let stored = self.runs.events_after(run_id, after_seq).await?;
      // ...
  }
  ```

- [ ] 2.4 Worker 集成：每生成事件后调用缓存
  - 在 worker.rs 中调用 `director.cache.record_event(...)`
  - 或者在事件写入数据库后自动记录

- [ ] 2.5 验证编译
  ```bash
  cargo check -p novel-application
  cargo test --lib
  ```

**验收标准**:
- ✓ `cargo check` 通过
- ✓ `cargo test --lib` 通过
- ✓ 缓存大小受限（≤1000 条事件/运行）

**估计时间**: 0.5 天

---

## 任务 3: 前端断点续传支持（0.5 天）

**目标**: 在后端记录客户端已确认的事件，支持自动恢复

**工作文件**:
- `crates/novel-http/src/routes/runs.rs` - 新增 confirm_event 端点
- `crates/novel-http/src/lib.rs` - 注册路由

**任务清单**:

- [ ] 3.1 定义 ConfirmEventInput
  ```rust
  #[derive(Deserialize)]
  pub struct ConfirmEventInput {
      run_id: String,
      last_confirmed_seq: i64,
  }
  ```

- [ ] 3.2 实现 confirm_event 端点
  ```rust
  pub async fn confirm_event(
      State(_state): State<KernelState>,
      Json(input): Json<ConfirmEventInput>,
  ) -> HttpResult<Json<serde_json::Value>> {
      // 当前阶段简单返回成功
      // 后续可扩展为：
      // 1. 记录到数据库（用于崩溃恢复）
      // 2. 更新内存状态
      Ok(Json(json!({"ok": true})))
  }
  ```

- [ ] 3.3 在 lib.rs 中注册路由
  ```rust
  .route(
      "/api/runs/:rid/confirm",
      post(routes::runs::confirm_event),
  )
  ```

- [ ] 3.4 编写前端集成指南
  - 在 README 中添加前端 SSE 订阅示例
  - 说明 `afterSeq` 参数的用法
  - 说明 confirm 端点的调用时机

- [ ] 3.5 验证编译
  ```bash
  cargo check -p novel-http
  cargo build -p novel-http
  ```

**验收标准**:
- ✓ `cargo check` 通过
- ✓ `cargo build` 通过
- ✓ `/api/runs/:rid/confirm` 端点可调用

**估计时间**: 0.5 天

---

## 任务 4: 集成测试覆盖 SSE 流（1 天）

**目标**: 编写完整的集成测试验证 SSE 功能

**工作文件**:
- `crates/novel-http/tests/integration.rs` - 新增 SSE 测试

**任务清单**:

- [ ] 4.1 测试：JSON 轮询模式仍正常工作
  ```rust
  #[tokio::test]
  async fn json_replay_still_works() {
      // 创建运行
      // GET /api/runs/:rid/events?afterSeq=0
      // 验证返回 JSON 数组
  }
  ```

- [ ] 4.2 测试：SSE 流返回正确的 content-type
  ```rust
  #[tokio::test]
  async fn sse_stream_returns_event_stream() {
      // 创建运行
      // GET /api/runs/:rid/events?stream=true
      // 验证 content-type = text/event-stream
  }
  ```

- [ ] 4.3 测试：SSE 流包含历史事件
  ```rust
  #[tokio::test]
  async fn sse_stream_includes_history() {
      // 创建运行，产生若干事件
      // 订阅 SSE
      // 验证首先收到所有历史事件
  }
  ```

- [ ] 4.4 测试：SSE 流支持 afterSeq 断点续传
  ```rust
  #[tokio::test]
  async fn sse_stream_respects_after_seq() {
      // 创建运行，产生 5 个事件
      // 订阅 SSE ?afterSeq=2
      // 验证只收到 seq > 2 的事件
  }
  ```

- [ ] 4.5 测试：confirm_event 端点可调用
  ```rust
  #[tokio::test]
  async fn confirm_event_accepts_request() {
      // POST /api/runs/:rid/confirm
      // 验证返回 200 ok
  }
  ```

- [ ] 4.6 性能测试（可选，后续优化）
  - 缓存命中率（目标 >70%）
  - SSE 首条事件延迟（目标 <200ms）

- [ ] 4.7 运行所有测试
  ```bash
  cargo test --test integration
  cargo test --lib
  cargo build  # 完整构建
  ```

**验收标准**:
- ✓ 所有新测试通过
- ✓ 无现有测试失败
- ✓ `cargo build` 完整编译通过

**估计时间**: 1 天

---

## 总体时间线

| 日期 | 任务 | 工作量 | 状态 |
|------|------|--------|------|
| 10-20 | 任务 1 | 1 天 | ⏳ 待开始 |
| 10-20 下午 | 任务 2 | 0.5 天 | ⏳ 待开始 |
| 10-20 下午 | 任务 3 | 0.5 天 | ⏳ 待开始 |
| 10-21 ~ 10-22 | 任务 4 | 1 天 | ⏳ 待开始 |
| 10-22 下午 | 集成验证 | - | ⏳ 待开始 |

---

## 验收检查清单

### 编译与测试
- [ ] `cargo check -p novel-http` ✓
- [ ] `cargo check -p novel-application` ✓
- [ ] `cargo test --lib` ✓
- [ ] `cargo test --test integration` ✓
- [ ] `cargo build` ✓

### 功能验证
- [ ] JSON 模式 `GET /api/runs/:rid/events?afterSeq=0` → JSON 数组
- [ ] SSE 模式 `GET /api/runs/:rid/events?stream=true` → text/event-stream
- [ ] 历史事件回放完整
- [ ] after_seq 参数工作正确
- [ ] 新事件能被推送
- [ ] confirm 端点正常

### 代码质量
- [ ] 无 TODO 和临时代码
- [ ] 注释清晰
- [ ] 错误处理完善
- [ ] 单元测试覆盖

### 文档更新
- [ ] 更新 README 前端 SSE 使用说明
- [ ] 更新 wiki 事件流架构

---

## 下一步

完成 Phase 2.2 后：

1. **合并到 beta 分支**
   ```bash
   git checkout beta
   git merge feature/rust-tasks-3.0
   ```

2. **启动 Phase 2.3** - 操作决策端点
   - approve, cancel, resume 端点
   - 状态转移验证
   - 错误恢复流程

3. **性能优化** (Phase 2.3+)
   - 升级到 tokio broadcast
   - 事件聚合与批量提交
   - 缓存策略优化

---

**更新日期**: 2026-10-19 23:50 UTC+8  
**下次同步**: Phase 2.2 任务 1 完成时

