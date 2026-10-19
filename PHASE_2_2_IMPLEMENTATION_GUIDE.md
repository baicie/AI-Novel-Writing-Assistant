# Phase 2.2 实现指南 - 工作流通道与事件流

**目标**: 实现打开运行的完整流程和事件流端点，为自动导演的核心工作流链路做准备。

## 前置条件

- ✓ Phase 2.1 完成（list_runs、get_chapter_full）
- ✓ worker.rs 中的后台任务循环已实现
- ✓ DirectorService 中的 open_run 基础框架已就位

## 核心任务

### 任务 1: 完善 open_run 的错误处理和状态初始化

**目标文件**: `crates/novel-application/src/director.rs`

**当前状态**: DirectorService::open_run 已经调用 WorkflowRepository::open_run，但缺少：
- 章节存在性验证
- 运行冲突检测的更好提示
- 初始事件的记录

**实现步骤**:

```rust
pub async fn open_run(&self, input: OpenRunInput) -> Result<CommandOutcome, AppError> {
    // 1. 验证章节存在
    let chapter = self.get_chapter(&input.novel_id, &input.target_chapter_id).await?;
    
    // 2. 验证提示词版本有效
    let prompt_version = input.prompt_versions
        .get("chapter_draft")
        .map(String::as_str)
        .unwrap_or("");
    if resolve("chapter_draft", prompt_version).is_err() {
        return Err(AppError::validation("正文提示词版本不存在。"));
    }
    
    // 3. 验证端点有效
    authorize_endpoint(&input.endpoint, input.allow_localhost_http)
        .map_err(|e| AppError::validation(e.to_string()))?;
    
    // 4. 调用存储层打开运行
    // WorkflowRepository::open_run 会：
    //   - 检查是否有进行中的运行（有则报 Conflict）
    //   - 创建 workflow_run 和 workflow_control 记录
    //   - 插入 run_started 事件
    
    Ok(self.runs.open_run(record).await?)
}
```

**验证**:
- 章节不存在时返回 404
- 同一小说内已有进行中的运行时返回 409 Conflict
- 提示词版本无效时返回 400 Validation
- 端点授权失败时返回 400 Validation

---

### 任务 2: 实现 events_after 方法，支持事件分页查询

**目标文件**: `crates/novel-storage-sqlite/src/workflow.rs`

**当前状态**: WorkflowRepository trait 中已定义 `events_after(run_id, after_seq)` 方法。

**实现逻辑**:

```rust
async fn events_after(
    &self,
    run_id: &str,
    after_seq: i64,
) -> Result<Vec<StoredEvent>, PortError> {
    let rows = sqlx::query(
        "SELECT run_id, seq, kind, step_id, payload_json, occurred_at
         FROM workflow_event
         WHERE run_id = ? AND seq > ?
         ORDER BY seq ASC",
    )
    .bind(run_id)
    .bind(after_seq)
    .fetch_all(&self.pool)
    .await
    .map_err(map_sql)?;

    let mut events = Vec::new();
    for row in rows {
        events.push(StoredEvent {
            run_id: row.try_get("run_id").map_err(map_sql)?,
            seq: row.try_get("seq").map_err(map_sql)?,
            kind: row.try_get("kind").map_err(map_sql)?,
            step_id: row.try_get("step_id").map_err(map_sql)?,
            payload_json: row.try_get("payload_json").map_err(map_sql)?,
            occurred_at: row.try_get("occurred_at").map_err(map_sql)?,
        });
    }
    Ok(events)
}
```

**用途**:
- 历史事件回放：after_seq = 0 获取全部事件
- 实时订阅：after_seq = 最后一条事件的 seq，用于断点续传

---

### 任务 3: 在 DirectorService 中添加 replay 方法

**目标文件**: `crates/novel-application/src/director.rs`

**实现**:

```rust
pub async fn replay(&self, run_id: &str, after_seq: i64) -> Result<Vec<GenerationEvent>, AppError> {
    let stored = self.runs.events_after(run_id, after_seq).await?;
    let mut events = Vec::new();
    for stored_event in stored {
        if let Some(event) = event_from_stored(
            &stored_event.run_id,
            stored_event.seq,
            &stored_event.kind,
            stored_event.step_id,
            &stored_event.payload_json,
            stored_event.occurred_at,
        ) {
            events.push(event);
        }
    }
    Ok(events)
}
```

---

### 任务 4: 实现 SSE 事件流路由

**目标文件**: `crates/novel-http/src/routes/runs.rs`

**新增函数**: `stream_events`（SSE 端点）

```rust
use axum::response::sse::{Event, KeepAlive, Sse};
use futures::stream::{self, Stream};
use std::time::Duration;

pub async fn stream_events(
    State(state): State<KernelState>,
    Path(run_id): Path<String>,
    Query(query): Query<StreamQuery>,
) -> HttpResult<Sse<impl Stream<Item = Result<Event, HttpError>>>> {
    // 1. 查询历史事件（如果 after_seq 为 0）
    let initial_events = state
        .runtime
        .director
        .replay(&run_id, query.after_seq)
        .await
        .map_err(HttpError::from)?;

    let run_id_clone = run_id.clone();
    let director = state.runtime.director.clone();
    
    // 2. 创建事件流（包括历史事件和订阅通道）
    let stream = {
        let initial = stream::iter(initial_events.into_iter().map(|e| {
            serde_json::to_string(&e)
                .ok()
                .map(|data| Event::default().data(data))
                .ok_or_else(|| HttpError::internal("事件序列化失败"))
        }));

        let subscription = stream::unfold(query.after_seq + initial_events.len() as i64, move |seq| {
            let run_id = run_id_clone.clone();
            let director = director.clone();
            async move {
                // 模拟长轮询（暂时实现）
                // 后续可改为 tokio broadcast channel
                tokio::time::sleep(Duration::from_millis(500)).await;
                
                match director.replay(&run_id, seq).await {
                    Ok(new_events) if !new_events.is_empty() => {
                        Some((
                            new_events.last().map(|e| e.seq().unwrap_or(seq)).unwrap_or(seq),
                            seq + new_events.len() as i64,
                        ))
                    }
                    _ => None,
                }
            }
        })
        .map(|seq| {
            serde_json::json!({"seq": seq})
                .to_string()
                .ok()
                .map(|data| Event::default().data(data))
                .ok_or_else(|| HttpError::internal("心跳序列化失败"))
        });

        stream::chain(initial, subscription)
    };

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamQuery {
    #[serde(default)]
    pub after_seq: i64,
}
```

**注意**: 当前实现使用长轮询模拟 SSE。后续可升级为 tokio broadcast channel 实现真正的实时推送。

---

### 任务 5: 注册 SSE 路由

**目标文件**: `crates/novel-http/src/lib.rs`

**修改**:

```rust
.route(
    "/api/runs/:rid/events",
    get(routes::runs::stream_events),  // 新增 SSE 端点
)
```

注意：保留现有的 `replay` 端点用于点对点查询，SSE 路由独占这个路径。

---

### 任务 6: 集成测试 - 事件流和重放

**目标文件**: `crates/novel-http/tests/integration.rs`

**新增测试**:

```rust
#[tokio::test]
async fn replay_returns_events_in_order() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();

    // 创建小说、章节
    // 打开运行（这会生成 run_started 事件）
    // 查询事件 with after_seq=0
    // 验证返回的事件序列
    
    todo!("实现测试");
}

#[tokio::test]
async fn sse_stream_begins_with_history() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();

    // 创建并打开运行
    // 订阅 SSE 事件流 with after_seq=0
    // 验证首先收到历史事件
    
    todo!("实现 SSE 集成测试");
}
```

---

## 架构决策

### 为什么分离 replay 和 SSE 端点？

- **replay** (`GET /api/runs/:rid/events?afterSeq=N`): 点对点查询，返回 JSON 数组，用于页面刷新或断点续传
- **SSE** (`GET /api/runs/:rid/events` 无参数): 流式端点，返回 text/event-stream，用于前端长连接订阅

### 为什么不立即用 tokio broadcast？

- 长轮询足以满足初期需求（响应延迟 <1s）
- broadcast 需要后台循环维护（Worker 需要扩展）
- 可作为 Phase 2.3 的性能优化

### 数据一致性

- 所有事件持久化到 workflow_event 表，seq 是 run_id 内的单调递增主键
- events_after(run_id, seq) 保证 seq > after_seq，避免重复
- 运行启动时自动插入 run_started 事件（seq=1）

---

## 验收标准

### 功能验收

- [ ] `open_run` 完整验证链路（章节、提示词、端点）
- [ ] `events_after` 返回正确的事件序列
- [ ] `replay` 端点返回 JSON 数组，支持 after_seq 参数
- [ ] SSE 端点返回 text/event-stream，支持历史回放
- [ ] 新增 2+ 集成测试覆盖核心场景

### 编译验证

- [ ] `cargo check` 通过
- [ ] `cargo test --lib` 通过
- [ ] `cargo test --test integration` 通过（新增测试）

### 集成验收

- [ ] 打开运行后能通过 replay 获取事件
- [ ] 前端长连接到 SSE 端点能接收事件流
- [ ] 事件序列号单调递增，无遗漏

---

## 时间估计

- 任务 1-3: 业务逻辑 (1 天)
- 任务 4-5: SSE 路由 + 测试 (1.5 天)
- 任务 6: 集成测试 (1 天)
- **Phase 2.2 总计: 3-3.5 天**

---

## 后续工作

- **Phase 2.2 完成后**: 可启动 Phase 2.3（操作决策端点）
- **性能优化**: 升级到 tokio broadcast 实时推送（需后台 Worker 支持）
- **前端集成**: 对接运行记录页的事件流订阅

EOF
