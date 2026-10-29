# P13.9 Realistic Scale Baseline

## 状态：PARTIAL

测试：`crates/novel-application/tests/p13_scale_baseline.rs`

- 100 / 500 / 1000 章；
- 每章约 1.4–1.5k 中文字符（与 p13 旧 1000 章基线同体量）；
- 正式 Chapter Revision 与同步知识索引；
- 目标章节无正式正文，避免目标正文自泄漏；
- Context budget 24,000 / reserve 8,000；
- Search 20 次；并发 8 tasks × 10 queries；
- 单章增量更新后验证收敛词。

## 结果

| 章节 | build_ms | context p50 | context p95 | context p99 | search p50 | search p95 | search p99 | 并发 8×10 | 增量 | DB bytes | chunks |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 100 | 208 | 3.68ms | 3.76ms | 3.76ms | 231us | 425us | 425us | 17ms | 1ms | 5,304,320 | 99 |
| 500 | 1170 | 11.25ms | 11.39ms | 11.39ms | 341us | 567us | 567us | 20ms | 3ms | 25,006,080 | 499 |
| 1000 | 2223 | 19.68ms | 20.03ms | 20.03ms | 347us | 616us | 616us | 22ms | 7ms | 49,594,368 | 999 |

系统：Apple M1，8GB，macOS 15.7.7；测试进程 jobs=4；SQLite WAL。

## 说明

- 测试内的 context/search 预热后重复采样；不是冷启动；
- 未采集进程内存峰值、RSS、Swap；
- FTS rows = chunks（每章一个 chunk）；Vectors=0（本测试不写 embedding）；
- 索引构建时间包含章节创建、正式正文保存、索引同步，不只是 FTS 写入；
- 并发是同一 SQLite 连接池上的读查询，不代表多进程或网络并发；
- 未跑 Tauri GUI、真实双窗口、系统输入法。
