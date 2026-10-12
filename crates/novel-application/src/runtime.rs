use std::sync::Arc;

use crate::director::DirectorService;
use crate::hub::EventHub;
use crate::ports::{Clock, FaultInjector, Ids, ModelSecrets, NovelRepository, WorkflowRepository};
use crate::worker::Worker;
use novel_llm::LlmProvider;

pub struct Runtime {
    pub director: DirectorService,
    pub worker: Worker,
    pub hub: Arc<EventHub>,
}

pub struct RuntimeDeps {
    pub novels: Arc<dyn NovelRepository>,
    pub runs: Arc<dyn WorkflowRepository>,
    pub llm: Arc<dyn LlmProvider>,
    pub clock: Arc<dyn Clock>,
    pub ids: Arc<dyn Ids>,
    pub faults: Arc<dyn FaultInjector>,
    pub secrets: Arc<dyn ModelSecrets>,
    pub owner: String,
    pub lease_ms: i64,
    pub concurrency: usize,
}

impl Runtime {
    pub fn new(deps: RuntimeDeps) -> Self {
        let hub = Arc::new(EventHub::new());
        let director = DirectorService::new(
            deps.novels.clone(),
            deps.runs.clone(),
            deps.clock.clone(),
            deps.ids.clone(),
        );
        let worker = Worker::new(
            deps.novels,
            deps.runs,
            deps.llm,
            deps.clock,
            deps.ids,
            hub.clone(),
            deps.faults,
            deps.secrets,
            deps.owner,
            deps.lease_ms,
            deps.concurrency,
        );
        Self {
            director,
            worker,
            hub,
        }
    }
}
