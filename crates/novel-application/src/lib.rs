// 这些 HTTP 应用骨架属于后续服务端内核，桌面内核尚未接线，显式允许未使用。
#[allow(dead_code)]
mod app;
mod batch;
mod context;
mod continuity;
// 编排层函数按显式参数传值，便于调用方看到完整契约；拆结构体属于后续重构，先按模块允许。
#[allow(clippy::too_many_arguments)]
mod director;
mod dto;
mod error;
mod hash;
#[allow(dead_code)]
mod http_error;
#[allow(dead_code)]
mod http_handlers;
#[allow(dead_code, clippy::collapsible_if)]
mod http_request;
#[allow(dead_code)]
mod http_response;
#[allow(dead_code)]
mod http_routes;
mod hub;
mod integration_tests;
mod knowledge;
#[allow(clippy::too_many_arguments)]
mod memory;
mod native_tasks;
mod obligations;
mod plot;
#[allow(clippy::too_many_arguments)]
mod ports;
mod provider_service;
mod runtime;
mod selection_rewrite;
mod services_chapter;
mod services_integration_test;
mod services_novel;
mod services_run;
mod setup;
mod structured_memory;
mod tasks;
mod usecase;
mod worker;

pub use batch::{
    BatchFence, BatchItemView, BatchRepository, BatchRunRequest, BatchRunView, BatchService,
    NewBatchRun,
};
pub use context::{
    CONTEXT_ASSEMBLY_VERSION, ContextAssembly, ContextPreview, ContextSourceTrace, ContextTrace,
    DEFAULT_CONTEXT_TOKEN_BUDGET, DEFAULT_OUTPUT_RESERVE, MAX_CONTEXT_TOKEN_BUDGET, RunContextPins,
    assemble_director_context, assemble_director_context_for_run, preview_director_context,
};
pub use continuity::{
    CONTINUITY_CANDIDATE_LIMIT, CONTINUITY_CONFLICT_LIMIT, ChapterContinuityProjection,
    ChapterContinuityService, ContinuityCandidate, ContinuityCharacter, ContinuityCharacterState,
    ContinuityConflict, ContinuityConflictSide, ContinuityFact, ContinuityLimits, ContinuityWorld,
};
pub use director::{DirectorService, OpenRunInput};
pub use dto::{
    CandidateEditDraftView, ChapterDraftView, ChapterFullDetail, ChapterRevisionSnapshot,
    ChapterSnapshot, CharacterSnapshot, CreateNovelInput, GenerationEvent,
    KnowledgeDocumentSummary, ListNovelsQuery, ListNovelsResult, ListRunsResult, NovelDetail,
    NovelSummary, QualityDebtView, RecoverableTaskListResponse, RecoverableTaskSummary,
    ReviewOccurrenceView, RunArtifactView, RunCandidateView, RunContract, RunDetailView,
    RunGateView, RunInputSnapshot, RunSummary, RunUsageView, TaskOverviewSummary,
    UpdateNovelFields, VolumeSnapshot, command_view, event_from_stored,
};
pub use error::{AppError, AppErrorCode, PortError};
pub use hash::{json_hash, sha256_hex};
pub use hub::EventHub;
pub use knowledge::assemble_context;
pub use memory::{ExtractMemoryInput, MemoryService};
pub use native_tasks::{
    NativeTaskQuery, NativeTaskRepository, NativeTaskService, NativeTaskSummary,
};
pub use plot::{
    CausalityEdgeView, CreatePlotThreadInput, NewCausalityEdge, PLOT_STATUS_ABANDONED,
    PLOT_STATUS_DEFERRED, PLOT_STATUS_DEVELOPING, PLOT_STATUS_OPEN, PLOT_STATUS_RESOLVED,
    PLOT_STATUS_STALE, PLOT_STATUS_SUPERSEDED, PLOT_TRANSITION_ABANDON, PLOT_TRANSITION_DEFER,
    PLOT_TRANSITION_DEVELOP, PLOT_TRANSITION_PAYOFF, PLOT_TRANSITION_RECOVER, PlotRepository,
    PlotService, PlotThreadEventView, PlotThreadView, PlotTransition, truncate_to_most_recent,
};
pub use ports::{
    CandidateBody, ChapterHeader, ClaimedRun, Clock, CommandOutcome, ControlCommit, ControlWrite,
    DeleteChapterRecord, FaultInjector, Ids, IndexJobRepository, IndexJobView,
    KnowledgeDocumentBody, KnowledgeRepository, LeaseFence, LookupRepository, MemoryCandidateView,
    MemoryConflictView, MemoryExtractionJobView, MemoryRepository, ModelSecrets, NewArtifact,
    NewAttempt, NewCandidate, NewChapter, NewCharacter, NewCharacterState, NewDocument,
    NewEmbedding, NewIndexJob, NewInvocation, NewMemoryCandidate, NewMemoryJob, NewNovel,
    NewPlotThread, NewQualityDebt, NewReviewOccurrence, NewTimelineEvent, NewVolume, NewWorld,
    NoFault, NoModelSecrets, NovelRepository, OpenRunRecord, PromoteOutcome, RestoreChapterRecord,
    RetrievalTrace, SaveCandidateEditDraftRecord, SaveChapterDraftRecord, SaveChapterRecord,
    SearchHit, StoredEvent, StoryFactView, SystemClock, TaskCenterRepository,
    UpdateChapterMetaRecord, UpdateVolumeRecord, UpdateWorldFields, UuidIds, WorkflowRepository,
    WorldRepository,
};
pub use provider_service::{
    CredentialAction, CredentialActionKind, CredentialIntents, CredentialReconcileReport,
    CredentialVault, FrozenRoute, FrozenRouteStatus, FrozenStepRoute, GenerationTarget,
    LegacyModelTarget, PendingCredentialIntent, ProviderDraft, ProviderError, ProviderErrorCode,
    ProviderRegistry, ProviderService, ProviderView, ResolvedModelRoute, RouteSnapshotRows,
    allow_keyless, apply_route_to_run, ensure_credential, resolve_generation_target,
};
pub use runtime::{Runtime, RuntimeDeps};
pub use selection_rewrite::{SelectionRewriteAttemptState, SelectionRewriteAttemptView};
pub use setup::SetupService;
pub use structured_memory::{
    NewStructuredFact, StructuredFactView, StructuredFactWithSource, StructuredMemoryRepository,
    StructuredMemoryService, visible_facts_at,
};
pub use tasks::TaskCenterService;
pub use usecase::{ChapterUseCases, NovelUseCases, PromoteOutcomeDto, RunUseCases};
pub use worker::{ConcurrencyGate, TickOutcome, Worker};
