# P13.4 Long-Form Performance

## 状态：BASELINE IMPLEMENTED

`crates/novel-application/tests/p13_performance.rs`：

```text
P13 performance:
chapters=1000
build_ms=672
context_p50_us=12964
context_p95_us=13726
index_200_ms=94
search_p50_us=178
search_p95_us=294
db_mb=0.91
```

条件：本机、单进程 SQLite、Mock/合成正文、无并发负载。

已具备测试规模：

- 100 章 Context Continuity
- 100 章 Mock 连续生成

尚未采集：

- 500/1000 章索引构建时间
- 增量更新耗时
- 检索 P50/P95
- Context Assembly P50/P95
- 数据库/内存规模
- 索引收敛时间

本轮不新增缓存架构。


仍未覆盖：

- 500/1000 章增量索引收敛
- 并发检索
- 真实 Embedding 模型
- Native UI 端到端性能