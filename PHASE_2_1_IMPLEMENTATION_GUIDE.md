# Phase 2.1 实现指南 - 基础存储与查询

**目标**: 实现运行列表和章节详情查询，为 HTTP 路由做准备

## 步骤 1：定义 DTO

在 `crates/novel-application/src/dto.rs` 中添加：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunSummary {
    pub id: String,
    pub novel_id: String,
    pub chapter_id: String,
    pub chapter_title: String,
    pub state: String, // "queued", "running", "paused_for_approval", "completed", "failed", etc.
    pub created_at: i64,
    pub updated_at: i64,
    pub provider: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListRunsResult {
    pub items: Vec<RunSummary>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChapterFullDetail {
    pub id: String,
    pub novel_id: String,
    pub title: String,
    pub sort_order: i32,
    pub content: String, // current draft or saved text
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
}
```

## 步骤 2：扩展 WorkflowRepository Trait

在 `crates/novel-application/src/ports.rs` 中添加两个新方法：

```rust
#[async_trait]
pub trait WorkflowRepository: Send + Sync {
    // ... existing methods ...
    
    /// 查询某本小说的所有运行，按创建时间倒序
    async fn list_runs(
        &self,
        novel_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<ListRunsResult, PortError>;
    
    /// 查询某本小说的所有章节及其最新内容
    async fn list_chapters_for_novel(
        &self,
        novel_id: &str,
    ) -> Result<Vec<ChapterFullDetail>, PortError>;
}
```

## 步骤 3：在 NovelRepository 中补充

在 `crates/novel-application/src/ports.rs` 中 `NovelRepository` trait 添加：

```rust
#[async_trait]
pub trait NovelRepository: Send + Sync {
    // ... existing methods ...
    
    /// 查询单个章节的完整内容（包括已保存正文）
    async fn get_chapter_full(&self, chapter_id: &str) -> Result<ChapterFullDetail, PortError>;
}
```

## 步骤 4：在 DirectorService 中提供业务逻辑

在 `crates/novel-application/src/director.rs` 中添加：

```rust
impl DirectorService {
    // ... existing methods ...
    
    pub async fn list_runs(
        &self,
        novel_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<ListRunsResult, AppError> {
        self.runs
            .list_runs(novel_id, limit, offset)
            .await
            .map_err(AppError::from)
    }

    pub async fn get_chapter_full(&self, chapter_id: &str) -> Result<ChapterFullDetail, AppError> {
        self.novels
            .get_chapter_full(chapter_id)
            .await
            .map_err(AppError::from)
    }
}
```

## 步骤 5：在 SQLite 中实现 Repository

在 `crates/novel-storage-sqlite/src/workflow.rs` 中实现 `list_runs` 方法：

```rust
#[async_trait]
impl WorkflowRepository for SqliteKernel {
    // ... existing methods ...
    
    async fn list_runs(
        &self,
        novel_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<ListRunsResult, PortError> {
        let mut conn = self.pool.get()
            .map_err(|e| PortError::Storage(e.to_string()))?;
        
        let total: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM workflow_run WHERE novel_id = ?",
                [novel_id],
                |row| row.get(0),
            )
            .map_err(|e| PortError::Storage(e.to_string()))?;
        
        let mut stmt = conn
            .prepare(
                "SELECT id, novel_id, chapter_id, chapter_title, state, created_at, updated_at, provider, model
                 FROM workflow_run 
                 WHERE novel_id = ? 
                 ORDER BY created_at DESC 
                 LIMIT ? OFFSET ?"
            )
            .map_err(|e| PortError::Storage(e.to_string()))?;
        
        let items = stmt
            .query_map((novel_id, limit, offset), |row| {
                Ok(RunSummary {
                    id: row.get(0)?,
                    novel_id: row.get(1)?,
                    chapter_id: row.get(2)?,
                    chapter_title: row.get(3)?,
                    state: row.get(4)?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                    provider: row.get(7)?,
                    model: row.get(8)?,
                })
            })
            .map_err(|e| PortError::Storage(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| PortError::Storage(e.to_string()))?;
        
        Ok(ListRunsResult { items, total })
    }
}
```

在 `crates/novel-storage-sqlite/src/novels.rs` 中实现 `get_chapter_full`：

```rust
#[async_trait]
impl NovelRepository for SqliteKernel {
    // ... existing methods ...
    
    async fn get_chapter_full(&self, chapter_id: &str) -> Result<ChapterFullDetail, PortError> {
        let mut conn = self.pool.get()
            .map_err(|e| PortError::Storage(e.to_string()))?;
        
        conn.query_row(
            "SELECT id, novel_id, title, sort_order, content, status, created_at, updated_at
             FROM chapter 
             WHERE id = ?",
            [chapter_id],
            |row| {
                Ok(ChapterFullDetail {
                    id: row.get(0)?,
                    novel_id: row.get(1)?,
                    title: row.get(2)?,
                    sort_order: row.get(3)?,
                    content: row.get(4)?,
                    status: row.get(5)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                })
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => PortError::NotFound(format!("Chapter {}", chapter_id)),
            e => PortError::Storage(e.to_string()),
        })
    }
}
```

## 步骤 6：在 HTTP 层注册路由

在 `crates/novel-http/src/routes/runs.rs` 中添加：

```rust
use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ListRunsQuery {
    #[serde(default = "default_limit")]
    limit: i64,
    #[serde(default)]
    offset: i64,
}

fn default_limit() -> i64 {
    100
}

pub async fn list(
    State(state): State<KernelState>,
    Path(novel_id): Path<String>,
    Query(query): Query<ListRunsQuery>,
) -> Result<Json<ListRunsResult>, HttpError> {
    let result = state
        .kernel
        .list_runs(&novel_id, query.limit, query.offset)
        .await?;
    Ok(Json(result))
}
```

在 `crates/novel-http/src/lib.rs` 中注册路由：

```rust
pub fn router(state: KernelState) -> Router {
    Router::new()
        // ... existing routes ...
        .route("/api/novels/:id/runs", get(routes::runs::list))
        // ...
}
```

在 `crates/novel-http/src/routes/chapters.rs` 中修改 `detail` 处理器添加完整内容返回。

## 步骤 7：测试

在 `crates/novel-http/tests/integration.rs` 中添加：

```rust
#[tokio::test]
async fn test_list_runs() {
    let state = test_state().await;
    
    // Create a test novel
    let novel = state
        .kernel
        .create_novel(NewNovel { /* ... */ })
        .await
        .expect("create novel");
    
    // List runs (should be empty)
    let result = state
        .kernel
        .list_runs(&novel.id, 100, 0)
        .await
        .expect("list runs");
    
    assert_eq!(result.total, 0);
    assert!(result.items.is_empty());
}

#[tokio::test]
async fn test_get_chapter_full() {
    let state = test_state().await;
    
    // Create novel and chapter
    let chapter = state
        .kernel
        .create_chapter(NewChapter { /* ... */ })
        .await
        .expect("create chapter");
    
    // Get full detail
    let detail = state
        .kernel
        .get_chapter_full(&chapter.id)
        .await
        .expect("get chapter");
    
    assert_eq!(detail.id, chapter.id);
}
```

## 关键注意事项

1. **Schema 假设**：上述实现假设 `workflow_run` 表有这些列。如果实际 schema 不同，需调整 SQL。
2. **时间戳**：确保 `created_at` / `updated_at` 使用一致的格式（毫秒还是秒）。
3. **内容字段**：`chapter.content` 是否包含完整正文，还是仅存储摘要？需与 schema 核实。
4. **错误处理**：使用 `PortError::NotFound` / `PortError::Storage` 分类不同错误。

## 验收标准

- [ ] DTO 编译通过
- [ ] 两个 Repository 方法编译通过
- [ ] DirectorService 新方法编译通过
- [ ] SQLite 实现编译通过
- [ ] HTTP 路由注册编译通过
- [ ] `cargo check --manifest-path src-tauri/Cargo.toml` 通过
- [ ] 集成测试通过
- [ ] 端到端测试：打开 `http://127.0.0.1:3000/api/novels/:id/runs` 返回正确数据

## 下一步

完成 Phase 2.1 后，继续 Phase 2.2（打开运行与事件流）。
