# Task 4.1 端点映射 - 规划 vs 现有代码

## 规划的 13 个端点 vs 现有实现

### 故事管理 (3 个)

1. ✅ POST /api/novels - 创建故事
   - 现有: `novels::create()` 在 lib.rs 已注册
   - 状态: 已实现

2. ✅ GET /api/novels/:novel_id - 获取故事详情
   - 现有: `novels::detail()` 在 lib.rs 已注册
   - 状态: 已实现

3. ✅ PUT /api/novels/:novel_id - 更新故事
   - 现有: `novels::update()` 在 lib.rs 已注册
   - 状态: 已实现

### 章节规划 (4 个)

4. ❓ POST /api/novels/:novel_id/chapters/plan - 创建章节计划
   - 现有: 可能在 `chapters::create()` 中或需要新增
   - 状态: 需要检查

5. ❓ GET /api/novels/:novel_id/chapters/plan - 获取章节计划
   - 现有: 可能在 `chapters::list()` 中或需要新增
   - 状态: 需要检查

6. ✅ GET /api/novels/:novel_id/chapters/:chapter_id - 获取单个章节
   - 现有: `chapters::detail()` 在 lib.rs 已注册
   - 状态: 已实现

7. ✅ PUT /api/novels/:novel_id/chapters/:chapter_id - 更新章节
   - 现有: `chapters::save()` 在 lib.rs 已注册
   - 状态: 已实现

### 生成和流式响应 (3 个)

8. ❓ POST /api/novels/:novel_id/chapters/generate - 开始章节生成 (SSE)
   - 现有: 可能在 `runs::open()` 中或需要新增
   - 状态: 需要检查

9. ❓ GET /api/novels/:novel_id/chapters/generate/:task_id - 获取生成进度
   - 现有: 可能在 `runs::list_runs()` 中或需要新增
   - 状态: 需要检查

10. ❓ POST /api/novels/:novel_id/chapters/generate/:task_id/stop - 停止生成
    - 现有: `runs::cancel_generation()` 可能已实现
    - 状态: 需要检查

### 错误恢复 (3 个)

11. ❓ GET /api/novels/:novel_id/recovery-points - 获取恢复点
    - 现有: 可能在 `tasks::recovery_candidates()` 中
    - 状态: 需要检查

12. ❓ POST /api/novels/:novel_id/recovery-points/:point_id/resume - 从恢复点恢复
    - 现有: 可能在 `runs::resume()` 中
    - 状态: 需要检查

13. ❓ GET /api/novels/:novel_id/errors - 获取错误日志
    - 现有: 可能在 `tasks::overview()` 中或需要新增
    - 状态: 需要检查

---

## 现有已注册的所有端点

```
GET  /api/health
GET  /api/novels
POST /api/novels
GET  /api/novels/:id
PUT  /api/novels/:id
DELETE /api/novels/:id
POST /api/novels/:id/creation-experience/:experience
GET  /api/novels/:id/chapters
POST /api/novels/:id/chapters
GET  /api/novels/:id/chapters/:cid
PUT  /api/novels/:id/chapters/:cid
GET  /api/novels/:id/chapters/:cid/full
GET  /api/novels/:id/runs
POST /api/novels/:id/runs
POST /api/runs/:rid/approve
POST /api/runs/:rid/cancel
POST /api/runs/:rid/resume
GET  /api/runs/:rid/events
POST /api/runs/:rid/confirm
POST /api/runs/:rid/cancel-generation
GET  /api/worlds
POST /api/worlds
GET  /api/worlds/:id
PUT  /api/worlds/:id
DELETE /api/worlds/:id
GET  /api/genres
GET  /api/story-modes
GET  /api/onboarding/first-novel
GET  /api/settings/quick-setup/status
GET  /api/setup/model
POST /api/setup/model
GET  /api/tasks/overview
GET  /api/tasks/recovery-candidates
```

---

## 结论

- **已实现**: 6-7 个（故事管理 3 个 + 章节部分 + 运行部分）
- **需要检查**: 6-7 个（生成/恢复等）
- **可能需要新增**: 0-2 个（如果某些端点完全缺失）

下一步: 深入审查代码，确认映射关系

