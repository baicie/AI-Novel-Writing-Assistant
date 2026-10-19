# Phase 3.0 文档索引与快速参考

> 最后更新：2026-10-20  
> 分支：`feature/rust-tasks-3.0`  
> 提交：`c45f2f87`

## 🎯 快速导航

### 设计文档（设计阶段完成）
- **核心设计**：[`docs/design/2026-10-20-phase-3.0-design.md`](./2026-10-20-phase-3.0-design.md)
  - 4 项 ADR 架构决策
  - 技术选型表
  - 10 项任务详细设计

- **任务清单**：[`docs/plans/phase-3.0-task-breakdown.md`](./../../plans/phase-3.0-task-breakdown.md)
  - 10 项任务的执行指南
  - 每项 task 的验证清单
  - 工期规划与依赖关系

- **阶段总结**：[`docs/design/2026-10-20-phase-3.0-summary.md`](./2026-10-20-phase-3.0-summary.md)
  - 设计成果概览
  - 技术决策解读
  - 预期成果指标

- **当前任务**：[`TASK.md`](./../../TASK.md)
  - Phase 3.0 主任务声明
  - Phase 2.2 完成情况
  - 历史任务归档

---

## 📊 任务一览表

| Task | 名称 | 优先级 | 工期 | 前置 | 依赖 |
|------|------|-------|------|------|------|
| 3.1 | 核心领域实体 | P0 | 2d | 无 | 无 |
| 3.2 | 数据库持久化 | P0 | 3d | 3.1 | 3.1 |
| 3.3 | 应用层用例 | P0 | 2d | 3.1-3.2 | 3.1-3.2 |
| 3.4 | LLM Provider | P0 | 3d | 3.1 | 3.1 |
| 3.5 | Prompt Registry | P0 | 2d | 3.1 | 3.1 |
| 3.6 | RAG 与检索 | P1 | 3d | 3.2 | 3.2 |
| 3.7 | 工作流引擎 | P0 | 4d | 3.1-3.3 | 3.1-3.3 |
| 3.8 | Tauri 集成 | P0 | 3d | 3.1-3.7 | 3.1-3.7 |
| 3.9 | 平台层 | P1 | 2d | 3.8 | 3.8 |
| 3.10 | 测试基础设施 | P0 | 4d | 3.1-3.9 | 3.1-3.9 |

---

## 🏗️ Rust Workspace 结构

```
crates/
├── novel-domain/              # Task 3.1
│   └─ 核心实体、业务规则（0 依赖）
│
├── novel-storage-sqlite/      # Task 3.2
│   └─ SQLx、数据库迁移、Repository
│
├── novel-application/         # Task 3.3
│   └─ 用例、DTO、事务边界
│
├── novel-llm/                 # Task 3.4
│   └─ LLM Provider、SSE、结构化输出
│
├── novel-prompt/              # Task 3.5
│   └─ Prompt Registry、模板渲染、Schema
│
├── novel-context/             # Task 3.6
│   └─ 分块、FTS5、向量基础、召回
│
├── novel-workflow/            # Task 3.7
│   └─ DAG、Step、Worker、事件日志
│
├── novel-platform/            # Task 3.9
│   └─ Keychain、日志、导入导出
│
├── novel-testkit/             # Task 3.10
│   └─ Mock Provider、Fake Clock、工厂
│
└── novel-http/                # 现有
    └─ Tauri 命令、IPC 端点
```

---

## 📋 Task 详细清单

### Task 3.1：核心领域实体定义
**文件**：`docs/plans/phase-3.0-task-breakdown.md` > Task 3.1  
**工期**：2 天  
**成果**：
- [ ] `crates/novel-domain/src/entities/novel.rs`
- [ ] `crates/novel-domain/src/entities/chapter.rs`
- [ ] `crates/novel-domain/src/entities/world.rs`
- [ ] `crates/novel-domain/src/entities/character.rs`
- [ ] `crates/novel-domain/src/entities/workflow.rs`
- [ ] 单元测试覆盖 > 80%

### Task 3.2：数据库持久化层
**文件**：`docs/plans/phase-3.0-task-breakdown.md` > Task 3.2  
**工期**：3 天  
**成果**：
- [ ] SQLite 迁移脚本（`migrations/001_initial_schema.sql`）
- [ ] Repository Ports 定义
- [ ] SQLx 实现
- [ ] 集成测试通过

### Task 3.3：应用层用例
**文件**：`docs/plans/phase-3.0-task-breakdown.md` > Task 3.3  
**工期**：2 天  
**成果**：
- [ ] UseCase 实现
- [ ] DTO 定义
- [ ] 错误映射
- [ ] 单元测试通过

### Task 3.4：LLM Provider
**文件**：`docs/plans/phase-3.0-task-breakdown.md` > Task 3.4  
**工期**：3 天  
**成果**：
- [ ] Provider trait
- [ ] OpenAI 实现
- [ ] 流式响应支持
- [ ] 单元测试通过

### Task 3.5：Prompt Registry
**文件**：`docs/plans/phase-3.0-task-breakdown.md` > Task 3.5  
**工期**：2 天  
**成果**：
- [ ] PromptAsset 定义
- [ ] Registry 实现
- [ ] 模板渲染
- [ ] Schema 验证

### Task 3.6：RAG 与知识库
**文件**：`docs/plans/phase-3.0-task-breakdown.md` > Task 3.6  
**工期**：3 天  
**成果**：
- [ ] 文档分块
- [ ] FTS5 集成
- [ ] 向量存储 trait
- [ ] 上下文组装

### Task 3.7：工作流引擎
**文件**：`docs/plans/phase-3.0-task-breakdown.md` > Task 3.7  
**工期**：4 天  
**成果**：
- [ ] WorkflowDefinition
- [ ] Step Registry
- [ ] Run 与状态机
- [ ] Worker 实现
- [ ] 事件日志

### Task 3.8：Tauri 集成
**文件**：`docs/plans/phase-3.0-task-breakdown.md` > Task 3.8  
**工期**：3 天  
**成果**：
- [ ] Tauri 命令定义
- [ ] 错误映射
- [ ] Transport 适配
- [ ] 流式响应支持

### Task 3.9：平台层
**文件**：`docs/plans/phase-3.0-task-breakdown.md` > Task 3.9  
**工期**：2 天  
**成果**：
- [ ] Keychain 实现
- [ ] 日志体系
- [ ] 导入导出功能

### Task 3.10：测试基础设施
**文件**：`docs/plans/phase-3.0-task-breakdown.md` > Task 3.10  
**工期**：4 天  
**成果**：
- [ ] Mock 工具
- [ ] 单元测试（所有 crate）
- [ ] 集成测试
- [ ] 端到端测试
- [ ] 性能基准

---

## 🎯 关键路径（优先顺序）

```
优先级别：P0 > P1

第 1 周：Task 3.1-3.5（基础层）
  ├─ Task 3.1: Domain (2d)
  ├─ Task 3.2: Storage (3d) [3.1 之后]
  ├─ Task 3.3: Application (2d) [3.1-3.2 之后]
  ├─ Task 3.4: LLM (3d) [3.1 之后，并行]
  └─ Task 3.5: Prompt (2d) [3.1 之后，并行]
  
第 2 周：Task 3.6-3.8（集成层）
  ├─ Task 3.6: RAG (3d) [3.2 之后，P1]
  ├─ Task 3.7: Workflow (4d) [3.1-3.3 之后]
  └─ Task 3.8: Tauri (3d) [3.1-3.7 之后]

第 3 周：Task 3.9-3.10（完整化）
  ├─ Task 3.9: Platform (2d) [3.8 之后，P1]
  └─ Task 3.10: Test (4d) [3.1-3.9 之后]

第 4 周：汇总与交付
  ├─ 代码审查
  ├─ 文档完整化
  └─ 准备向 beta 分支合并
```

---

## ✅ 验证标准

### 每日检查（每项 task）
- [ ] `cargo build` 编译通过
- [ ] `cargo test` 单元测试通过
- [ ] `cargo clippy` 无 warning

### 每 task 完成
- [ ] 提交代码（git commit）
- [ ] 更新相关文档
- [ ] 运行集成测试
- [ ] 验证清单通过

### 每周总结（周末）
- [ ] 审查本周成果
- [ ] 更新进度文档
- [ ] 规划下周重点

### 阶段完成（所有 task）
- [ ] 所有测试通过
- [ ] 文档完整（> 90%）
- [ ] 代码覆盖率 > 70%
- [ ] 性能基准建立
- [ ] 准备向 beta 分支合并

---

## 📈 工期规划

### 总体工期
- **目标**：4 周（28 个工作日）
- **关键路径**：18 天（并行优化可达 3 周）

### 任务分布
| 优先级 | 任务 | 总工期 |
|-------|------|--------|
| P0 | Task 3.1-3.5, 3.7-3.8, 3.10 | 24 天 |
| P1 | Task 3.6, 3.9 | 5 天 |
| 总计 | | 28 天 |

### 里程碑
| 日期 | 事项 | 完成度 |
|------|------|--------|
| 2026-10-20 | 设计文档完成 | ✅ 100% |
| 2026-10-22 | Task 3.1-3.5 启动 | ⏳ |
| 2026-10-29 | Task 3.1-3.5 完成 | ⏳ |
| 2026-11-05 | Task 3.6-3.8 完成 | ⏳ |
| 2026-11-10 | Task 3.9-3.10 完成 | ⏳ |
| 2026-11-17 | 代码审查与交付 | ⏳ |

---

## 🔗 相关文档

### 架构与设计
- [Rust 原生后端重构总体设计](./2026-10-12-rust-native-kernel.md)
- [Phase 3.0 核心设计](./2026-10-20-phase-3.0-design.md)
- [Phase 3.0 任务分解](./../../plans/phase-3.0-task-breakdown.md)

### 项目文档
- [主任务清单](./../../TASK.md)
- [开发 wiki](./../../wiki/)
- [发布记录](./../../releases/release-notes.md)

### 相关代码
- [novel-http 现有实现](./../../crates/novel-http/)
- [desktop 集成](./../../desktop/)
- [client 前端](./../../client/)

---

## 🚀 启动 Phase 3.0

### 前置条件
- ✅ Phase 2.2 已完成
- ✅ 设计文档已审查
- ✅ 分支已创建：`feature/rust-tasks-3.0`

### 启动步骤

1. **审查设计**
   ```bash
   # 逐项检查 Task 3.0-3.10
   cat docs/design/2026-10-20-phase-3.0-design.md
   cat docs/plans/phase-3.0-task-breakdown.md
   ```

2. **建立 Workspace**
   ```bash
   cd crates
   cargo new novel-domain --lib
   cargo new novel-application --lib
   cargo new novel-storage-sqlite --lib
   # ... 其他 crate
   ```

3. **配置测试框架**
   ```bash
   cargo test --all
   # 配置 CI/CD 与覆盖率
   ```

4. **启动 Task 3.1**
   ```bash
   cd crates/novel-domain
   # 定义核心实体
   ```

5. **建立进度跟踪**
   - 每日编译检查
   - 每周进度更新
   - 检查点验证

---

## 📞 支持与反馈

- **设计评审**：审查 ADR 与技术决策
- **任务澄清**：逐项 task 的详细说明见任务清单
- **问题反馈**：GitHub Issues
- **讨论交流**：GitHub Discussions

---

*文档最后更新*：2026-10-20  
*工作分支*：`feature/rust-tasks-3.0`  
*预期完成*：2026-11-17
