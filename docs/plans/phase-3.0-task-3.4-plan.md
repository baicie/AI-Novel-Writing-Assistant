# Phase 3.0 Task 3.4 - HTTP 层适配规划

> 开始日期：2026-10-20  
> 预计完成：2-3 小时  
> 状态：📋 规划中

---

## 📌 任务目标

将 Task 3.3 中完成的 UseCase 接口层转换为 HTTP REST API，为前端客户端提供完整的请求/响应接口。

---

## 🎯 核心目标

### 主要目标
- ✅ 为三个 UseCase 创建对应的 HTTP 路由
- ✅ 实现请求参数解析和验证
- ✅ 实现响应序列化和错误映射
- ✅ 编写集成测试验证 HTTP 流

### 次要目标
- ✅ OpenAPI/Swagger 文档生成
- ✅ 错误响应格式统一
- ✅ 请求/响应类型安全

---

## 📂 文件结构设计

```
server/src/
├── app/
│   └── routes/
│       ├── novels.rs          (新建) - 小说 API
│       ├── chapters.rs        (新建) - 章节 API
│       ├── runs.rs            (新建) - 运行 API
│       └── mod.rs             (修改) - 路由汇总
├── http/
│   ├── request/
│   │   ├── novel_request.rs   (新建) - 小说请求 DTO
│   │   ├── chapter_request.rs (新建) - 章节请求 DTO
│   │   └── run_request.rs     (新建) - 运行请求 DTO
│   ├── response/
│   │   ├── novel_response.rs  (新建) - 小说响应 DTO
│   │   ├── chapter_response.rs(新建) - 章节响应 DTO
│   │   └── run_response.rs    (新建) - 运行响应 DTO
│   └── error.rs               (修改) - HTTP 错误映射
└── integration_tests/
    └── http_integration_test.rs (新建) - HTTP 层集成测试
```

---

## 🏗️ API 端点设计

### 小说 API (NovelUseCases)

```http
POST /api/novels
├─ 请求：CreateNovelInput
├─ 响应：NovelDetail
└─ 状态码：201 Created / 400 Bad Request

GET /api/novels
├─ 请求：ListNovelsQuery (query params)
├─ 响应：ListNovelsResult
└─ 状态码：200 OK

GET /api/novels/:id
├─ 请求：(无)
├─ 响应：NovelDetail
└─ 状态码：200 OK / 404 Not Found

PUT /api/novels/:id
├─ 请求：UpdateNovelFields
├─ 响应：NovelDetail
└─ 状态码：200 OK / 400 / 404

DELETE /api/novels/:id
├─ 请求：(无)
├─ 响应：(无)
└─ 状态码：204 No Content / 404 Not Found

POST /api/novels/:id/creation-experience
├─ 请求：{ experience: string }
├─ 响应：NovelDetail
└─ 状态码：200 OK / 400 / 404
```

### 章节 API (ChapterUseCases)

```http
POST /api/novels/:novel_id/chapters
├─ 请求：{ title: string, sort_order: i32 }
├─ 响应：ChapterSnapshot
└─ 状态码：201 Created / 400 / 404

GET /api/novels/:novel_id/chapters
├─ 请求：(无)
├─ 响应：ChapterSnapshot[]
└─ 状态码：200 OK / 404

GET /api/novels/:novel_id/chapters/:chapter_id
├─ 请求：(无)
├─ 响应：ChapterSnapshot
└─ 状态码：200 OK / 404

PUT /api/novels/:novel_id/chapters/:chapter_id
├─ 请求：SaveChapterRecord
├─ 响应：ChapterSnapshot
└─ 状态码：200 OK / 400 / 404

POST /api/chapters/:chapter_id/promote
├─ 请求：{ candidate_id: string, revision_id: string }
├─ 响应：PromoteOutcomeDto
└─ 状态码：200 OK / 400 / 404
```

### 运行 API (RunUseCases)

```http
POST /api/novels/:novel_id/runs
├─ 请求：{ command_id: string }
├─ 响应：RunSummary
└─ 状态码：201 Created / 400 / 404

POST /api/runs/:run_id/approve
├─ 请求：{ command_id: string }
├─ 响应：RunSummary
└─ 状态码：200 OK / 400 / 404
```

---

## 📋 实现计划

### 阶段 1: 请求/响应 DTO（20 分钟）

**目标**：定义所有 HTTP 层的数据传输对象

**文件**：
- `http/request/novel_request.rs` (新建)
- `http/request/chapter_request.rs` (新建)
- `http/request/run_request.rs` (新建)
- `http/response/novel_response.rs` (新建)
- `http/response/chapter_response.rs` (新建)
- `http/response/run_response.rs` (新建)

**工作内容**：
```rust
// 示例：novel_request.rs
#[derive(Debug, Clone, Deserialize)]
pub struct CreateNovelRequest {
    pub title: String,
    pub description: Option<String>,
    // ... 其他字段
}

#[derive(Debug, Clone, Serialize)]
pub struct NovelResponse {
    pub id: String,
    pub title: String,
    // ... 其他字段
}
```

**验证方式**：
- 所有 DTO 都能正确序列化/反序列化
- 错误消息清晰明确

---

### 阶段 2: HTTP 错误映射（15 分钟）

**目标**：将 AppError 映射到合适的 HTTP 响应

**文件**：`http/error.rs` (修改/新建)

**实现**：
```rust
pub fn error_to_http_response(error: AppError) -> (StatusCode, JsonResponse) {
    match error.code {
        AppErrorCode::Validation => (StatusCode::BAD_REQUEST, error),
        AppErrorCode::NotFound => (StatusCode::NOT_FOUND, error),
        AppErrorCode::Conflict => (StatusCode::CONFLICT, error),
        AppErrorCode::IntegrityError => (StatusCode::INTERNAL_SERVER_ERROR, error),
    }
}
```

---

### 阶段 3: 路由实现（40 分钟）

**目标**：实现所有 HTTP 路由处理器

**文件**：
- `app/routes/novels.rs` (新建)
- `app/routes/chapters.rs` (新建)
- `app/routes/runs.rs` (新建)
- `app/routes/mod.rs` (修改)

**每个路由文件包含**：
```rust
pub async fn create_novel(
    State(service): State<Arc<DirectorService>>,
    Json(req): Json<CreateNovelRequest>,
) -> Result<(StatusCode, Json<NovelResponse>), AppError> {
    let input = CreateNovelInput::from(req);
    let result = service.create_novel(&input).await?;
    Ok((StatusCode::CREATED, Json(result.into())))
}
```

**验证方式**：
- 所有路由都能被调用
- 参数正确传递到 UseCase
- 响应序列化正确

---

### 阶段 4: 集成测试（30 分钟）

**目标**：完整的 HTTP 层端到端测试

**文件**：`integration_tests/http_integration_test.rs` (新建)

**测试场景**：
- ✅ 创建小说
- ✅ 查询小说列表
- ✅ 获取小说详情
- ✅ 更新小说
- ✅ 删除小说
- ✅ 创建章节
- ✅ 查询章节
- ✅ 保存章节
- ✅ 启动运行
- ✅ 批准运行
- ✅ 错误处理（404, 400, 409）

**测试框架**：使用 `reqwest` 或 `actix-test`

```rust
#[actix_web::test]
async fn test_create_novel() {
    let req = CreateNovelRequest {
        title: "Test Novel".to_string(),
        // ...
    };
    
    let resp = client.post("/api/novels")
        .json(&req)
        .send()
        .await;
    
    assert_eq!(resp.status(), StatusCode::CREATED);
    // ...
}
```

---

### 阶段 5: 文档和验证（15 分钟）

**目标**：文档完善和最终验证

**工作内容**：
- 编写 API 文档注释
- 生成 OpenAPI 规范
- 运行完整编译和测试
- 更新 Wiki 和 Release Notes

---

## 📊 关键指标

| 指标 | 目标 | 备注 |
|------|------|------|
| 代码行数 | ~1,200 行 | DTO + 路由 + 测试 |
| HTTP 端点数 | 11 个 | 6 个小说 + 5 个章节/运行 |
| 测试数 | 20+ 个 | 覆盖所有路由和错误场景 |
| 编译 | 零错误 | 类型安全验证 |
| 测试通过率 | 100% | 所有场景通过 |

---

## 🔗 依赖关系

```
Task 3.3 (使用案例) ✅ 完成
    ↓ (依赖)
Task 3.4 (HTTP 适配) ← 当前
    ↓
Task 3.5 (客户端集成)
```

---

## ⚠️ 潜在挑战

### 1. 类型转换复杂性
**问题**：需要在多个 DTO 类型之间转换
**解决**：实现 `From/Into` trait，保持转换逻辑清晰

### 2. 错误处理一致性
**问题**：确保所有错误都能正确映射到 HTTP 状态码
**解决**：在 `http/error.rs` 中统一管理映射

### 3. 验证层重复
**问题**：UseCase 层已有验证，HTTP 层是否需要重复
**解决**：HTTP 层只做基础格式验证，业务验证由 UseCase 承担

### 4. 测试数据库依赖
**问题**：集成测试需要真实的数据库连接
**解决**：使用测试数据库或 mock Repository

---

## 📝 提交计划

每个阶段完成后提交一次：

```
1. 阶段 1 完成 → "新增：Task 3.4 第一步 - 请求/响应 DTO"
2. 阶段 2 完成 → "优化：Task 3.4 第二步 - HTTP 错误映射"
3. 阶段 3 完成 → "新增：Task 3.4 第三步 - HTTP 路由实现"
4. 阶段 4 完成 → "测试：Task 3.4 第四步 - 集成测试"
5. 阶段 5 完成 → "报告：Task 3.4 - 最终完成报告"
```

---

## 🎯 成功标准

任务完成时应满足：

- ✅ 所有 11 个 HTTP 端点都能正常工作
- ✅ 请求参数正确验证和反序列化
- ✅ 响应数据正确序列化
- ✅ 错误处理返回正确的 HTTP 状态码
- ✅ 20+ 个集成测试全部通过
- ✅ 编译零错误、零致命警告
- ✅ 所有代码都有文档注释
- ✅ 架构清晰，易于维护扩展

---

## 📚 相关文档

- 前置 Task：[Task 3.3 最终报告](phase-3.0-task-3.3-final-report.md)
- 架构设计：[UseCase 层](../wiki/architecture/usecase-layer.md)
- 后续 Task：Task 3.5 - 客户端集成

---

**状态**：📋 规划完成，准备开始实现

**下一步**：执行阶段 1 - 请求/响应 DTO 定义
