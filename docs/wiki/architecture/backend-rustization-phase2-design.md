# 后端 Rust 化 Phase 2 设计 - 创作核心工作流迁入

**状态**: 设计中  
**日期**: 2026-10-19  
**工作量**: 1-2 周  
**优先级**: 最高

## 背景

Phase 1（已完成）建立了 HTTP 框架和内核启动机制。现在 Rust 内核虽然能响应 25 个基础端点，但自动导演的关键工作流——章节生产链（draft → review → save）仍完全由 Node 承担。

Phase 2 的目标是把**完整的章节生产链路从规划 → 生成 → 审查 → 保存** 迁入 Rust，使得 Rust 模式下的自动导演能完整工作。

## 关键业务流程回顾

当前 Node 的章节生产链包括：

1. **打开运行 `POST /api/novels/:id/runs`**
   - 输入：目标章节、模型选择、预算、重试次数等
   - 输出：运行 ID、初始状态 (queued)

2. **代理生成 → 规划 → 初稿→ 审查 → 保存**（`src-tauri/src/lib.rs` 中 `open_run` IPC 命令已在 Rust 中实现 worker loop）
   - Worker 在后台循环推进状态机
   - 每个阶段产生事件流（SSE）
   - 中间失败时 `cancel_generation` / `resume_run` 可介入

3. **查询进度 & 事件 `GET /api/runs/:rid/events`**
   - 返回该运行从头到尾的事件流
   - Replay 场景：打开旧运行后重新拉事件

4. **操作决策**
   - `POST /api/runs/:rid/approve` - 同意当前草稿
   - `POST /api/runs/:rid/cancel` - 放弃整个运行
   - `POST /api/runs/:rid/resume` - 从暂停点继续
   - `POST /api/runs/:rid/cancel-generation` - 只放弃当前生成，重试

5. **查询运行 & 章节** 
   - `GET /api/novels/:id/runs` - 列表（需要）
   - `GET /api/chapters/:cid` - 获取已保存内容（需要）

## Phase 2 迁入范围

### 核心端点（必须）

1. **打开运行** `POST /api/novels/:id/runs`
   - 调用 `Runtime::open_run(...)`（已在 Rust 内核中）
   - 返回 `RunSnapshot` with 初始事件

2. **查询事件流** `GET /api/runs/:rid/events`
   - 从 `Kernel::run_events(rid)` 拉完整历史
   - 支持 EventSource (SSE) 重新连接

3. **操作决策** (4 个 POST 端点)
   - `/approve`, `/cancel`, `/resume`, `/cancel-generation`
   - 调用 `Runtime::approve_run()` / `cancel_run()` / `resume_run()` / `cancel_generation()`

4. **列表和详情** (需要支持的查询)
   - `GET /api/novels/:id/runs` - 返回该小说的所有运行
   - `GET /api/chapters/:id` - 返回章节内容（包含已保存的正文）

### 依赖补齐

- `novel-application` 需要 `list_runs(novel_id)` 方法
- `novel-application` 需要 `chapter_detail(chapter_id)` 返回完整章节与正文
- `novel-storage-sqlite` 需要实现对应的 Repository 方法

## 技术考量

### 1. 事件流处理

Rust 内核已有 `GenerationEvent` 和事件 hub（`EventHub` in `novel-application`）。HTTP 层需要：

```rust
// crates/novel-http/src/routes/runs.rs
pub async fn events(
    State(state): State<KernelState>,
    Path(rid): Path<String>,
) -> impl IntoResponse {
    // 历史事件：从数据库拉取
    let history = state.kernel.run_events(&rid)?;
    
    // 实时事件：订阅 EventHub 直到运行完成
    let stream = state.runtime.event_hub.subscribe_run(&rid);
    
    // 返回 EventStream response
}
```

关键是**SSE 重连处理**：如果客户端断开，下次连接时从 `events` 端点重新拉历史 + 续订。

### 2. 长期保存

- 运行事件存储在 `workflow_run` / `workflow_control` / `quality_debt` 三张表
- 生成的文本暂存在哪里？Node 用 Prisma 的 `draft` 字段；Rust 需要对应的存储位置
- **问题**：当前 `novel-storage-sqlite` 的 schema 可能没有这些临时保存字段

### 3. 模型调用

Rust 侧 `novel-application` 使用 `UnconfiguredProvider` 占位。实际生成由谁做？

**选项 A**（完全 Rust 化 - 不做）
- 实现 `novel-llm` 的真实 provider（OpenAI / Qwen / 等）
- Rust 内核直接发起 HTTP 调用
- 风险：密钥管理、多厂商支持不完整

**选项 B**（保持混合 - 推荐）
- Rust Worker 在生成阶段调用 Node 服务的 LLM 端点
- Node 保留 `POST /api/llm/generate` 等生成接口
- Rust 向 Node 后端请求生成，拿回流然后转发给客户端

这样 Rust 层可以专注于工作流编排，LLM 生成逻辑仍由 Node 承担（可独立迁移）。

### 4. 同步与冲突

切流期间，用户可能在 Node 模式打开运行，然后切到 Rust 模式。两个服务的数据库一致性怎么保证？

**答案**：严格禁止。Rust 和 Node 各自持有一份数据，初始化时只能导入一次 Prisma → SQLite。用户一旦在 Rust 模式启动，就不应该切回 Node 模式，因为新的运行只存在于 SQLite。

切流前的过渡期，可以禁用"打开运行"功能的 Node 路由，迫使用户用新模式。

## 阶段划分

### 小 Phase 2.1：基础存储与查询（3 天）

**范围**：
- `novel-application` 实现 `list_runs()`, `run_detail()`, `chapter_full()` 服务
- `novel-storage-sqlite` 实现对应 Repository 方法
- HTTP 端点：`GET /api/novels/:id/runs`, `GET /api/chapters/:cid` (已有 detail，补充 full 内容)
- 集成测试通过

**验收**：
- 列表查询返回正确的运行列表
- 详情查询返回完整章节对象（含正文、反馈等）

### 小 Phase 2.2：打开运行与事件流（3-4 天）

**范围**：
- HTTP 端点 `POST /api/novels/:id/runs` 调用 `Runtime::open_run()`
- HTTP 端点 `GET /api/runs/:rid/events` 返回事件流（SSE）
- 支持历史事件回放
- 实时事件流（需要 EventHub 的 tokio 通道支持）

**依赖**：
- Node 保留 LLM 生成接口，Rust Worker 在生成阶段会回调 Node
- 或：先做空载测试（不实际生成），只验证工作流状态机

**验收**：
- 打开运行成功，返回初始事件
- 事件流可持续拉取（SSE 保持连接）
- 历史重放可完整拉取

### 小 Phase 2.3：操作决策（2-3 天）

**范围**：
- `POST /api/runs/:rid/approve` 通过当前阶段
- `POST /api/runs/:rid/cancel` 放弃运行
- `POST /api/runs/:rid/resume` 继续运行
- `POST /api/runs/:rid/cancel-generation` 放弃当前生成

**验收**：
- 操作后状态转移正确
- 新的事件流反映操作结果
- 不允许的转移返回 400 / 409

### Phase 2 总计：1-2 周

## 代码结构

```
crates/novel-application/
├── src/
│   ├── director.rs → 新增 run_list, run_detail, list_runs
│   ├── chapter.rs → 新增或补充 chapter_full_details
│   └── runtime.rs → 已有 open_run，补充 event 查询接口

crates/novel-storage-sqlite/
├── src/
│   ├── runs.rs → 新增 Run repository impl
│   ├── chapters.rs → 补充 full chapter query
│   └── lib.rs → pub use

crates/novel-http/
├── src/
│   ├── routes/
│   │   ├── runs.rs → open, approve, cancel, resume, cancel-generation, events (已有，补充)
│   │   └── chapters.rs → detail 补充 full_details
│   └── handlers/
│       └── sse.rs → EventSource 流处理（新建）
```

## 关键决策

### Q: Rust Worker 如何回调 Node 的 LLM 生成？

**A**: 
```rust
// 在 Rust Worker 的生成阶段
let generation_request = GenerationInput { /* ... */ };
let mut response = http::Client::post("http://127.0.0.1:3000/api/llm/generate")
    .json(&generation_request)
    .send()
    .await?;

// 流式转发给客户端
while let Some(chunk) = response.chunk().await? {
    event_hub.emit(GenerationEvent::Output { text: chunk });
}
```

这样 Node 的 `/api/llm/generate` 保留为内部 API，不暴露给客户端。

### Q: EventHub 的 tokio 通道支持？

**A**: `novel-application/src/hub.rs` 已有 `tokio::sync::broadcast`。HTTP 路由可以 subscribe 并转换为 axum SSE stream。

### Q: 存储临时生成结果？

**A**: 暂不存储草稿。事件流本身就是历史。真实打开运行时，Worker 会重新生成。后续 Phase 3 可以加缓存。

## 风险和缓解

| 风险 | 缓解 |
|-----|-----|
| 工作流状态机转移错误 | 充分的单元测试，参考 Node 的现有逻辑 |
| EventHub SSE 连接掉线丢事件 | 客户端重连后从 DB 拉历史补全 |
| LLM 回调 Node 端失败 | 返回 5xx，客户端可 `resume_run` 重试 |
| Rust 和 Node 数据不一致 | 禁止切换模式（需前端卡死按钮或 dismiss 提示） |

## 后续工作追踪

- [ ] Phase 2.1 完成：列表查询、章节详情 E2E 测试通过
- [ ] Phase 2.2 完成：打开运行、事件流 E2E 测试通过
- [ ] Phase 2.3 完成：决策操作 E2E 测试通过
- [ ] Phase 2 集成测试全部通过
- [ ] 前端功能测试（导演页面、运行记录）
- [ ] 性能对标（Rust vs Node）

## 相关参考

- `docs/wiki/architecture/backend-rustization-roadmap.md` → Phase 2 完整规划
- `src-tauri/src/lib.rs` → `open_run` IPC 实现参考
- `novel-application/src/runtime.rs` → `Runtime` 和 Worker 现有实现
- `novel-application/src/hub.rs` → EventHub 结构
