//! 本地创作内核的 HTTP 入口。
//!
//! 这个 crate 把 `novel-application` 里的业务能力（小说 / 章节 / 自动导演运行 / 模型设置）
//! 暴露成与 Node 旧服务相同路径的 HTTP 接口，从而让 Webview 端可以继续用
//! `fetch("/api/...")` 调用，不需要切换到 Tauri IPC。
//!
//! 设计取舍：见 `docs/wiki/architecture/backend-rustization-roadmap.md`。
//! - 单端口策略：内核独占 3000。
//! - 严格失败：未实现路由返回 501 + `migration: pending` 字段。
//! - 数据迁移：旧 Prisma 库走 `migration-tools` 一次性导入，本 crate 不直接读 Prisma。
//! - Prompt / LLM：仍由 Node 服务承担；本 crate 不直接发起模型调用。

mod error;
mod routes;
mod state;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::routing::{get, post};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

pub use error::{HttpError, HttpResult};
pub use state::KernelState;

use novel_application::Runtime;
use novel_platform::SecretStore;
use novel_storage_sqlite::SqliteKernel;

/// 构造 axum 路由树。
pub fn router(state: KernelState) -> Router {
    Router::new()
        .route("/api/health", get(routes::health::health))
        .route(
            "/api/novels",
            get(routes::novels::list).post(routes::novels::create),
        )
        .route("/api/novels/:id", get(routes::novels::detail))
        .route(
            "/api/novels/:id",
            axum::routing::put(routes::novels::update).delete(routes::novels::delete),
        )
        .route(
            "/api/novels/:id/creation-experience/:experience",
            axum::routing::post(routes::novels::set_creation_experience),
        )
        .route(
            "/api/novels/:id/chapters",
            get(routes::chapters::list).post(routes::chapters::create),
        )
        .route(
            "/api/novels/:id/chapters/:cid",
            get(routes::chapters::detail).put(routes::chapters::save),
        )
        .route("/api/novels/:id/runs", post(routes::runs::open))
        .route("/api/runs/:rid/approve", post(routes::runs::approve))
        .route("/api/runs/:rid/cancel", post(routes::runs::cancel))
        .route("/api/runs/:rid/resume", post(routes::runs::resume))
        .route("/api/runs/:rid/events", get(routes::runs::replay))
        .route(
            "/api/runs/:rid/cancel-generation",
            post(routes::runs::cancel_generation),
        )
        .route(
            "/api/worlds",
            get(routes::worlds::list).post(routes::worlds::create),
        )
        .route("/api/worlds/:id", get(routes::worlds::detail))
        .route(
            "/api/worlds/:id",
            axum::routing::put(routes::worlds::update).delete(routes::worlds::delete),
        )
        .route(
            "/api/genres",
            get(routes::onboarding::list_genres),
        )
        .route(
            "/api/story-modes",
            get(routes::onboarding::list_story_modes),
        )
        .route(
            "/api/onboarding/first-novel",
            get(routes::onboarding::first_novel),
        )
        .route(
            "/api/settings/quick-setup/status",
            get(routes::onboarding::quick_setup_status),
        )
        .route(
            "/api/setup/model",
            get(routes::model_setup::get).post(routes::model_setup::save),
        )
        .route(
            "/api/tasks/overview",
            get(routes::tasks::overview),
        )
        .route(
            "/api/tasks/recovery-candidates",
            get(routes::tasks::recovery_candidates),
        )
        .fallback(routes::not_implemented::handler)
        .layer(TraceLayer::new_for_http())
        .layer(build_cors_layer())
        .with_state(state)
}

fn build_cors_layer() -> CorsLayer {
    // webview 实际是从 `tauri://localhost` 加载的，fetch 会带 Origin 头；
    // 允许任意 origin 保持和 Node 旧服务 `CORS_ORIGIN` 一样的开放策略，
    // stage 1 切流时前端跨域不会因为 origin 变化被拦。
    CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any)
        .allow_credentials(false)
}

/// 在给定的 host:port 上启动 axum server。返回 `Serve` 句柄，调用 `.await` 才会真正跑起来。
pub fn serve(
    state: KernelState,
    host: &str,
    port: u16,
) -> impl std::future::Future<Output = Result<(), HttpError>> {
    let router = router(state);
    async move {
        let address: SocketAddr = format!("{host}:{port}")
            .parse()
            .map_err(|error| HttpError::internal(format!("无法解析 {host}:{port}：{error}")))?;
        let listener = tokio::net::TcpListener::bind(&address)
            .await
            .map_err(|error| HttpError::internal(format!("无法绑定 {address}：{error}")))?;
        tracing::info!(%address, "本地创作内核 HTTP server 已就绪");
        axum::serve(listener, router)
            .await
            .map_err(|error| HttpError::internal(format!("本地创作服务 HTTP 异常退出：{error}")))
    }
}

/// 便捷构造器。
pub fn build_state(
    runtime: Arc<Runtime>,
    secrets: Arc<dyn SecretStore>,
    kernel: Arc<SqliteKernel>,
) -> KernelState {
    KernelState::new(runtime, secrets, kernel)
}
