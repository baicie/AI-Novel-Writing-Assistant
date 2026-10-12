mod export;
mod logging;
mod secrets;

pub use export::{ExportChapter, ExportNovel, chapter_markdown, novel_json};
pub use logging::init_logging;
pub use novel_llm::redact_secrets;
pub use secrets::{MemorySecretStore, SecretStore};

#[cfg(feature = "os-keychain")]
pub use secrets::OsSecretStore;

pub fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}
