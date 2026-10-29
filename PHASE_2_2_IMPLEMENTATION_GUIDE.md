# Phase 2.2 实现指南 - SSE 实时事件流升级

**目标**: 将当前的 JSON 轮询升级为真正的 Server-Sent Events，实现实时的事件推送。

**前置条件**:
- ✓ Phase 2.1 完成（open_run、events_after、replay 已实现）
- ✓ HTTP 路由已注册 `GET /api/runs/:rid/events`
- ✓ GenerationEvent 事件类型已定义

**核心任务**

### 任务 1: 改造 replay 端点为 SSE 流（1 天）

**当前状态**: `replay()` 返回 JSON 数组（整体响应）

**目标**: 支持两种响应模式
- 查询模式：`GET /api/runs/:rid/events?afterSeq=100` → JSON 数组（兼容旧前端）
- 流式模式：`GET /api/runs/:rid/events?stream=true` → text/event-stream（新前端）

**实现步骤**:

```rust
#[derive(Deserialize)]
pub struct ReplayQuery {
    #[serde(default)]
    after_seq: i64,
    #[serde(default)]
    stream: bool,  // 新增：是否流式返回
}

pub async fn replay(
    State(state): State<KernelState>,
    Path(run_id): Path<String>,
    Query(query): Query<ReplayQuery>,
) -> HttpResult<impl IntoResponse> {
    if query.stream {
        // SSE 流式响应
        sse_stream(&state, &run_id, query.after_seq).await
    } else {
        // JSON 数组响应（兼容旧模式）
        json_replay(&state, &run_id, query.after_seq).await
    }
}

async fn sse_stream(
    state: &KernelState,
    run_id: &str,
    after_seq: i64,
) -> HttpResult<Sse<impl Stream<Item = Result<Event, Infallible>>>> {
    let director = state.runtime.director.clone();
    let run_id = run_id.to_string();

    let stream = async_stream::stream! {
        // 1. 发送历史事件
        match director.replay(&run_id, after_seq).await {
            Ok(events) => {
                for event in events {
                    if let Ok(json) = serde_json::to_string(&event) {
                        yield Ok(Event::default().data(json));
                    }
                }
            }
            Err(_) => {
                yield Ok(Event::default().data(json!({"error": "无法加载历史事件"})));
            }
        }

        // 2. 进入长轮询模式（后续可升级为 broadcast）
        let mut last_seq = after_seq + 1;
        loop {
            tokio::time::sleep(Duration::from_millis(500)).await;
            match director.replay(&run_id, last_seq - 1).await {
                Ok(new_events) => {
                    if !new_events.is_empty() {
                        for event in new_events {
                            if let Ok(json) = serde_json::to_string(&event) {
                                yield Ok(Event::default().data(json));
                            }
                            last_seq = event.seq().unwrap_or(last_seq);
                        }
                    }
                }
                Err(_) => break,
            }
        }
    };

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

async fn json_replay(
    state: &KernelState,
    run_id: &str,
    after_seq: i64,
) -> HttpResult<impl IntoResponse> {
    let events = state.runtime.director.replay(run_id, after_seq).await?;
    Ok(Json(json!({
        "data": events,
        "ok": true,
    })))
}
```

**验证**:
- ✓ `GET /api/runs/:rid/events?afterSeq=0` 返回 JSON 数组
- ✓ `GET /api/runs/:rid/events?stream=true` 返回 text/event-stream
- ✓ SSE 流正确处理新事件订阅

### 任务 2: 实现事件流内存缓存（0.5 天）

**目标**: 减少数据库查询，缓存活跃运行的最近事件

**实现**:

```rust
use std::sync::Arc;
use dashmap::DashMap;

pub struct EventCache {
    // run_id -> Vec<(seq, GenerationEvent)>
    active_runs: DashMap<String, Vec<(i64, GenerationEvent)>>,
}

impl EventCache {
    pub fn new() -> Self {
        Self {
            active_runs: DashMap::new(),
        }
    }

    pub fn record_event(&self, run_id: String, seq: i64, event: GenerationEvent) {
        let mut entry = self.active_runs.entry(run_id).or_insert_with(Vec::new);
        entry.push((seq, event));
        
        // 只保留最近 1000 条事件
        if entry.len() > 1000 {
            entry.drain(0..100);
        }
    }

    pub fn get_events(&self, run_id: &str, after_seq: i64) -> Vec<GenerationEvent> {
        self.active_runs
            .get(run_id)
            .map(|entry| {
                entry
                    .iter()
                    .filter(|(seq, _)| *seq > after_seq)
                    .map(|(_, event)| event.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn clear_run(&self, run_id: &str) {
        self.active_runs.remove(run_id);
    }
}
```

**集成**:
- DirectorService 创建时初始化 EventCache
- replay() 优先查询缓存，缓存未命中才读数据库
- 后台 Worker 每次生成事件后调用 cache.record_event()

### 任务 3: 前端断点续传支持（0.5 天）

**目标**: 记录前端已确认的最后事件序号，支持自动恢复

**后端实现**:

```rust
#[derive(Deserialize)]
pub struct ConfirmEventInput {
    run_id: String,
    last_confirmed_seq: i64,
}

pub async fn confirm_event(
    State(state): State<KernelState>,
    Json(input): Json<ConfirmEventInput>,
) -> HttpResult<Json<serde_json::Value>> {
    // 记录前端的确认位置（可选：存到数据库用于崩溃恢复）
    // 当前阶段可以简单忽略，由前端自己维护
    Ok(Json(json!({"ok": true})))
}
```

**前端实现** (伪代码):
```javascript
let lastConfirmedSeq = 0;

async function subscribeEvents(runId) {
  const response = await fetch(
    `/api/runs/${runId}/events?stream=true&afterSeq=${lastConfirmedSeq}`
  );
  const reader = response.body.getReader();
  
  while (true) {
    const {value, done} = await reader.read();
    if (done) break;
    
    const event = JSON.parse(new TextDecoder().decode(value));
    processEvent(event);
    lastConfirmedSeq = event.seq;
    
    // 定期发送确认（可选）
    if (lastConfirmedSeq % 10 === 0) {
      fetch(`/api/runs/${runId}/confirm`, {
        method: 'POST',
        body: JSON.stringify({run_id: runId, last_confirmed_seq: lastConfirmedSeq})
      });
    }
  }
}
```

### 任务 4: 集成测试覆盖 SSE 流（1 天）

**新增测试**:

```rust
#[tokio::test]
async fn sse_stream_returns_event_stream() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();

    // 创建运行
    let run = create_test_run().await;

    // 订阅 SSE
    let response = client
        .get(&format!("{}/api/runs/{}/events?stream=true", server.url(), run.id))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    assert!(response
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .contains("text/event-stream"));
}

#[tokio::test]
async fn json_replay_still_works() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();

    let run = create_test_run().await;

    // 不带 stream 参数，应该返回 JSON
    let response = client
        .get(&format!("{}/api/runs/{}/events?afterSeq=0", server.url(), run.id))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["data"].is_array());
}
```

---

## 关键实现决策

### 为什么保留 JSON 模式兼容性？
- 前端可能还需要点对点查询（如页面刷新）
- 兼容旧版本的前端代码
- 便于性能对比和灰度升级

### 为什么不立即用 broadcast channel？
- 长轮询已足以满足 <1s 的交互延迟需求
- broadcast 需要后台维护通道，增加复杂性
- 可作为 Phase 2.3 的性能优化

### 缓存的一致性保证
- 缓存仅用于已持久化的事件（不缓存未提交的）
- 缓存命中率不影响正确性，只是性能优化
- 周期性清理（运行完成时）

---

## 验收标准

### 功能验收
- [ ] SSE 端点返回 text/event-stream 格式
- [ ] JSON 轮询端点仍正常工作（兼容）
- [ ] 历史事件和新事件都能正确推送
- [ ] 事件顺序保证（seq 单调递增）
- [ ] 缓存命中率达到 70% 以上

### 测试验收
- [ ] ✓ `cargo test --lib` 通过
- [ ] ✓ `cargo test --test integration` 通过（新增 4+ 测试）
- [ ] ✓ 编译无警告

### 性能验收
- [ ] SSE 首条事件延迟 <200ms
- [ ] 缓存查询 <10ms
- [ ] 内存占用稳定（<100MB）

---

## 时间估计

- 任务 1: SSE 改造 (1 天)
- 任务 2: 缓存实现 (0.5 天)
- 任务 3: 前端支持 (0.5 天)
- 任务 4: 集成测试 (1 天)
- **Phase 2.2 总计: 3 天**

---

## 后续工作

- **Phase 2.3**: 操作决策端点 (approve, cancel, resume)
- **Phase 2.3+**: 升级到 tokio broadcast（性能优化）


