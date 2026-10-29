use std::sync::Arc;

use crate::director::DirectorService;
use crate::hub::EventHub;
use crate::ports::{
    Clock, FaultInjector, Ids, LookupRepository, ModelSecrets, NovelRepository,
    TaskCenterRepository, WorkflowRepository, WorldRepository,
};
use crate::setup::SetupService;
use crate::tasks::TaskCenterService;
use crate::worker::Worker;
use novel_llm::LlmProvider;

pub struct Runtime {
    pub director: DirectorService,
    pub setup: SetupService,
    pub tasks: TaskCenterService,
    pub worker: Worker,
    pub hub: Arc<EventHub>,
}

pub struct RuntimeDeps {
    pub novels: Arc<dyn NovelRepository>,
    pub runs: Arc<dyn WorkflowRepository>,
    pub worlds: Arc<dyn WorldRepository>,
    pub lookups: Arc<dyn LookupRepository>,
    pub task_center: Arc<dyn TaskCenterRepository>,
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
        let setup = SetupService::new(
            deps.worlds.clone(),
            deps.lookups.clone(),
            deps.ids.clone(),
            deps.clock.clone(),
            deps.secrets.clone(),
        );
        let tasks = TaskCenterService::new(deps.task_center.clone());
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
            setup,
            tasks,
            worker,
            hub,
        }
    }
}
