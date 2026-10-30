use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use novel_application::{BatchService, NativeTaskService, Runtime, StructuredMemoryService};
use novel_llm::LlmProvider;
use novel_platform::OsSecretStore;
use novel_storage_sqlite::SqliteKernel;
use tokio_util::sync::CancellationToken;

/// Shared state for Tauri commands.
///
/// Domain/application services are injected here; command handlers only translate IPC input into
/// application calls. They never acquire business logic of their own.
pub struct AppState {
    pub runtime: Arc<Runtime>,
    pub batch: Arc<BatchService>,
    pub data_dir: std::path::PathBuf,
    pub native_tasks: Arc<NativeTaskService>,
    pub structured_memory: Arc<StructuredMemoryService>,
    pub kernel: Arc<SqliteKernel>,
    pub secrets: Arc<OsSecretStore>,
    pub llm: Arc<dyn LlmProvider>,
    pub cancellations: Mutex<HashMap<String, CancellationToken>>,
    /// 发现结果预览（仅进程内、短 TTL、一次性 token），不落库。
    pub provider_previews: crate::commands::provider::ProviderPreviews,
}
