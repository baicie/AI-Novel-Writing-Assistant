mod app;
mod director;
mod dto;
mod error;
mod hash;
mod http_error;
mod http_handlers;
mod http_request;
mod http_response;
mod http_routes;
mod hub;
mod integration_tests;
mod knowledge;
mod ports;
mod runtime;
mod services_chapter;
mod services_integration_test;
mod services_novel;
mod services_run;
mod setup;
mod tasks;
mod usecase;
mod worker;

pub use director::{DirectorService, OpenRunInput};
pub use dto::{
    ChapterFullDetail, ChapterRevisionSnapshot, ChapterSnapshot, CharacterSnapshot,
    CreateNovelInput, GenerationEvent, ListNovelsQuery, ListNovelsResult, ListRunsResult,
    NovelDetail, NovelSummary, RecoverableTaskListResponse, RecoverableTaskSummary, RunContract,
    RunSummary, TaskOverviewSummary, UpdateNovelFields, VolumeSnapshot, command_view,
    event_from_stored,
};
pub use error::{AppError, AppErrorCode, PortError};
pub use hash::{json_hash, sha256_hex};
pub use hub::EventHub;
pub use knowledge::assemble_context;
pub use ports::{
    CandidateBody, ClaimedRun, Clock, CommandOutcome, ControlCommit, ControlWrite,
    DeleteChapterRecord, FaultInjector, Ids, KnowledgeRepository, LookupRepository, ModelSecrets,
    NewArtifact, NewAttempt, NewCandidate, NewChapter, NewDocument, NewEmbedding, NewInvocation,
    NewNovel, NewVolume, NewWorld, NoFault, NoModelSecrets, NovelRepository, OpenRunRecord,
    PromoteOutcome, RetrievalTrace, SaveChapterRecord, SearchHit, StoredEvent, SystemClock,
    TaskCenterRepository, UpdateChapterMetaRecord, UpdateVolumeRecord, UpdateWorldFields, UuidIds,
    WorkflowRepository, WorldRepository,
};
pub use runtime::{Runtime, RuntimeDeps};
pub use setup::SetupService;
pub use tasks::TaskCenterService;
pub use usecase::{ChapterUseCases, NovelUseCases, PromoteOutcomeDto, RunUseCases};
pub use worker::{ConcurrencyGate, TickOutcome, Worker};
