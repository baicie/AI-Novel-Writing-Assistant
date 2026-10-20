# Task 4.2 - HTTP 与数据库集成测试

**日期**: 2026-10-20  
**阶段**: Phase 4.0 - 实现与集成  
**任务**: Task 4.2 - HTTP 与数据库集成测试  
**预期完成**: 2026-10-27  

## 任务目标

Task 4.1 只验证了 HTTP 端点的响应格式合法性。**Task 4.2 的目标是验证 HTTP 端点真正连接到数据库**，确保数据的完整 CRUD 持久化链路工作正常。

## 现状

### ✅ 已完成的部分
- SQLite 存储层 (`novel-storage-sqlite` crate, ~3250 行)
- 数据库 schema 和迁移脚本 (3 个文件)
- Repository trait 和实现 (NovelRepository, ChapterRepository 等)
- HTTP 路由完全连接到 SqliteKernel
- 4 个 invariant 测试（存储层低级验证）

### ❌ 缺失的部分
- **集成测试**：HTTP 层 + 数据库往返的真实验证
- **事务与并发测试**：多用户、并发安全
- **端到端工作流测试**：小说创建 → 章节 → 运行的完整链路
- **数据持久化验证**：不仅检查响应，还要验证数据真正保存

## Task 4.2 分解

### Part 1: 集成测试框架 (1 天)

建立测试基础设施：

```rust
// tests/integration_setup.rs
pub async fn setup_test_db() -> SqliteKernel {
    // 创建临时数据库
    // 运行所有迁移
    // 返回可用的内核
}

pub async fn with_kernel<F>(test: F)
where
    F: Fn(Arc<SqliteKernel>) -> Pin<Box<dyn Future>>,
{
    // 为每个测试准备独立的数据库
}
```

**预期**: ~100 行辅助代码

### Part 2: 小说 CRUD 集成测试 (2 天)

验证小说的完整持久化链路：

```rust
#[tokio::test]
async fn test_create_novel_persists_to_db() {
    // 1. 通过 HTTP 创建小说
    // 2. 查询数据库验证数据存在
    // 3. 通过 HTTP 读取验证内容
}

#[tokio::test]
async fn test_update_novel_persists() {
    // 创建 → 更新 → 验证数据库
}

#[tokio::test]
async fn test_delete_novel_removes_from_db() {
    // 创建 → 删除 → 验证数据库中不存在
}

#[tokio::test]
async fn test_list_novels_returns_persisted() {
    // 创建多个小说 → 列表返回全部 → 验证顺序
}

#[tokio::test]
async fn test_novel_concurrent_creation() {
    // 并发创建 10 个小说
    // 验证全部都被保存且互不冲突
}
```

**预期**: 8-10 个测试，~400 行代码

### Part 3: 章节工作流集成测试 (2 天)

验证章节的创建、保存、晋升流程：

```rust
#[tokio::test]
async fn test_create_chapter_persists() {
    // 创建小说 → 创建章节 → 验证数据库
}

#[tokio::test]
async fn test_save_chapter_content_persists() {
    // 创建章节 → 保存内容 → 验证版本管理
}

#[tokio::test]
async fn test_promote_candidate_to_official() {
    // 创建候选内容 → 晋升为正式版 → 验证版本号递增
}

#[tokio::test]
async fn test_chapter_protection_state_changes() {
    // 测试章节保护状态的转换
}

#[tokio::test]
async fn test_chapter_version_conflict_detection() {
    // 并发编辑同一章节 → 验证冲突检测
}
```

**预期**: 8-10 个测试，~500 行代码

### Part 4: 运行 (Run) 生命周期测试 (2 天)

验证自动导演运行的完整流程：

```rust
#[tokio::test]
async fn test_open_run_persists_to_db() {
    // 创建运行 → 验证数据库记录
}

#[tokio::test]
async fn test_run_event_sequence_persists() {
    // 运行事件序列完整保存
}

#[tokio::test]
async fn test_approve_run_updates_status() {
    // 运行状态转换正确持久化
}

#[tokio::test]
async fn test_cancel_run_persists() {
    // 取消运行 → 验证状态和错误信息
}

#[tokio::test]
async fn test_run_recovery_point_persists() {
    // 恢复点信息完整保存
}
```

**预期**: 8-10 个测试，~450 行代码

### Part 5: 事务与约束测试 (1 天)

验证数据库级别的安全性：

```rust
#[tokio::test]
async fn test_foreign_key_constraint() {
    // 尝试删除有章节的小说 → 验证级联删除
}

#[tokio::test]
async fn test_concurrent_version_increment() {
    // 并发保存同一章节 → 验证版本一致性
}

#[tokio::test]
async fn test_transaction_rollback_on_error() {
    // 事务中途失败 → 验证数据一致性
}

#[tokio::test]
async fn test_unique_constraint_violation() {
    // 重复数据插入 → 验证约束生效
}
```

**预期**: 6-8 个测试，~300 行代码

### Part 6: 性能基准测试 (1 天)

验证性能指标：

```rust
#[tokio::test]
async fn bench_create_novel() {
    // 测试 100 次创建的平均响应时间
    // 期望 < 50ms
}

#[tokio::test]
async fn bench_list_novels_pagination() {
    // 创建 1000 个小说 → 分页查询性能
}

#[tokio::test]
async fn bench_concurrent_access() {
    // 100 个并发请求 → 验证吞吐量
}
```

**预期**: 4-6 个测试，~250 行代码

### Part 7: 文档 (1 天)

```markdown
docs/wiki/architecture/http-database-integration.md
- HTTP 层与存储层的连接
- 事务模式和并发模型
- 性能基准和优化建议

crates/novel-http/tests/README.md
- 集成测试运行指南
- 测试数据库初始化流程
- 调试失败的集成测试
```

**预期**: ~500 行文档

## 测试总计

- **集成测试**: 40-50 个新测试
- **代码总量**: ~2500 行测试代码
- **覆盖路径**: 所有 15+ HTTP 端点的数据库持久化
- **预期通过率**: 100%

## 工作流程

```
Day 1:  集成测试框架 + 小说 CRUD 开始
Day 2:  小说 CRUD 完成 + 章节工作流开始
Day 3:  章节工作流完成 + 运行生命周期开始
Day 4:  运行生命周期完成 + 事务测试
Day 5:  性能基准 + 文档
Day 6:  集成验证 + 修复失败案例
Day 7:  最终优化 + 提交
```

**总计**: 6-7 工作天

## 成功标准

- [ ] 40+ 集成测试全部通过
- [ ] 数据库约束全部验证通过
- [ ] 并发测试无数据竞争
- [ ] 所有 HTTP 端点都有端到端测试
- [ ] 性能基准符合预期
- [ ] 文档完整清晰

## 与 Phase 4.0 的关系

Task 4.2 完成后，Phase 4.0 的状态：
- ✅ Task 4.1: HTTP 端点合法性 (完成)
- ✅ Task 4.2: HTTP-DB 集成验证 (将完成)
- ⏳ Task 4.3: 用户认证与权限 (待开始)
- ⏳ Task 4.4: 错误处理与恢复 (待开始)
- ⏳ Task 4.5: 性能优化与文档 (待开始)
