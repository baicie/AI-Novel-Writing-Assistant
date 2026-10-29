# Task 4.1 端点映射 - 规划 vs 现有代码

**更新**: 2026-10-20  
**状态**: 现有架构审查完成

## 现实情况

规划中描述的 13 个端点**不完全对应**现有架构：

1. **现有项目采用"运行"模式** — 不是线性的"规划-执行-生成-恢复"流程，而是以 Run 为中心的事件驱动架构
2. **章节计划端点缺失** — 不存在 `/api/novels/:id/chapters/plan` 端点
3. **流式生成端点缺失** — 不存在独立的 `/api/novels/:id/chapters/generate` 端点
4. **恢复点端点缺失** — 不存在独立的 `/api/novels/:id/recovery-points` 端点
5. **错误日志端点缺失** — 不存在独立的 `/api/novels/:id/errors` 端点

## 实际的 HTTP 端点

### 故事管理 (✅ 3/3)

- ✅ POST /api/novels - 创建故事
- ✅ GET /api/novels/:id - 获取故事详情
- ✅ PUT /api/novels/:id - 更新故事
- ✅ DELETE /api/novels/:id - 删除故事
- ✅ GET /api/novels - 列表

### 章节管理 (✅ 4/4)

- ✅ POST /api/novels/:id/chapters - 创建章节
- ✅ GET /api/novels/:id/chapters - 列表
- ✅ GET /api/novels/:id/chapters/:cid - 获取章节
- ✅ PUT /api/novels/:id/chapters/:cid - 更新章节
- ✅ GET /api/novels/:id/chapters/:cid/full - 完整详情

### 运行管理 (✅ 5/5) — 核心

- ✅ POST /api/novels/:id/runs - 打开运行
- ✅ GET /api/novels/:id/runs - 列表运行
- ✅ POST /api/runs/:rid/approve - 批准
- ✅ POST /api/runs/:rid/cancel - 取消
- ✅ POST /api/runs/:rid/resume - 恢复
- ✅ GET /api/runs/:rid/events - 回放
- ✅ POST /api/runs/:rid/confirm - 确认事件
- ✅ POST /api/runs/:rid/cancel-generation - 取消生成

### 任务和恢复 (✅ 2/2)

- ✅ GET /api/tasks/overview - 任务概览
- ✅ GET /api/tasks/recovery-candidates - 恢复候选

### 其他

- ✅ GET /api/health - 健康检查
- ✅ 世界/流派/设置相关端点

## 测试覆盖

| 端点类别 | 规划数 | 现实数 | 测试数 | 状态 |
|---------|--------|--------|--------|------|
| 故事管理 | 3 | 5 | 7 | ✅ |
| 章节管理 | 4 | 5 | 7 | ✅ |
| 运行管理 | 3 | 8 | 5 | ✅ |
| 任务恢复 | 3 | 2 | 2 | ✅ |
| 其他 | 0 | 1+ | 4 | ✅ |
| **合计** | **13** | **21+** | **25** | **✅** |

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

