# Task 4.4 - 性能与监控详细设计

**日期**: 2026-10-20  
**阶段**: Phase 4.0 - 实现与集成  
**任务**: Task 4.4 - 性能与监控  
**目标**: 添加性能监控、日志追踪和性能优化

---

## 任务概述

### 目标

建立完整的可观测性系统，包括指标收集、日志追踪、性能监控和性能优化。

### 关键指标

```
响应时间 (Latency):
  • P50: < 50ms
  • P99: < 100ms
  • P99.9: < 500ms

吞吐量 (Throughput):
  • > 1000 req/s

错误率 (Error Rate):
  • < 0.1%

可用性 (Availability):
  • > 99.5%
```

---

## Task 4.4 分解

### Part 1: Prometheus 集成 (1-2 天)

#### 1.1 依赖配置

```toml
[dependencies]
prometheus = "0.13"
prometheus-static-metrics = "0.5"
actix-web-prometheus = "0.1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt", "json"] }
tracing-appender = "0.2"
```

#### 1.2 Prometheus 初始化

**文件**: `src/metrics/mod.rs`

```rust
use prometheus::{
    register_counter_vec_with_registry,
    register_histogram_vec_with_registry,
    register_gauge_vec_with_registry,
    Registry, CounterVec, HistogramVec, GaugeVec,
};

pub struct Metrics {
    pub http_requests_total: CounterVec,
    pub http_request_duration_seconds: HistogramVec,
    pub http_request_size_bytes: HistogramVec,
    pub http_response_size_bytes: HistogramVec,
    pub active_connections: GaugeVec,
    pub database_query_duration_seconds: HistogramVec,
}

impl Metrics {
    pub fn new() -> Result<(Self, Registry), Box<dyn std::error::Error>> {
        let registry = Registry::new();

        let http_requests_total = register_counter_vec_with_registry!(
            "http_requests_total",
            "Total HTTP requests",
            &["method", "path", "status"],
            registry
        )?;

        let http_request_duration_seconds = register_histogram_vec_with_registry!(
            "http_request_duration_seconds",
            "HTTP request duration",
            &["method", "path"],
            registry
        )?;

        let http_request_size_bytes = register_histogram_vec_with_registry!(
            "http_request_size_bytes",
            "HTTP request size",
            &["method", "path"],
            registry
        )?;

        let http_response_size_bytes = register_histogram_vec_with_registry!(
            "http_response_size_bytes",
            "HTTP response size",
            &["method", "path"],
            registry
        )?;

        let active_connections = register_gauge_vec_with_registry!(
            "active_connections",
            "Active connections",
            &["handler"],
            registry
        )?;

        let database_query_duration_seconds = register_histogram_vec_with_registry!(
            "database_query_duration_seconds",
            "Database query duration",
            &["query"],
            registry
        )?;

        Ok((
            Metrics {
                http_requests_total,
                http_request_duration_seconds,
                http_request_size_bytes,
                http_response_size_bytes,
                active_connections,
                database_query_duration_seconds,
            },
            registry,
        ))
    }
}
```

**预期**: ~100 行代码

#### 1.3 /metrics 端点

```rust
pub async fn metrics_handler(registry: web::Data<Registry>) -> Result<HttpResponse> {
    let encoder = TextEncoder::new();
    let metric_families = registry.gather();
    let mut buffer = vec![];
    encoder.encode(&metric_families, &mut buffer)
        .map_err(|e| actix_web::error::ErrorInternalServerError(format!("Error: {}", e)))?;

    Ok(HttpResponse::Ok()
        .content_type("text/plain; version=0.0.4")
        .body(buffer))
}
```

**预期**: ~15 行代码

---

### Part 2: 分布式追踪 (1-2 天)

#### 2.1 Tracing 配置

**文件**: `src/tracing/mod.rs`

```rust
use tracing_subscriber::{fmt, prelude::*, EnvFilter};
use tracing_appender::non_blocking;
use std::io;

pub fn init_tracing() -> Result<(), Box<dyn std::error::Error>> {
    // 创建日志文件输出
    let file_appender = tracing_appender::rolling::daily("./logs", "app.log");
    let (non_blocking, _guard) = non_blocking(file_appender);

    // 配置 tracing
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("info")))
        .with(
            fmt::layer()
                .with_writer(io::stdout)
                .with_target(true)
                .with_line_number(true)
                .with_thread_ids(true)
        )
        .with(
            fmt::layer()
                .with_writer(non_blocking)
                .json()
                .with_target(true)
        )
        .init();

    Ok(())
}
```

**预期**: ~30 行代码

#### 2.2 请求追踪

```rust
use tracing::{instrument, span, Level};

#[instrument(skip(req))]
pub async fn create_novel(
    req: web::Json<CreateNovelRequest>,
) -> Result<HttpResponse> {
    let span = span!(Level::DEBUG, "create_novel", title = ?req.title);
    let _enter = span.enter();

    tracing::info!("Creating novel: {}", req.title);

    // 业务逻辑...

    tracing::debug!("Novel created successfully");
    Ok(HttpResponse::Created().json(serde_json::json!({})))
}
```

**预期**: ~20 行代码 (每个处理器)

---

### Part 3: 性能指标中间件 (1 天)

#### 3.1 指标收集中间件

**文件**: `src/metrics/middleware.rs`

```rust
use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    Error,
};
use futures::future::LocalBoxFuture;
use std::time::Instant;
use prometheus::HistogramTimer;

pub struct MetricsMiddleware {
    metrics: web::Data<Metrics>,
}

impl<S, B> Transform<S, ServiceRequest> for MetricsMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = MetricsMiddlewareService<S>;
    type InitError = ();
    type Future = std::future::Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        std::future::ready(Ok(MetricsMiddlewareService {
            service,
            metrics: self.metrics.clone(),
        }))
    }
}

pub struct MetricsMiddlewareService<S> {
    service: S,
    metrics: web::Data<Metrics>,
}

impl<S, B> Service<ServiceRequest> for MetricsMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let method = req.method().to_string();
        let path = req.path().to_string();
        let start = Instant::now();
        
        let histogram_timer = self.metrics
            .http_request_duration_seconds
            .with_label_values(&[&method, &path])
            .start_timer();

        let fut = self.service.call(req);

        Box::pin(async move {
            let res = fut.await?;
            let status = res.status();
            
            histogram_timer.observe_duration();

            self.metrics
                .http_requests_total
                .with_label_values(&[&method, &path, status.as_str()])
                .inc();

            Ok(res)
        })
    }
}
```

**预期**: ~80 行代码

---

### Part 4: 缓存策略 (1-2 天)

#### 4.1 内存缓存

```rust
use std::sync::Arc;
use std::collections::HashMap;
use std::sync::RwLock;
use chrono::{DateTime, Utc, Duration};

#[derive(Clone)]
pub struct CacheEntry<T> {
    value: T,
    expires_at: DateTime<Utc>,
}

pub struct MemoryCache<T: Clone> {
    data: Arc<RwLock<HashMap<String, CacheEntry<T>>>>,
}

impl<T: Clone> MemoryCache<T> {
    pub fn new() -> Self {
        MemoryCache {
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn get(&self, key: &str) -> Option<T> {
        let mut data = self.data.write().unwrap();
        
        if let Some(entry) = data.get(key) {
            if entry.expires_at > Utc::now() {
                return Some(entry.value.clone());
            } else {
                data.remove(key);
            }
        }
        
        None
    }

    pub fn set(&self, key: String, value: T, ttl: Duration) {
        let entry = CacheEntry {
            value,
            expires_at: Utc::now() + ttl,
        };
        
        self.data.write().unwrap().insert(key, entry);
    }

    pub fn invalidate(&self, key: &str) {
        self.data.write().unwrap().remove(key);
    }
}
```

**预期**: ~50 行代码

#### 4.2 查询缓存应用

```rust
pub async fn get_novels(cache: web::Data<MemoryCache<Vec<Novel>>>) -> Result<HttpResponse> {
    // 先查缓存
    if let Some(cached) = cache.get("novels:all") {
        return Ok(HttpResponse::Ok().json(cached));
    }

    // 从数据库查询
    let novels = NovelRepository::list().await?;

    // 存入缓存 (1 小时过期)
    cache.set("novels:all".to_string(), novels.clone(), Duration::hours(1));

    Ok(HttpResponse::Ok().json(novels))
}
```

**预期**: ~25 行代码 (每个需要缓存的端点)

---

### Part 5: 数据库查询优化 (0.5 天)

#### 5.1 查询优化技巧

1. **使用索引**
   ```sql
   CREATE INDEX idx_novels_created_at ON novels(created_at);
   CREATE INDEX idx_chapters_novel_id ON chapters(novel_id);
   ```

2. **N+1 查询优化**
   ```rust
   // 差: N+1 查询
   let novels = list_novels().await?;
   for novel in novels {
       let chapters = list_chapters(&novel.id).await?;  // N 次查询
   }

   // 好: 一次查询
   let data = sqlx::query(
       "SELECT n.*, c.* FROM novels n 
        LEFT JOIN chapters c ON n.id = c.novel_id
        WHERE n.id IN (SELECT id FROM novels LIMIT 10)"
   ).fetch_all(&pool).await?;
   ```

3. **批量操作**
   ```rust
   // 差: 逐个插入
   for novel in novels {
       insert_novel(&novel).await?;
   }

   // 好: 批量插入
   sqlx::query(
       "INSERT INTO novels (id, title, created_at)
        VALUES (?, ?, ?), (?, ?, ?), ..."
   ).execute(&pool).await?;
   ```

**预期**: 文档 + 优化建议

---

### Part 6: 性能基准测试 (1-2 天)

#### 6.1 基准测试框架

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_get_novel(c: &mut Criterion) {
    c.bench_function("get_novel", |b| {
        b.iter(|| {
            let repo = NovelRepository::new();
            let id = black_box("novel-1");
            repo.get_by_id(id)
        });
    });
}

criterion_group!(benches, benchmark_get_novel);
criterion_main!(benches);
```

#### 6.2 负载测试

使用 Apache Bench 或 wrk:

```bash
# 基本测试
ab -n 1000 -c 10 http://localhost:8080/novels

# 吞吐量测试
wrk -t4 -c100 -d30s http://localhost:8080/novels

# 延迟分布测试
wrk -t4 -c100 -d30s --latency http://localhost:8080/novels
```

**预期**: 性能基准报告

---

### Part 7: 监控仪表板配置 (0.5 天)

#### 7.1 Prometheus 配置

**文件**: `prometheus.yml`

```yaml
global:
  scrape_interval: 15s
  evaluation_interval: 15s

scrape_configs:
  - job_name: 'novel-server'
    static_configs:
      - targets: ['localhost:8080']
    metrics_path: '/metrics'
```

#### 7.2 Grafana 仪表板

关键面板:
- HTTP 请求速率
- 响应时间 (P50, P99, P99.9)
- 错误率和错误类型
- 活跃连接数
- 数据库查询性能
- 缓存命中率

**预期**: Grafana 配置 JSON

---

### Part 8: 测试 (1 天)

```rust
#[test]
fn test_memory_cache_set_get() {
    let cache: MemoryCache<String> = MemoryCache::new();
    cache.set("key".to_string(), "value".to_string(), Duration::hours(1));
    assert_eq!(cache.get("key"), Some("value".to_string()));
}

#[test]
fn test_memory_cache_expiry() {
    let cache: MemoryCache<String> = MemoryCache::new();
    cache.set("key".to_string(), "value".to_string(), Duration::seconds(0));
    assert_eq!(cache.get("key"), None);
}

#[actix_web::test]
async fn test_metrics_endpoint() {
    let app = test::init_service(App::new().route("/metrics", web::get().to(metrics_handler))).await;
    let req = test::TestRequest::get().uri("/metrics").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
}
```

**预期**: 10+ 性能测试

---

## 完整时间表

```
Day 1-2: Prometheus 集成
  • 依赖配置
  • 指标定义
  • /metrics 端点

Day 3-4: 分布式追踪
  • Tracing 初始化
  • 请求追踪
  • 日志配置

Day 5: 性能指标中间件
  • 指标收集
  • 性能数据聚合

Day 6-7: 缓存和优化
  • 内存缓存实现
  • 查询优化
  • 基准测试

Day 8: 仪表板和测试
  • Prometheus 配置
  • Grafana 配置
  • 测试编写
```

**总计**: 6-8 工作天

---

## 成功标准

- [ ] Prometheus 集成完整
- [ ] 分布式追踪工作
- [ ] 缓存策略实现
- [ ] 性能基准建立
- [ ] 仪表板配置完成
- [ ] 10+ 性能测试通过

---

**预计开始**: 2026-11-19  
**预计完成**: 2026-11-26
