use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use novel_application::{
    FaultInjector, ModelSecrets, PortError, Runtime, RuntimeDeps, SystemClock, TickOutcome, UuidIds,
};
use novel_llm::OpenAiCompatibleProvider;
use novel_storage_sqlite::SqliteKernel;

struct HarnessSecrets;
#[async_trait]
impl ModelSecrets for HarnessSecrets {
    async fn api_key(&self, _provider: &str) -> Result<Option<String>, PortError> {
        Ok(Some("local-harness-key".into()))
    }
}

struct CheckpointFault {
    marker: std::path::PathBuf,
}
#[async_trait]
impl FaultInjector for CheckpointFault {
    async fn fail_after_candidate(&self, _step_id: &str) -> bool {
        false
    }
    async fn after_repair_candidate_persisted(&self) -> Result<(), PortError> {
        tokio::fs::write(&self.marker, format!("{}", std::process::id()))
            .await
            .map_err(|error| PortError::Storage(error.to_string()))?;
        std::future::pending::<()>().await;
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let db = std::env::var("P11_DB").expect("P11_DB");
    let endpoint = std::env::var("P11_ENDPOINT").expect("P11_ENDPOINT");
    let novel_id = std::env::var("P11_NOVEL").expect("P11_NOVEL");
    let owner = std::env::var("P11_OWNER").expect("P11_OWNER");
    let marker = std::env::var_os("P11_MARKER").map(std::path::PathBuf::from);
    let auto_approve = std::env::var("P11_AUTO_APPROVE").ok().as_deref() == Some("1");
    let kernel = Arc::new(
        SqliteKernel::open(std::path::Path::new(&db))
            .await
            .expect("open db"),
    );
    let fault: Arc<dyn FaultInjector> = match marker {
        Some(marker) => Arc::new(CheckpointFault { marker }),
        None => Arc::new(novel_application::NoFault),
    };
    let runtime = Runtime::new(RuntimeDeps {
        novels: kernel.clone(),
        runs: kernel.clone(),
        worlds: kernel.clone(),
        lookups: kernel.clone(),
        task_center: kernel.clone(),
        llm: Arc::new(OpenAiCompatibleProvider::new()),
        clock: Arc::new(SystemClock),
        ids: Arc::new(UuidIds),
        faults: fault,
        secrets: Arc::new(HarnessSecrets),
        owner,
        lease_ms: 120_000,
        concurrency: 1,
    });
    let _ = endpoint;
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(120) {
        match runtime.worker.tick().await.expect("worker tick") {
            TickOutcome::Advanced { status, run_id } => {
                if status == "waiting_approval" && auto_approve {
                    let detail = runtime
                        .director
                        .get_run_detail(&novel_id, &run_id)
                        .await
                        .expect("detail");
                    runtime
                        .director
                        .approve(
                            &format!("harness-{run_id}-{}", detail.control_version),
                            &run_id,
                        )
                        .await
                        .expect("approve");
                }
                if status == "waiting_approval" && !auto_approve {
                    break;
                }
                if matches!(
                    status.as_str(),
                    "paused" | "succeeded" | "cancelled" | "failed"
                ) {
                    break;
                }
            }
            TickOutcome::Idle => {
                if let Some(run) = runtime
                    .director
                    .active_run(&novel_id)
                    .await
                    .expect("active run")
                {
                    if matches!(
                        run.state.as_str(),
                        "waiting_approval" | "paused" | "succeeded" | "cancelled" | "failed"
                    ) {
                        break;
                    }
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            TickOutcome::Interrupted { .. } | TickOutcome::LostLease { .. } => break,
            TickOutcome::Stopped | TickOutcome::Saturated => break,
        }
    }
}
