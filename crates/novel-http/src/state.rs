use std::sync::Arc;

use novel_application::Runtime;
use novel_platform::SecretStore;
use novel_storage_sqlite::SqliteKernel;

/// Tauri 内核里 HTTP server 持有的共享状态。
///
/// - `runtime`：novel-application 的 `Runtime`，里面是 `DirectorService` + `Worker` + `EventHub`。
/// - `secrets`：系统密钥存储（macOS Keychain / Windows DPAPI / Linux Secret Service）。
/// - `kernel`：SQLite 内核资料库，model setup 这类需要直接访问仓储的接口用它。
/// - `supported_paths`：所有已经被 Rust 内核实现的路径，给 501 响应里展示。
#[derive(Clone)]
pub struct KernelState {
    pub runtime: Arc<Runtime>,
    pub secrets: Arc<dyn SecretStore>,
    pub kernel: Arc<SqliteKernel>,
    pub supported_paths: Arc<Vec<String>>,
}

impl KernelState {
    pub fn new(
        runtime: Arc<Runtime>,
        secrets: Arc<dyn SecretStore>,
        kernel: Arc<SqliteKernel>,
    ) -> Self {
        let supported_paths = Arc::new(supported_path_list());
        Self {
            runtime,
            secrets,
            kernel,
            supported_paths,
        }
    }
}

/// 给所有已经实现的路径排序后展示，避免在 501 响应里每次顺序不同。
fn supported_path_list() -> Vec<String> {
    let mut paths: Vec<String> = [
        "GET /api/health",
        "GET /api/novels",
        "POST /api/novels",
        "GET /api/novels/:id",
        "PUT /api/novels/:id",
        "DELETE /api/novels/:id",
        "POST /api/novels/:id/creation-experience/:experience",
        "GET /api/novels/:id/chapters",
        "POST /api/novels/:id/chapters",
        "GET /api/novels/:id/chapters/:cid",
        "PUT /api/novels/:id/chapters/:cid",
        "POST /api/novels/:id/runs",
        "POST /api/runs/:rid/approve",
        "POST /api/runs/:rid/cancel",
        "POST /api/runs/:rid/resume",
        "GET /api/runs/:rid/events",
        "POST /api/runs/:rid/cancel-generation",
        "GET /api/worlds",
        "POST /api/worlds",
        "GET /api/worlds/:id",
        "PUT /api/worlds/:id",
        "DELETE /api/worlds/:id",
        "GET /api/genres",
        "GET /api/story-modes",
        "GET /api/onboarding/first-novel",
        "GET /api/settings/quick-setup/status",
        "GET /api/setup/model",
        "POST /api/setup/model",
        "GET /api/tasks/overview",
        "GET /api/tasks/recovery-candidates",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    paths.sort();
    paths
}
