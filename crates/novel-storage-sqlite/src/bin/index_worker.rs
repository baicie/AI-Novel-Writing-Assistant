use std::sync::Arc;
use std::time::Duration;

use novel_application::IndexJobRepository;
use novel_storage_sqlite::SqliteKernel;

#[tokio::main]
async fn main() {
    let db = std::env::var("INDEX_WORKER_DB").expect("INDEX_WORKER_DB");
    let job_id = std::env::var("INDEX_WORKER_JOB").expect("INDEX_WORKER_JOB");
    let owner = std::env::var("INDEX_WORKER_OWNER").expect("INDEX_WORKER_OWNER");
    let marker = std::env::var("INDEX_WORKER_MARKER").expect("INDEX_WORKER_MARKER");
    let lease_ms = std::env::var("INDEX_WORKER_LEASE_MS")
        .unwrap_or_else(|_| "500".into())
        .parse::<i64>()
        .unwrap_or(500);
    let kernel = Arc::new(
        SqliteKernel::open(std::path::Path::new(&db))
            .await
            .expect("open db"),
    );
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_millis() as i64;
    let claimed = kernel
        .claim_index_job(&job_id, &owner, now + lease_ms, now)
        .await
        .expect("claim");
    if claimed.is_none() {
        std::process::exit(3);
    }
    std::fs::write(&marker, format!("{} {now}", std::process::id())).expect("marker");
    tokio::time::sleep(Duration::from_secs(120)).await;
}
