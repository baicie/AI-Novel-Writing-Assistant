# Task 4.2 - HTTP 与数据库集成测试 实现清单

**状态**: 🚀 启动  
**分支**: `feature/rust-tasks-3.0`  
**创建时间**: 2026-10-20  

## Part 1: 集成测试框架

- [ ] 创建 `crates/novel-http/tests/integration_setup.rs`
  - [ ] `setup_test_db()` 函数实现
  - [ ] 临时数据库文件管理
  - [ ] 迁移自动运行
  - [ ] 连接池配置

- [ ] 创建 `crates/novel-http/tests/common/mod.rs`
  - [ ] `TestContext` 结构体
  - [ ] `with_test_kernel` 宏
  - [ ] 测试数据生成辅助函数

- [ ] 验证框架可用
  - [ ] 编译通过
  - [ ] 测试数据库可初始化

**预期完成**: Day 1

---

## Part 2: 小说 CRUD 集成测试

- [ ] 创建 `crates/novel-http/tests/integration_novels.rs`

#### 创建测试
- [ ] `test_create_novel_persists_to_db`
  - [ ] HTTP POST /api/novels
  - [ ] 验证 HTTP 响应 200
  - [ ] 直接查询数据库验证数据存在
  - [ ] 验证所有字段正确

- [ ] `test_create_novel_empty_fields`
  - [ ] 创建最小化小说（仅标题）
  - [ ] 验证默认值正确

#### 读取测试
- [ ] `test_get_novel_returns_persisted_data`
  - [ ] 创建 → GET → 验证数据完整性

- [ ] `test_list_novels_returns_persisted`
  - [ ] 创建 3 个小说
  - [ ] 列表返回全部
  - [ ] 验证排序（updated_at DESC）

#### 更新测试
- [ ] `test_update_novel_persists`
  - [ ] 创建 → 更新标题 → 验证数据库
  - [ ] 验证 updated_at 变化

- [ ] `test_update_nonexistent_novel`
  - [ ] 更新不存在的小说 → 404

#### 删除测试
- [ ] `test_delete_novel_removes_from_db`
  - [ ] 创建 → 删除 → 验证数据库不存在

- [ ] `test_delete_nonexistent_novel`
  - [ ] 删除不存在的小说 → 404

#### 并发测试
- [ ] `test_novel_concurrent_creation`
  - [ ] 10 个并发请求创建小说
  - [ ] 验证全部保存且无重复
  - [ ] 验证每个小说都有唯一 ID

- [ ] `test_concurrent_update_same_novel`
  - [ ] 并发修改同一小说的不同字段
  - [ ] 验证最终结果一致

**预期完成**: Day 2  
**预期测试数**: 10-12 个  
**预期代码**: 500-600 行

---

## Part 3: 章节工作流集成测试

- [ ] 创建 `crates/novel-http/tests/integration_chapters.rs`

#### 创建测试
- [ ] `test_create_chapter_persists`
  - [ ] 创建小说 → POST /api/novels/:id/chapters
  - [ ] 验证 HTTP 响应
  - [ ] 验证数据库中章节存在
  - [ ] 验证 sort_order 正确

- [ ] `test_create_multiple_chapters_preserve_order`
  - [ ] 创建 5 个章节（不同 sort_order）
  - [ ] 列表返回按 sort_order 排序

#### 内容保存测试
- [ ] `test_save_chapter_content_persists`
  - [ ] 创建章节 → PUT 保存内容
  - [ ] 验证 chapter_revision 表中插入新记录
  - [ ] 验证版本号从 0 → 1

- [ ] `test_save_chapter_version_increment`
  - [ ] 保存 → 修改 → 保存
  - [ ] 验证版本号递增（1 → 2 → 3）

#### 晋升测试
- [ ] `test_promote_candidate_to_official`
  - [ ] 创建候选正文
  - [ ] 晋升为正式版
  - [ ] 验证 official_revision_id 更新
  - [ ] 验证版本号递增

- [ ] `test_promote_blocked_by_protection`
  - [ ] 章节被用户编辑保护
  - [ ] 尝试晋升 → 拒绝

#### 状态转换测试
- [ ] `test_chapter_protection_state_changes`
  - [ ] 验证状态转换规则
  - [ ] none → user_edited → others

#### 并发编辑冲突
- [ ] `test_chapter_version_conflict_detection`
  - [ ] 并发修改同一章节
  - [ ] 第二个请求检测版本冲突
  - [ ] 返回 409 Conflict

**预期完成**: Day 3  
**预期测试数**: 10-12 个  
**预期代码**: 600-700 行

---

## Part 4: 运行 (Run) 生命周期集成测试

- [ ] 创建 `crates/novel-http/tests/integration_runs.rs`

#### 创建运行
- [ ] `test_open_run_persists_to_db`
  - [ ] POST /api/novels/:id/runs
  - [ ] 验证运行记录在数据库中
  - [ ] 验证状态、类型、时间戳

#### 运行事件
- [ ] `test_run_event_sequence_persists`
  - [ ] 创建运行 → 检查事件列表
  - [ ] 事件按 seq 排序
  - [ ] 所有事件类型正确

#### 状态转换
- [ ] `test_approve_run_updates_status`
  - [ ] 创建运行 → approve
  - [ ] 验证数据库状态变化

- [ ] `test_cancel_run_persists`
  - [ ] 创建运行 → cancel
  - [ ] 验证 error 消息保存

- [ ] `test_resume_run_recovers_state`
  - [ ] 创建运行 → 暂停 → 恢复
  - [ ] 验证恢复点正确

#### 事件确认
- [ ] `test_confirm_event_persists`
  - [ ] 添加事件确认
  - [ ] 验证在数据库中持久化

#### 并发运行
- [ ] `test_concurrent_runs_same_novel`
  - [ ] 同一小说同时运行多个任务
  - [ ] 验证互不干扰

**预期完成**: Day 4  
**预期测试数**: 10-12 个  
**预期代码**: 550-650 行

---

## Part 5: 事务与约束测试

- [ ] 创建 `crates/novel-http/tests/integration_constraints.rs`

#### 外键约束
- [ ] `test_cascade_delete_novel_removes_chapters`
  - [ ] 创建小说 + 章节
  - [ ] 删除小说
  - [ ] 验证章节级联删除

- [ ] `test_foreign_key_prevents_orphan_chapter`
  - [ ] 尝试创建孤立章节（无效 novel_id）
  - [ ] 验证约束失败

#### 版本冲突
- [ ] `test_concurrent_version_increment`
  - [ ] 并发保存同一章节 10 次
  - [ ] 验证版本号 1-10 不重复

- [ ] `test_unique_constraint_on_chapter_order`
  - [ ] 尝试在同一小说中创建重复 sort_order
  - [ ] 验证约束生效

#### 事务回滚
- [ ] `test_save_chapter_transaction_safety`
  - [ ] 创建章节 + 修订 + 工件
  - [ ] 模拟中途错误
  - [ ] 验证三个表都回滚

#### 并发安全
- [ ] `test_concurrent_delete_and_read`
  - [ ] 并发删除 + 读取同一资源
  - [ ] 验证无数据竞争

**预期完成**: Day 4-5  
**预期测试数**: 8-10 个  
**预期代码**: 400-500 行

---

## Part 6: 性能基准测试

- [ ] 创建 `crates/novel-http/tests/integration_benchmarks.rs`

#### 创建性能
- [ ] `bench_create_novel_latency`
  - [ ] 100 次创建
  - [ ] 平均响应时间 < 50ms
  - [ ] P95 < 100ms

#### 查询性能
- [ ] `bench_list_novels_pagination`
  - [ ] 1000 个小说
  - [ ] 各分页大小查询性能
  - [ ] 第 1 页 < 20ms
  - [ ] 第 50 页 < 30ms

#### 并发性能
- [ ] `bench_concurrent_requests`
  - [ ] 100 个并发请求
  - [ ] 平均响应时间 < 100ms
  - [ ] 无超时

#### 更新性能
- [ ] `bench_update_chapter_performance`
  - [ ] 修改同一章节 50 次
  - [ ] 每次 < 40ms

**预期完成**: Day 5  
**预期测试数**: 4-6 个  
**预期代码**: 300-400 行

---

## Part 7: 文档

- [ ] 创建 `docs/wiki/architecture/http-database-integration.md`
  - [ ] HTTP 层与 SqliteKernel 连接图
  - [ ] 数据流：HTTP 请求 → Repository → 数据库
  - [ ] 事务处理模式
  - [ ] 并发锁定机制
  - [ ] 性能优化建议

- [ ] 创建 `crates/novel-http/tests/README.md`
  - [ ] 集成测试运行指南
  - [ ] `cargo test --test integration_*`
  - [ ] 测试数据库清理
  - [ ] 调试单个测试
  - [ ] 常见问题排查

- [ ] 更新 `crates/novel-http/README.md`
  - [ ] 链接到集成测试文档

**预期完成**: Day 6  
**预期代码**: 800-1000 行

---

## 构建和验证

- [ ] Day 6 - 集成验证
  - [ ] `cargo test --test integration_*` 全部通过
  - [ ] 检查 clippy 警告
  - [ ] 检查覆盖率（目标 > 90%）

- [ ] Day 7 - 最终优化
  - [ ] 性能优化（如果有热点）
  - [ ] 代码清理
  - [ ] 文档校对

---

## 提交计划

**中间提交**:
```
1. feat: 集成测试框架 + 小说 CRUD 测试 (12 个测试通过)
2. feat: 章节工作流集成测试 (12 个测试通过)
3. feat: 运行生命周期集成测试 (12 个测试通过)
4. feat: 事务与约束测试 (10 个测试通过)
5. feat: 性能基准测试 (6 个测试通过)
6. docs: HTTP-DB 集成文档和使用指南
7. test: Task 4.2 完成 - 50+ 集成测试全部通过 ✅
```

**最终状态**:
- 分支推送到远程
- 工作区干净
- 准备进入 Task 4.3（用户认证）

---

## 检查清单

- [ ] 所有 50+ 测试通过
- [ ] 0 编译错误和 clippy 警告
- [ ] 没有 panic 或 unwrap
- [ ] 数据库 schema 版本一致
- [ ] 文档完整清晰
- [ ] 提交信息遵循格式

---

**上次更新**: 2026-10-20 11:35 UTC+8  
**状态**: 🟢 Ready to start Part 1
