# P13.4 Long-Form Performance

## 状态：BASELINE IMPLEMENTED

`crates/novel-application/tests/p13_performance.rs`：

```text
P13 performance:
chapters=1000
build_ms=2695
context_p50_us=18439
context_p95_us=24364
incremental_update_ms=6
index_200_ms=204
search_p50_us=166
search_p95_us=289
db_mb=1.74
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


Native 回归本轮执行：

```text
Director 4 passing
Editor 2 passing
Candidate Journal SIGKILL crash + restart 1 passing
```

仍未覆盖：

- 500/1000 章增量索引收敛
- 并发检索
- 真实 Embedding 模型
- Native UI 端到端性能

最终构建：`.app` 20.54 MiB；DMG 9.2 MiB；Tauri release audit PASS。


最终构建（第 4 轮）：`.app` 20.72 MiB；DMG 9.3 MiB；Tauri release audit PASS。


1000 章 build_ms 已包含章节正式修订同步写入知识索引的耗时。


## 增量索引收敛

修改第 500 章正式正文后：

- `incremental_update_ms=6`
- 新正文立即进入 `chapter:ch500:0`
- 旧正文 revision 不再命中
- 与章节保存同一事务完成


第 6 轮构建：`.app` 20.72 MiB；DMG 9.3 MiB；audit PASS。


第 8 轮最终构建：`.app` 20.74 MiB；DMG 9.3 MiB；audit PASS。
