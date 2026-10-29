use std::sync::Arc;

use novel_application::{BatchService, Clock, Runtime, RuntimeDeps, SystemClock, UuidIds};
use novel_llm::UnconfiguredProvider;
use novel_storage_sqlite::SqliteKernel;

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
        task_center: kernel.clone(),
        llm: Arc::new(UnconfiguredProvider),
        clock: Arc::new(SystemClock),
        ids: Arc::new(UuidIds),
        faults: Arc::new(novel_application::NoFault),
        secrets: Arc::new(novel_application::NoModelSecrets),
        owner: format!("batch-barrier-{}", std::process::id()),
        lease_ms: 60_000,
        concurrency: 1,
    });
    let service = BatchService::new(kernel.clone(), Arc::new(UuidIds));
    let owner = format!("batch-barrier-{}", std::process::id());
    let _ = service
        .tick(&runtime.director, &owner, SystemClock.now_ms())
        .await;
    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
}
