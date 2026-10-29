# Phase 3.0 Task 3.2 实现进度 - 数据库持久化层

> 任务：数据库持久化层（SQLite + SQLx）  
> 预计工期：3 天  
> 状态：实现中  
> 启动时间：2026-10-20

## 工作内容清单

### ✅ 前置完成
- [x] Task 3.1 核心领域实体定义（Novel、Chapter、Character、WorldSetting、WorkflowStep、RunContract、QualityIssue）
- [x] 编译通过、13/13 测试通过

### ⏳ Task 3.2 实现内容

#### 1. 数据库迁移脚本
- [ ] 迁移 001：novels 表
- [ ] 迁移 002：chapters 表
- [ ] 迁移 003：characters 表
- [ ] 迁移 004：worlds（settings）表
- [ ] 迁移 005：workflow_steps 表
- [ ] 迁移 006：run_contracts 表
- [ ] 迁移 007：索引与外键

#### 2. Repository Ports 定义（application 层）
- [ ] NovelRepository trait
- [ ] ChapterRepository trait
- [ ] CharacterRepository trait
- [ ] WorldSettingRepository trait
- [ ] WorkflowStepRepository trait
- [ ] RunContractRepository trait

#### 3. SQLite 实现（storage-sqlite 层）
- [ ] SQLx 连接池初始化
- [ ] 迁移执行逻辑
- [ ] NovelRepository 实现
- [ ] ChapterRepository 实现
- [ ] CharacterRepository 实现
- [ ] WorldSettingRepository 实现
- [ ] WorkflowStepRepository 实现
- [ ] RunContractRepository 实现

#### 4. 集成测试
- [ ] 数据库初始化测试
- [ ] CRUD 操作测试
- [ ] 迁移可重入测试
- [ ] 数据完整性测试

## 技术方案

### 分层架构

```
novel-application/   (Ports - trait 定义)
├─ src/ports/novel_repository.rs
├─ src/ports/chapter_repository.rs
└─ ...

novel-storage-sqlite/ (实现)
├─ migrations/
│  ├─ 001_novels.sql
│  ├─ 002_chapters.sql
│  └─ ...
├─ src/
│  ├─ db.rs (连接池、迁移初始化)
│  ├─ repos/
│  │  ├─ novel_repo.rs
│  │  ├─ chapter_repo.rs
│  │  └─ ...
│  └─ lib.rs
└─ tests/
   └─ integration_tests.rs
```

### 关键决策

1. **迁移框架**：使用 SQLx 的嵌入式迁移（`sqlx migrate!`）
2. **查询方式**：SQLx 查询宏（编译时验证）
3. **错误处理**：统一映射为 ApplicationError
4. **数据库**：SQLite + WAL 模式
5. **连接池**：SqlitePool with tuning parameters

### 开发次序

1. **第一步**：定义 Ports（application 层）
2. **第二步**：创建迁移脚本
3. **第三步**：实现 SQLx 初始化
4. **第四步**：实现各 Repository
5. **第五步**：集成测试与验证

---

## 预期成果

- SQLite 数据库 schema 完整
- 6 个 Repository 实现
- 7+ 个迁移脚本
- 集成测试 > 80% 覆盖
- 编译通过、测试通过

## 注意事项

- Domain 层实体不修改（只在 application 层添加 Ports）
- SQLx 查询必须使用宏验证
- 迁移脚本必须可重入
- 错误处理要统一，不散落
- 测试使用内存 SQLite（`:memory:`）

---

下一阶段：Task 3.3 应用层用例与事务边界
