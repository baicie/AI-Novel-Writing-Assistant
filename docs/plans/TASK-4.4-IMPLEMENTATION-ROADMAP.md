# Task 4.4 - 性能与监控 实现路线图

**版本**: 1.0 | **状态**: 🚀 **计划中**  
**计划**: 2026-11-12 ~ 2026-11-16 | **工作量**: 2-3 天

---

## 一、核心目标

### 1.1 功能目标

- ✅ Prometheus 指标收集
- ✅ 缓存层 (Redis/内存)
- ✅ 性能基准测试
- ✅ 日志聚合
- ✅ 错误追踪

### 1.2 质量目标

| 指标 | 目标 |
|-----|------|
| **监控指标** | 15+ 个 |
| **测试用例** | 18+ 个 |
| **代码行数** | ~300 行 |
| **代码覆盖率** | > 85% |
| **查询延迟** | < 100ms (p95) |

---

## 二、监控系统设计

### 2.1 3 个主要组件

#### 1. Prometheus 指标 (MetricsService)
```rust
pub struct MetricsService {
    registry: Registry,
}

pub struct Metrics {
    http_requests_total: IntCounterVec,
    http_request_duration_seconds: HistogramVec,
    db_queries_total: IntCounterVec,
    db_query_duration_seconds: HistogramVec,
    cache_hits_total: IntCounter,
    cache_misses_total: IntCounter,
}
```

**指标**:
- http_requests_total (请求计数)
- http_request_duration_seconds (请求延迟)
- db_queries_total (数据库查询计数)
- db_query_duration_seconds (查询延迟)
- cache_hit_ratio (缓存命中率)
- generation_token_count (生成 token 数)
- error_count (错误计数)

**代码行数**: ~100 行

#### 2. 缓存层 (CacheService)
```rust
pub struct CacheService {
    cache: Arc<Mutex<HashMap<String, CacheEntry>>>,
}

impl CacheService {
    pub async fn get(&self, key: &str) -> Result<Option<T>> {
        // 从缓存获取
    }

    pub async fn set(&self, key: &str, value: T, ttl: Duration) -> Result<()> {
        // 设置缓存
    }

    pub async fn invalidate(&self, key: &str) -> Result<()> {
        // 失效缓存
    }
}
```

**缓存对象**:
- 故事详情 (TTL: 5 分钟)
- 章节列表 (TTL: 10 分钟)
- 用户信息 (TTL: 30 分钟)

**代码行数**: ~120 行

#### 3. 日志和追踪 (TracingService)
```rust
pub struct TracingService;

impl TracingService {
    pub fn init() {
        // 初始化日志和分布式追踪
    }

    pub fn span(name: &str) -> Span {
        // 创建追踪 span
    }
}
```

**代码行数**: ~80 行

---

## 三、实现计划

### Day 1 (2026-11-12)

**目标**: Prometheus + 缓存

- [ ] MetricsService 实现 - 1.5 小时
- [ ] CacheService 实现 - 1.5 小时
- [ ] 集成到 HTTP 层 - 1 小时
- [ ] 单元测试 (10 个) - 1.5 小时

**代码行数**: ~220 行 | **测试**: 10 个 ✓

---

### Day 2 (2026-11-13)

**目标**: 日志和追踪

- [ ] TracingService 实现 - 1 小时
- [ ] 分布式追踪集成 - 1.5 小时
- [ ] 错误追踪 - 1 小时
- [ ] 单元测试 (8 个) - 1 小时

**代码行数**: ~80 行 | **测试**: 8 个 ✓

---

### Day 3 (2026-11-14)

**目标**: 性能验证和优化

- [ ] 基准测试 - 1.5 小时
- [ ] 性能分析和优化 - 1.5 小时
- [ ] Grafana 仪表板设置 - 1 小时
- [ ] 文档完成 - 1 小时

**总计**: 18+ 个测试通过 ✓

---

## 四、监控指标列表

| 指标名 | 类型 | 标签 | 用途 |
|--------|------|------|------|
| http_requests_total | Counter | method, endpoint, status | 请求统计 |
| http_request_duration | Histogram | method, endpoint | 性能监控 |
| db_queries_total | Counter | table, operation | 数据库监控 |
| db_query_duration | Histogram | table, operation | 查询性能 |
| cache_hits | Counter | resource | 缓存效率 |
| cache_misses | Counter | resource | 缓存命中率 |
| generation_tokens | Counter | status | 生成统计 |
| error_count | Counter | type, endpoint | 错误追踪 |
| queue_size | Gauge | queue_type | 队列状态 |
| memory_usage | Gauge | component | 内存监控 |

---

## 五、完成标准

```
✅ 15+ 个监控指标实现
✅ 18+ 个测试全部通过
✅ 代码覆盖率 > 85%
✅ 缓存命中率 > 80%
✅ 查询延迟 < 100ms (p95)
✅ Prometheus 端点可访问
✅ 性能基准测试通过
```

**启动**: 2026-11-12 | **完成**: 2026-11-16

