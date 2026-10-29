# Task 4.1 实现清单 - Web 框架集成

**状态**: 🚀 开始实现  
**目标**: 验证并补充 13 个 HTTP 端点  
**时间**: 2026-10-20 开始

---

## 端点实现进度

### 故事管理 (3/3)

- [ ] POST /api/novels - 创建故事
  - 文件: crates/novel-http/src/routes/novels.rs
  - 函数: create()
  - 测试: 需补充 3 个测试
  
- [ ] GET /api/novels/:novel_id - 获取故事详情  
  - 文件: crates/novel-http/src/routes/novels.rs
  - 函数: get() 或 show()
  - 测试: 需检查是否有 4 个测试
  
- [ ] PUT /api/novels/:novel_id - 更新故事
  - 文件: crates/novel-http/src/routes/novels.rs
  - 函数: update()
  - 测试: 需补充 3 个测试

### 章节规划 (4/4)

- [ ] POST /api/novels/:novel_id/chapters/plan - 创建章节计划
  - 文件: crates/novel-http/src/routes/chapters.rs
  - 测试: 需补充 4 个测试
  
- [ ] GET /api/novels/:novel_id/chapters/plan - 获取章节计划
  - 文件: crates/novel-http/src/routes/chapters.rs
  - 测试: 需补充 3 个测试
  
- [ ] GET /api/novels/:novel_id/chapters/:chapter_id - 获取单个章节
  - 文件: crates/novel-http/src/routes/chapters.rs
  - 测试: 需补充 4 个测试
  
- [ ] PUT /api/novels/:novel_id/chapters/:chapter_id - 更新章节
  - 文件: crates/novel-http/src/routes/chapters.rs
  - 测试: 需补充 3 个测试

### 生成和流式响应 (3/3)

- [ ] POST /api/novels/:novel_id/chapters/generate - 开始章节生成 (SSE)
  - 文件: crates/novel-http/src/routes/chapters.rs
  - 注意: 需要 SSE 框架，检查 crates/novel-http/src/sse.rs
  - 测试: 需补充 5 个测试
  
- [ ] GET /api/novels/:novel_id/chapters/generate/:task_id - 获取生成进度
  - 文件: crates/novel-http/src/routes/chapters.rs
  - 测试: 需补充 4 个测试
  
- [ ] POST /api/novels/:novel_id/chapters/generate/:task_id/stop - 停止生成
  - 文件: crates/novel-http/src/routes/chapters.rs
  - 测试: 需补充 3 个测试

### 错误恢复 (3/3)

- [ ] GET /api/novels/:novel_id/recovery-points - 获取恢复点
  - 文件: crates/novel-http/src/routes/runs.rs (或新建 recovery.rs)
  - 测试: 需补充 3 个测试
  
- [ ] POST /api/novels/:novel_id/recovery-points/:point_id/resume - 从恢复点恢复
  - 文件: crates/novel-http/src/routes/runs.rs (或新建 recovery.rs)
  - 测试: 需补充 4 个测试
  
- [ ] GET /api/novels/:novel_id/errors - 获取错误日志
  - 文件: crates/novel-http/src/routes/runs.rs (或新建 recovery.rs)
  - 测试: 需补充 3 个测试

---

## 工作步骤

1. [ ] 审查现有代码，确认哪些端点已实现
2. [ ] 补充缺失的端点实现
3. [ ] 为每个端点编写单元测试（目标 35+ 个）
4. [ ] 运行 cargo test --all，确保 100% 通过
5. [ ] 检查代码覆盖率 > 85%
6. [ ] 提交代码和测试
7. [ ] 完成 Task 4.1 验收

---

## 测试目标分解

| 端点类别 | 端点数 | 测试数/端点 | 总计 |
|--------|--------|------------|------|
| 故事管理 | 3 | 3-4 | 10 |
| 章节规划 | 4 | 3-4 | 14 |
| 生成响应 | 3 | 3-5 | 12 |
| 错误恢复 | 3 | 3-4 | 10 |
| **合计** | **13** | **平均 2.7** | **46** |

目标: 35+ 个测试 ✓ (计划 46 个，超出预期)

---

## 开始日期

**2026-10-20 10:54 AM UTC+8** - 开始实现

