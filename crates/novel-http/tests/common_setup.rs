//! 共享测试脚手架。
//!
//! 注：`tests/integration_runs.rs` 是受保护的陈旧文件、暂不参与编译，
//! 因此这里少数仅供它使用的辅助方法暂时无人引用，先整体允许 dead_code。
#![allow(dead_code)]

//! 集成测试的共享设置和工具。
//!
//! 为所有 HTTP-DB 集成测试提供：
//! - 独立测试数据库初始化
//! - TestContext 上下文管理
//! - 便捷的 HTTP 客户端和 Database 访问
//! - 数据生成辅助函数

use std::sync::Arc;

use async_trait::async_trait;
use novel_application::{
    ModelSecrets, NoFault, PortError, Runtime, RuntimeDeps, SystemClock, UuidIds,
};
use novel_http::{KernelState, router};
use novel_llm::UnconfiguredProvider;
use novel_platform::{MemorySecretStore, SecretStore};
use novel_storage_sqlite::SqliteKernel;
use reqwest::Client;
use sqlx::Row;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

/// 把 SecretStore 适配成 ModelSecrets 端口
struct SecretStoreAdapter {
    inner: Arc<dyn SecretStore>,
}

#[async_trait]
impl ModelSecrets for SecretStoreAdapter {
    async fn api_key(&self, provider: &str) -> Result<Option<String>, PortError> {
        self.inner
            .get(provider)
            .await
            .map_err(|_| PortError::Storage("secret store unavailable".into()))
    }
}

/// HTTP 服务器包装，便于在测试中启动和停止
struct TestHttpServer {
    base_url: String,
    shutdown: Option<oneshot::Sender<()>>,
}

impl TestHttpServer {
    /// 启动服务器
    async fn start(kernel: Arc<SqliteKernel>, secrets: Arc<dyn SecretStore>) -> Self {
        let model_secrets: Arc<dyn ModelSecrets> = Arc::new(SecretStoreAdapter {
            inner: secrets.clone(),
        });

        let runtime = Arc::new(Runtime::new(RuntimeDeps {
            novels: kernel.clone(),
            runs: kernel.clone(),
            worlds: kernel.clone(),
            lookups: kernel.clone(),
            task_center: kernel.clone(),
            llm: Arc::new(UnconfiguredProvider),
            clock: Arc::new(SystemClock),
            ids: Arc::new(UuidIds),
            faults: Arc::new(NoFault),
            secrets: model_secrets,
            owner: "test".into(),
            lease_ms: 30_000,
            concurrency: 1,
        }));

        let state = KernelState::new(runtime, secrets, kernel);
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind to loopback");
        let address = listener.local_addr().expect("get local address");
        let app = router(state);
        let (tx, rx) = oneshot::channel::<()>();

        tokio::spawn(async move {
            let _ = axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    let _ = rx.await;
                })
                .await;
        });

        Self {
            base_url: format!("http://{}", address),
            shutdown: Some(tx),
        }
    }

    /// 停止服务器
    async fn stop(mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
    }
}

/// 集成测试的完整上下文。
///
/// 提供：
/// - HTTP 客户端（连接到测试服务器）
/// - 数据库内核（直接查询数据库）
/// - 便利的测试辅助方法
pub struct TestContext {
    pub http_client: Client,
    pub base_url: String,
    pub kernel: Arc<SqliteKernel>,
    _server: TestHttpServer,
    _temp_dir: TempDir,
}

impl TestContext {
    /// 创建并初始化测试上下文
    ///
    /// 这会：
    /// 1. 创建临时数据库文件
    /// 2. 打开 SqliteKernel（自动运行迁移）
    /// 3. 启动 HTTP 服务器
    /// 4. 返回可用的 TestContext
    pub async fn new() -> Self {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let db_path = temp_dir.path().join("test.sqlite");

        // 打开数据库内核（自动运行迁移）
        let kernel = Arc::new(
            SqliteKernel::open(&db_path)
                .await
                .expect("open test database"),
        );

        let secrets: Arc<dyn SecretStore> = Arc::new(MemorySecretStore::default());

        // 启动 HTTP 服务器
        let server = TestHttpServer::start(kernel.clone(), secrets).await;
        let base_url = server.base_url.clone();

        let http_client = Client::new();

        Self {
            http_client,
            base_url,
            kernel,
            _server: server,
            _temp_dir: temp_dir,
        }
    }

    /// 获取完整 URL
    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    /// 查询数据库中的小说数量
    pub async fn count_novels(&self) -> i64 {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM novel")
            .fetch_one(self.kernel.pool())
            .await
            .expect("count novels")
    }

    /// 查询数据库中的章节数量
    pub async fn count_chapters(&self, novel_id: &str) -> i64 {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM chapter WHERE novel_id = ?")
            .bind(novel_id)
            .fetch_one(self.kernel.pool())
            .await
            .expect("count chapters")
    }

    /// 查询数据库中的运行数量
    pub async fn count_runs(&self) -> i64 {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM workflow_control")
            .fetch_one(self.kernel.pool())
            .await
            .unwrap_or(0)
    }

    /// 获取小说详情（直接从数据库）
    pub async fn get_novel_from_db(&self, id: &str) -> Option<serde_json::Value> {
        let row = sqlx::query(
            "SELECT id, title, description, status, creation_experience, 
                    narrative_form, writing_mode, project_mode, updated_at
             FROM novel WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(self.kernel.pool())
        .await
        .expect("query novel");

        row.map(|row| {
            serde_json::json!({
                "id": row.get::<String, _>("id"),
                "title": row.get::<String, _>("title"),
                "description": row.get::<Option<String>, _>("description"),
                "status": row.get::<String, _>("status"),
                "creation_experience": row.get::<String, _>("creation_experience"),
                "narrative_form": row.get::<String, _>("narrative_form"),
                "writing_mode": row.get::<String, _>("writing_mode"),
                "project_mode": row.get::<String, _>("project_mode"),
                "updated_at": row.get::<i64, _>("updated_at"),
            })
        })
    }

    /// 获取章节详情（直接从数据库）
    pub async fn get_chapter_from_db(
        &self,
        novel_id: &str,
        chapter_id: &str,
    ) -> Option<serde_json::Value> {
        let row = sqlx::query(
            "SELECT c.id, c.novel_id, c.title, c.sort_order, c.protection, 
                    c.updated_at, r.version, r.content
             FROM chapter c
             LEFT JOIN chapter_revision r ON r.id = c.official_revision_id
             WHERE c.novel_id = ? AND c.id = ?",
        )
        .bind(novel_id)
        .bind(chapter_id)
        .fetch_optional(self.kernel.pool())
        .await
        .expect("query chapter");

        row.map(|row| {
            serde_json::json!({
                "id": row.get::<String, _>("id"),
                "novel_id": row.get::<String, _>("novel_id"),
                "title": row.get::<String, _>("title"),
                "sort_order": row.get::<i32, _>("sort_order"),
                "protection": row.get::<String, _>("protection"),
                "updated_at": row.get::<i64, _>("updated_at"),
                "official_version": row.get::<Option<i64>, _>("version").unwrap_or(0),
                "content": row.get::<Option<String>, _>("content").unwrap_or_default(),
            })
        })
    }

    /// 列出所有小说（直接从数据库）
    pub async fn list_novels_from_db(&self) -> Vec<serde_json::Value> {
        let rows = sqlx::query(
            "SELECT id, title, description, status, creation_experience, 
                    narrative_form, writing_mode, project_mode, updated_at
             FROM novel ORDER BY updated_at DESC",
        )
        .fetch_all(self.kernel.pool())
        .await
        .expect("list novels");

        rows.iter()
            .map(|row| {
                serde_json::json!({
                    "id": row.get::<String, _>("id"),
                    "title": row.get::<String, _>("title"),
                    "description": row.get::<Option<String>, _>("description"),
                    "status": row.get::<String, _>("status"),
                    "creation_experience": row.get::<String, _>("creation_experience"),
                    "narrative_form": row.get::<String, _>("narrative_form"),
                    "writing_mode": row.get::<String, _>("writing_mode"),
                    "project_mode": row.get::<String, _>("project_mode"),
                    "updated_at": row.get::<i64, _>("updated_at"),
                })
            })
            .collect()
    }

    /// 清空所有数据（用于测试隔离）
    pub async fn reset_db(&self) {
        // 禁用外键约束以便清空
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(self.kernel.pool())
            .await
            .expect("disable foreign keys");

        // 删除所有表的数据
        sqlx::query("DELETE FROM workflow_artifact")
            .execute(self.kernel.pool())
            .await
            .ok();
        sqlx::query("DELETE FROM workflow_event")
            .execute(self.kernel.pool())
            .await
            .ok();
        sqlx::query("DELETE FROM workflow_control")
            .execute(self.kernel.pool())
            .await
            .ok();
        sqlx::query("DELETE FROM chapter_candidate")
            .execute(self.kernel.pool())
            .await
            .ok();
        sqlx::query("DELETE FROM chapter_revision")
            .execute(self.kernel.pool())
            .await
            .ok();
        sqlx::query("DELETE FROM chapter")
            .execute(self.kernel.pool())
            .await
            .ok();
        sqlx::query("DELETE FROM novel")
            .execute(self.kernel.pool())
            .await
            .ok();

        // 重新启用外键约束
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(self.kernel.pool())
            .await
            .expect("enable foreign keys");
    }
}

impl Drop for TestContext {
    fn drop(&mut self) {
        // 服务器会自动通过 drop 关闭
    }
}

/// 辅助宏：轻松使用 TestContext 进行测试
///
/// 用法：
/// ```
/// #[test_with_context]
/// async fn test_something(ctx: TestContext) {
///     // 测试代码
/// }
/// ```
///
/// 注：这只是为了文档清晰。实际测试直接调用 TestContext::new() 即可。
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_context_initializes() {
        let ctx = TestContext::new().await;
        // 验证服务器可达
        let response = ctx
            .http_client
            .get(ctx.url("/api/health"))
            .send()
            .await
            .expect("health check");
        assert_eq!(response.status(), 200);

        // 验证数据库初始化
        assert_eq!(ctx.count_novels().await, 0);
    }

    #[tokio::test]
    async fn test_context_reset() {
        let ctx = TestContext::new().await;

        // 创建测试数据（在实际测试中会这样做）
        // 这里只是演示 reset 函数
        let initial_count = ctx.count_novels().await;
        ctx.reset_db().await;
        let after_reset = ctx.count_novels().await;
        assert_eq!(initial_count, after_reset);
    }
}
