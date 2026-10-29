# Task 3.3 任务澄清与行动计划

> 日期：2026-10-20  
> 当前状态：需要重新评估

## 情况分析

### 发现的事实
1. **项目结构**：这是一个混合项目
   - Rust 后端：`crates/` 目录下有 10+ 个 crate
   - 完整的 3 层架构已实现

2. **应用层现状**
   - ✅ Domain 层：7 个实体，13 个单元测试通过
   - ✅ Ports 层：6 个 Repository trait 定义
   - ✅ Storage 层：SQLite 实现完成
   - ✅ Application 层：已有 DirectorService、SetupService、WorkflowRepository 等
   - ✅ DTO：已有 CreateNovelInput、NovelDetail 等

3. **现有应用层代码**
   ```
   crates/novel-application/src/
   ├─ director.rs      (522 行)  - 运行导演服务
   ├─ dto.rs          (451 行)  - 数据传输对象
   ├─ error.rs         (91 行)  - 错误定义
   ├─ setup.rs        (133 行)  - 小说设置服务
   ├─ worker.rs       (786 行)  - 工作流引擎
   ├─ runtime.rs       (76 行)  - 运行时配置
   ├─ knowledge.rs     (78 行)  - 知识上下文
   └─ ... (其他模块)
   
   总计：2,715 行代码
   ```

## 任务重新定义

### 原设计文档的假设
设计文档假设需要从零实现：
- CreateNovelUseCase
- GetNovelUseCase  
- ListNovelsUseCase
- UpdateNovelUseCase
- DeleteNovelUseCase
- CreateChapterUseCase
- SaveChapterUseCase
- PromoteChapterUseCase
- 事务管理框架

### 实际情况
**大部分功能已经存在，只是组织形式不同**：
- ✅ 创建小说逻辑：在 director.rs 中（OpenRunInput）
- ✅ 设置管理逻辑：在 setup.rs 中（SetupService）
- ✅ 工作流管理逻辑：在 worker.rs 中（Worker 接口）
- ✅ DTO 定义：在 dto.rs 中

## 决策

### 选项 A：继续原设计（推荐）
**优点**：
- 符合设计文档
- 提供清晰的 UseCase 抽象
- 便于测试和维护
- 符合 DDD 架构最佳实践

**缺点**：
- 需要重新组织已有代码
- 可能产生重复代码
- 工作量较大

### 选项 B：优化现有实现
**优点**：
- 工作量小
- 快速完成
- 避免重复

**缺点**：
- 偏离设计文档
- 不符合 DDD 规范
- 后续维护困难

## 推荐方案

**执行 Option A，但使用增量方法**：

1. **识别并提取现有的业务逻辑**
   - 在 SetupService 中找出创建小说的逻辑
   - 在 DirectorService 中找出运行管理的逻辑
   - 在 Worker 中找出章节操作的逻辑

2. **创建 UseCase 包装层**
   - 不是完全重写，而是创建清晰的 UseCase 抽象
   - 委托给现有的服务实现
   - 统一的错误映射和 DTO 转换

3. **补充缺失的部分**
   - 事务管理框架
   - 统一的错误处理
   - 完整的集成测试

## 后续行动

### 第一步：代码探索
- [ ] 阅读 SetupService（创建小说逻辑）
- [ ] 阅读 DirectorService（运行管理逻辑）
- [ ] 阅读 Worker（章节操作逻辑）
- [ ] 理解 Repository trait 的实际使用

### 第二步：架构设计更新
- [ ] 更新 Task 3.3 设计文档，反映实际情况
- [ ] 明确 UseCase 的包装方式
- [ ] 定义事务管理策略

### 第三步：实现
- [ ] 创建 UseCase 封装层
- [ ] 添加错误映射
- [ ] 完善集成测试

---

**当前建议：先进行代码探索，然后制定具体的实现计划。**
