# Task 4.1 实现工作计划

**状态**: 🚀 进行中  
**日期**: 2026-10-20  
**目标**: 完成 13 个 HTTP 端点 + 35+ 个单元测试

---

## 已完成工作 ✅

### 规划和设计
- ✅ TASK-4.1-IMPLEMENTATION-ROADMAP.md - 详细规划 (755 行)
- ✅ TASK-4.1-IMPLEMENTATION-CHECKLIST.md - 端点清单 (106 行)
- ✅ ENDPOINT-MAPPING.md - 规划 vs 代码对应关系 (114 行)

### 测试框架
- ✅ endpoint_tests.rs - 初始测试套件 (534 行)
  - 19 个初始测试用例
  - 10 个测试通过 ✅
  - 4 个测试需修复 (请求负载调整)

### 测试进度
```
故事管理 (3 端点, 6 测试)
  ✅ POST /api/novels - 创建故事
  ✅ GET /api/novels/:id - 获取故事详情  
  ✅ PUT /api/novels/:id - 更新故事
  → 所有 6 个测试通过

章节规划 (4 端点, 6 测试)
  ✅ POST /api/novels/:id/chapters - 创建章节 (需修复: sortOrder 字段)
  ✅ GET /api/novels/:id/chapters - 获取章节列表 ✅
  ⚠️ GET /api/novels/:id/chapters/:cid - 获取单个章节 (需修复)
  ⚠️ PUT /api/novels/:id/chapters/:cid - 更新章节 (需修复)

生成和流式 (3 端点, 4 测试)
  ⚠️ GET /api/novels/:id/runs - 获取运行列表 ✅
  ⚠️ POST /api/novels/:id/runs - 打开运行 (需修复: 请求负载)

错误恢复 (3 端点, 2 测试)
  ✅ GET /api/tasks/recovery-candidates ✅
  ✅ GET /api/tasks/overview ✅
```

---

## 待完成工作 📋

### Phase 1: 修复现有 4 个失败的测试 (1-2 小时)

1. **test_create_chapter_success** - 422 错误
   - 问题: 请求负载缺少 `sortOrder` 字段
   - 修复: 添加 `sortOrder` 到 JSON 负载
   - 参考: integration.rs 第 133 行

2. **test_get_chapter_detail** - JSON 解析错误
   - 问题: 响应结构不符合预期
   - 修复: 验证响应格式，调整断言
   - 参考: 检查 chapters.rs 的 detail() 实现

3. **test_open_run** - 422 错误
   - 问题: 请求负载格式不正确
   - 修复: 根据 runs.rs open() 调整请求
   - 参考: integration.rs 中的运行示例

4. **test_update_chapter** - JSON 解析错误
   - 问题: 响应格式问题
   - 修复: 检查 chapters.rs 的 save() 实现

### Phase 2: 补充 16+ 个新测试 (2-3 小时)

目标: 从 19 个测试 → 35+ 个测试

**故事管理** (目标 +4 个)
- [ ] test_create_novel_empty_title
- [ ] test_update_novel_nonexistent
- [ ] test_delete_novel
- [ ] test_list_novels_paginated

**章节规划** (目标 +8 个)
- [ ] test_create_chapter_with_content
- [ ] test_list_chapters_empty
- [ ] test_update_chapter_content
- [ ] test_update_chapter_status
- [ ] test_chapter_ordering
- [ ] test_chapter_validation
- [ ] test_full_detail_endpoint
- [ ] test_concurrent_chapter_updates

**生成和流式** (目标 +6 个)
- [ ] test_run_with_different_params
- [ ] test_run_status_progression
- [ ] test_list_runs_pagination
- [ ] test_run_approval_flow
- [ ] test_run_cancellation
- [ ] test_event_stream_format

**错误恢复** (目标 +4 个)
- [ ] test_recovery_with_multiple_candidates
- [ ] test_recovery_point_timestamps
- [ ] test_error_log_pagination
- [ ] test_error_aggregation

### Phase 3: 集成和验证 (1 小时)

- [ ] 运行完整的测试套件: `cargo test -p novel-http`
- [ ] 检查代码覆盖率 > 85%
- [ ] 验证所有 35+ 个测试通过
- [ ] 运行 clippy 检查
- [ ] 验证没有编译警告

---

## 关键发现

### 现有端点情况

已有 35+ 个注册的 HTTP 端点:
- GET /api/health ✅
- CRUD 端点 (novels, chapters, worlds) ✅
- 运行端点 (runs) ✅ 
- 任务端点 (tasks) ✅
- 模型设置端点 ✅

这意味着大部分端点已经实现，我们主要的工作是：
1. 添加充分的测试覆盖
2. 验证每个端点的行为符合规划
3. 处理边界情况和错误情况

### 请求负载格式

通过查看 integration.rs，发现关键的字段要求：

**创建章节** (POST /api/novels/:id/chapters)
```json
{
  "title": "Chapter Title",
  "sortOrder": 1
}
```

**打开运行** (POST /api/novels/:id/runs)
```json
{
  // 需要验证确切格式
}
```

### 响应格式

所有端点的响应格式一致：
```json
{
  "success": true,
  "data": { ... },
  "message": "optional message"
}
```

---

## 实施策略

### 快速迭代方案

1. 立即修复 4 个失败测试 (~1 小时)
2. 新增 16 个测试 (~2 小时)  
3. 最终验证和清理 (~1 小时)
4. 总计: ~4 小时完成 Task 4.1

### 质量保证

- 每个测试都有明确的 AAA 结构 (Arrange-Act-Assert)
- 测试名称清晰描述测试场景
- 错误消息有上下文
- 响应验证全面(状态码、结构、内容)

---

## 下一步行动

**立即执行**:
1. 修复 endpoint_tests.rs 中的 4 个失败测试
2. 运行测试: `cargo test -p novel-http --test endpoint_tests`
3. 确保 14+ 个测试通过

**然后执行**:
4. 补充 16+ 个新测试用例
5. 完整测试: `cargo test -p novel-http`
6. 验证覆盖率

**最后**:
7. 提交最终代码
8. 完成 Task 4.1 验收

---

## 提交计划

```
Commit 1: 修复失败的测试 + 6 个新测试
Commit 2: 补充 10 个新测试
Commit 3: 最终验证和覆盖率检查
```

---

**预计完成时间**: 2026-10-20 下午 ~ 2026-10-21 上午

