# 后端 Rust 化 Phase 2.2 设计 - 工作流通道与事件流

**状态**: 设计中  
**日期**: 2026-10-19  
**工作量**: 1.5 周  
**优先级**: 最高  
**前置条件**: Phase 2.1 完成

## 背景

Phase 2.1 完成了基础存储与查询（list_runs、get_chapter_full）。现在 Rust 内核能列出运行，但仍缺少：

1. **打开运行的完整链路** - 当前框架已有，但缺少错误处理和验证
2. **事件流端点** - 前端需要订阅运行的实时事件
3. **历史事件回放** - 打开旧运行后能获取完整事件序列
4. **断点续传** - 支持从指定事件序号继续拉取（用于前端刷新恢复）

## 关键业务流程回顾

### 完整的打开运行流程

```
前端 (运行记录页)
  ↓
POST /api/novels/:id/runs {
  command_id: "cmd123",
  target_chapter_id: "ch2",
  provider: "openai",
  model: "gpt-4",
  prompt_versions: { chapter_draft: "v1.2" },
  token_budget: 8000,
  max_automatic_retries: 3,
  endpoint: "https://api.openai.com",
  allow_localhost_http: false
}
  ↓
DirectorService::open_run (验证 + 授权)
  - 章节存在性
  - 提示词版本有效性
  - 端点授权（防止内外网混杂）
  - 运行冲突检测
  ↓
WorkflowRepository::open_run (持久化)
  - 创建 workflow_run 记录
  - 创建 workflow_control 记录（状态=queued）
  - 插入 run_started 事件
  - 记录 workflow_command（请求去重）
  ↓
Worker 后台循环
  - claim_next 领取这个运行
  - 根据合同生成初稿
  - 产生一系列事件（step_started, token_delta, checkpoint_saved, ...）
```

### 事件流生命周期

```
运行启动
  ↓
run_started (seq=1, occurred_at=now)
  ↓
step_started(draft) (seq=2)
  ↓
token_delta* (多条，不计 seq)
  ↓
checkpoint_saved (seq=3, candidate_id=c1)
  ↓
needs_approval (seq=4)
  ↓
[用户在前端点击"同意"]
  ↓
step_succeeded (seq=5)
  ↓
run_completed (seq=6)
```

每一条事件都记录在 workflow_event 表中，seq 是同一 run_id 内的单调递增主键。

### 前端订阅事件流的两种模式

**模式 1：历史回放（打开旧运行）**
```
GET /api/runs/:rid/events?afterSeq=0
响应: JSON 数组
[
  { kind: "run_started", runId: "r1", seq: 1, ... },
  { kind: "step_started", runId: "r1", seq: 2, ... },
  ...
]
```

**模式 2：实时订阅（正在进行的运行）**
```
GET /api/runs/:rid/events (SSE)
响应: text/event-stream

data: {"kind":"step_started","runId":"r1","seq":2,...}\n\n
data: {"kind":"token_delta","runId":"r1","stepId":"draft","text":"她"}\n\n
data: {"kind":"token_delta","runId":"r1","stepId":"draft","text":"走"}\n\n
...
```

## 关键设计决策

### 1. 事件表的索引策略

```sql
-- workflow_event 表结构（已在 0001_init.sql 中）
CREATE TABLE workflow_event (
  run_id TEXT NOT NULL REFERENCES workflow_run(id),
  seq INTEGER NOT NULL,
  kind TEXT NOT NULL,
  step_id TEXT,
  payload_json TEXT NOT NULL,
  occurred_at INTEGER NOT NULL,
  PRIMARY KEY (run_id, seq)
);
```

**查询优化**:
- events_after 查询使用 `run_id, seq` 的主键范围扫描，O(n) 其中 n 为返回事件数
- 无需额外索引，主键已支持等值+范围查询

### 2. SSE vs 长轮询

**初期实现：长轮询** (Phase 2.2)
- 前端轮询 `/api/runs/:rid/events?afterSeq=last_seq`，间隔 500ms
- 优点：简单，无后台维护
- 缺点：延迟 <1s，成本较高
- 适用：初期功能验证，用户数少

**后期优化：tokio broadcast** (Phase 2.3+)
- Worker 每产生一条事件就广播到所有订阅者
- 前端通过 SSE 接收实时事件
- 优点：零延迟，成本低
- 需要：后台 EventHub 维护（已有基础）

### 3. 运行冲突检测

```sql
-- 表约束确保同一小说同时只有一个活跃运行
CREATE UNIQUE INDEX one_active_run_per_novel
ON workflow_control(novel_id)
WHERE status IN ('queued', 'running', 'waiting_approval', 'paused');
```

**错误处理**:
```rust
if active > 0 {
    return Err(PortError::Conflict(
        "这本书已经有一个进行中的创作任务，请先完成或取消它。"
    ));
}
```

### 4. 端点授权策略

```rust
pub fn authorize_endpoint(endpoint: &str, allow_localhost: bool) -> Result<(), String> {
    let url = Url::parse(endpoint)?;
    let host = url.host_str().ok_or("端点没有主机名")?;
    
    // 本地模型端点只能在开发模式启用
    if (host == "localhost" || host == "127.0.0.1") && !allow_localhost {
        return Err("生产环境不允许本地端点".into());
    }
    
    Ok(())
}
```

这防止了误配置（如误用本地模型端点导致生产中调用失败）。

## 关键数据结构

### StoredEvent（数据库层）

```rust
#[derive(Debug, Clone)]
pub struct StoredEvent {
    pub run_id: String,
    pub seq: i64,
    pub kind: String,
    pub step_id: Option<String>,
    pub payload_json: String,
    pub occurred_at: i64,
}
```

用途：从 workflow_event 表直接读取的原始行。

### GenerationEvent（应用层）

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum GenerationEvent {
    #[serde(rename = "run_started")]
    RunStarted { run_id: String, seq: i64, occurred_at: i64 },
    #[serde(rename = "step_started")]
    StepStarted { run_id: String, seq: i64, step_id: String, occurred_at: i64 },
    #[serde(rename = "token_delta")]
    TokenDelta { run_id: String, step_id: String, text: String },
    // ... 其他事件
}
```

用途：经过反序列化和转换的业务事件，供 HTTP 层序列化为 JSON。

## 实现路线图

### Week 1

**Day 1: 完善 open_run 验证链路**
- DirectorService::open_run 补充章节验证
- 提示词版本和端点授权检查
- 集成测试覆盖错误场景

**Day 2-3: 实现 events_after 和 replay**
- WorkflowRepository::events_after 查询实现
- DirectorService::replay 方法
- 集成测试验证事件序列

**Day 4-5: 实现 SSE 路由**
- runs.rs 中的 stream_events 实现
- 支持历史回放 + 长轮询
- 集成测试覆盖 SSE 订阅

### Week 2

**Day 1: 集成测试完善**
- 覆盖运行冲突场景
- 事件序列一致性验证
- 端到端测试（打开 → 查询 → 订阅）

**Day 2: 性能和文档**
- 基准测试（事件查询延迟）
- 更新 wiki（事件流生命周期、SSE 策略）

**Day 3: 合并到 beta**
- 通过所有验收标准
- 准备合并审查

## 验收标准

### 功能标准

- [ ] `POST /api/novels/:id/runs` 完整验证（章节、提示词、端点、冲突）
- [ ] `GET /api/runs/:rid/events?afterSeq=N` 返回 JSON 数组，支持历史回放
- [ ] `GET /api/runs/:rid/events` SSE 端点，支持实时事件流
- [ ] 事件序列号单调递增，无遗漏或重复
- [ ] 打开运行后立即生成 run_started 事件

### 测试标准

- [ ] 编译通过：`cargo check` ✓
- [ ] 单元测试：`cargo test --lib` ✓
- [ ] 集成测试：`cargo test --test integration` ✓（新增 5+ 测试）
  - open_run 成功场景
  - open_run 失败场景（冲突、无效章节、无效提示词）
  - events_after 分页查询
  - SSE 事件流订阅

### 性能标准

- [ ] 单个运行的事件查询 <100ms（1000 条事件以内）
- [ ] SSE 首条事件延迟 <200ms
- [ ] 长轮询模式下内存占用稳定

## 风险评估

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| SSE 客户端兼容性 | 低 | 中 | 降级到长轮询 |
| 事件序号冲突 | 极低 | 高 | 使用主键约束 + 事务 |
| 内存泄漏（长连接） | 中 | 中 | 定期关闭 + 内存监控 |
| 数据库并发瓶颈 | 低 | 中 | 索引优化 + 连接池 |

## 相关模块依赖

- `novel-application`: DirectorService、ports、dto
- `novel-storage-sqlite`: WorkflowRepository 实现
- `novel-http`: 路由注册、SSE 响应
- `novel-workflow`: GenerationEvent 映射
- `novel-llm`: 端点授权逻辑

## 后续优化方向

1. **tokio broadcast 升级**: 升级到真正的实时推送（Phase 2.3+）
2. **事件消费者追踪**: 记录前端的最后确认事件，用于恢复指导
3. **事件聚合**: 对频繁的 token_delta 进行分批（减少数据库写入）
4. **事件压缩**: 存档完成的运行的事件到单一 blob（节省存储）

---

**下一步**: 按照 `PHASE_2_2_IMPLEMENTATION_GUIDE.md` 的任务清单依次实现。

