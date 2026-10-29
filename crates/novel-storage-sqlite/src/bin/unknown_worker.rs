use std::sync::Arc;

use async_trait::async_trait;
use novel_application::{ModelSecrets, PortError, Runtime, RuntimeDeps, SystemClock, UuidIds};
use novel_llm::OpenAiCompatibleProvider;
use novel_storage_sqlite::SqliteKernel;

struct TestSecrets;
#[async_trait]
impl ModelSecrets for TestSecrets {
    async fn api_key(&self, _provider: &str) -> Result<Option<String>, PortError> {
        Ok(Some("test-key".into()))
    }
}

#[tokio::main]
async fn main() {
    let db = std::env::args().nth(1).expect("db path");
    let kernel = Arc::new(
        SqliteKernel::open(std::path::Path::new(&db))
            .await
            .expect("open db"),
    );
    let runtime = Runtime::new(RuntimeDeps {
        novels: kernel.clone(),
        runs: kernel.clone(),
        worlds: kernel.clone(),
        lookups: kernel.clone(),
        task_center: kernel,
        llm: Arc::new(OpenAiCompatibleProvider::default()),
        clock: Arc::new(SystemClock),
        ids: Arc::new(UuidIds),
        faults: Arc::new(novel_application::NoFault),
        secrets: Arc::new(TestSecrets),
        owner: format!("unknown-worker-{}", std::process::id()),
        lease_ms: 60_000,
        concurrency: 1,
    });
    match runtime.worker.tick().await {
        Ok(outcome) => eprintln!("tick outcome: {outcome:?}"),
        Err(error) => eprintln!("tick error: {error}"),
    }
    tokio::time::sleep(std::time::Duration::from_secs(120)).await;
}
