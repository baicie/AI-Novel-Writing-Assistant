mod app;
mod context;
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
mod memory;
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

pub use context::{
    CONTEXT_ASSEMBLY_VERSION, ContextAssembly, ContextSourceTrace, ContextTrace,
    DEFAULT_CONTEXT_TOKEN_BUDGET, DEFAULT_OUTPUT_RESERVE, assemble_director_context,
};
pub use director::{DirectorService, OpenRunInput};
pub use dto::{
    CandidateEditDraftView, ChapterDraftView, ChapterFullDetail, ChapterRevisionSnapshot,
    ChapterSnapshot, CharacterSnapshot, CreateNovelInput, GenerationEvent,
    KnowledgeDocumentSummary, ListNovelsQuery, ListNovelsResult, ListRunsResult, NovelDetail,
    NovelSummary, QualityDebtView, RecoverableTaskListResponse, RecoverableTaskSummary,
    ReviewOccurrenceView, RunArtifactView, RunCandidateView, RunContract, RunDetailView,
    RunGateView, RunSummary, RunUsageView, TaskOverviewSummary, UpdateNovelFields, VolumeSnapshot,
    command_view, event_from_stored,
};
pub use error::{AppError, AppErrorCode, PortError};
pub use hash::{json_hash, sha256_hex};
pub use hub::EventHub;
pub use knowledge::assemble_context;
pub use memory::{ExtractMemoryInput, MemoryService};
pub use ports::{
    CandidateBody, ClaimedRun, Clock, CommandOutcome, ControlCommit, ControlWrite,
    DeleteChapterRecord, FaultInjector, Ids, IndexJobRepository, IndexJobView,
    KnowledgeDocumentBody, KnowledgeRepository, LeaseFence, LookupRepository, MemoryCandidateView,
    MemoryConflictView, MemoryExtractionJobView, MemoryRepository, ModelSecrets, NewArtifact,
    NewAttempt, NewCandidate, NewChapter, NewCharacter, NewCharacterState, NewDocument,
    NewEmbedding, NewIndexJob, NewInvocation, NewMemoryCandidate, NewMemoryJob, NewNovel,
    NewPlotThread, NewQualityDebt, NewReviewOccurrence, NewTimelineEvent, NewVolume, NewWorld,
    NoFault, NoModelSecrets, NovelRepository, OpenRunRecord, PromoteOutcome, RetrievalTrace,
    SaveCandidateEditDraftRecord, SaveChapterDraftRecord, SaveChapterRecord, SearchHit,
    StoredEvent, StoryFactView, SystemClock, TaskCenterRepository, UpdateChapterMetaRecord,
    UpdateVolumeRecord, UpdateWorldFields, UuidIds, WorkflowRepository, WorldRepository,
};
pub use runtime::{Runtime, RuntimeDeps};
pub use setup::SetupService;
pub use tasks::TaskCenterService;
pub use usecase::{ChapterUseCases, NovelUseCases, PromoteOutcomeDto, RunUseCases};
pub use worker::{ConcurrencyGate, TickOutcome, Worker};
