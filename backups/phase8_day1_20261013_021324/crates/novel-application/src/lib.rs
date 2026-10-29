mod director;
mod dto;
mod error;
mod hash;
mod hub;
mod knowledge;
mod ports;
mod runtime;
mod worker;

pub use director::{DirectorService, OpenRunInput};
pub use dto::{
    ChapterSnapshot, GenerationEvent, NovelSummary, RunContract, command_view, event_from_stored,
};
pub use error::{AppError, AppErrorCode, PortError};
pub use hash::{json_hash, sha256_hex};
pub use hub::EventHub;
pub use knowledge::assemble_context;
pub use ports::{
    CandidateBody, ClaimedRun, Clock, CommandOutcome, ControlCommit, ControlWrite, FaultInjector,
    Ids, KnowledgeRepository, ModelSecrets, NewArtifact, NewAttempt, NewCandidate, NewChapter,
    NewDocument, NewEmbedding, NewInvocation, NewNovel, NoFault, NoModelSecrets, NovelRepository,
    OpenRunRecord, PromoteOutcome, RetrievalTrace, SaveChapterRecord, SearchHit, StoredEvent,
    SystemClock, UuidIds, WorkflowRepository,
};
pub use runtime::{Runtime, RuntimeDeps};
pub use worker::{ConcurrencyGate, TickOutcome, Worker};
