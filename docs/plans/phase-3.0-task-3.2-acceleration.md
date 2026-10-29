# Task 3.2 实现加速计划

> 任务：完成数据库持久化层（SQLite + SQLx）  
> 当前状态：框架完成，需要补全实现  
> 目标：本周完成（2026-10-23）

## 当前代码统计

```
✅ 已有实现：
  ├─ novels.rs:     754 行 (NovelRepository)
  ├─ workflow.rs:   970 行 (WorkflowRepository)
  ├─ tasks.rs:      152 行 (TaskCenterRepository)
  ├─ worlds.rs:     143 行 (WorldRepository)
  ├─ knowledge.rs:  164 行 (KnowledgeRepository)
  ├─ lookups.rs:    128 行 (LookupRepository)
  ├─ db.rs:         (连接池与迁移初始化)
  ├─ model.rs:      (实体映射)
  └─ 小计：2,311+ 行

✅ 迁移脚本：3 个 SQL 文件

✅ 集成测试框架：invariants.rs

总体完成度：60-70%
```

## 紧急优先实现项

### 1. 验证编译 + 集成测试框架 (30 min)
```bash
cargo build -p novel-storage-sqlite
cargo test -p novel-storage-sqlite
```

### 2. 完成缺失的关键方法 (1-2 小时)
- NovelRepository: 检查所有 15 个方法的完整实现
- WorkflowRepository: 检查事件管理方法

### 3. 创建集成测试用例 (1-2 小时)
- Novel CRUD 测试
- Chapter 操作测试
- 迁移可重入测试

### 4. 性能验证 (1 小时)
- 查询性能基准
- 连接池配置验证

## 实现路线图

```
现在 → 验证完整性 → 补全缺失 → 集成测试 → 最终提交
 ↓       (15 min)   (1-2 h)   (1-2 h)     (30 min)
验证编译                                    
运行初步测试                               Target: 2026-10-23
```

## 决策

- 复用已有框架，不重构
- 优先完整性和测试覆盖
- 跳过细微优化（后续迭代）
- 重点：编译 + 测试通过 + 完整 Repository

---

下一步：立即开始验证编译和测试
