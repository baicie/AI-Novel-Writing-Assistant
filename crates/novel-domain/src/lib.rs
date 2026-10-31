//! 纯领域模型。这里不允许引用 Tauri、SQLx、HTTP 客户端或系统时间。
//! 版本号、序号和租赁到期时间都由调用方传入。

mod chapter;
mod entities;
mod error;
mod provider;
mod run;
mod setup;
mod workflow;

pub use chapter::{ChapterProtection, can_mark_stale, ensure_can_promote};
pub use entities::{
    Chapter, Character, Novel, NovelMode, NovelStatus, QualityIssue, RunContract, StepStatus,
    WorkflowStep, WorkflowStepType, WorldSetting,
};
pub use error::DomainError;
pub use provider::{
    CapabilityField, CapabilityValue, CredentialKind, CredentialReference, DiscoveryErrorCode,
    Modality, ModelBinding, ModelCapabilities, ModelDiscoveryOutcome, ModelProfile, ModelSource,
    ProviderProfile, ProviderProtocol,
};
pub use run::{
    ControlEvent, PauseKind, RunControl, RunGate, RunPause, RunStatus, StopSignalKind, apply_event,
    initial_control, note_lease, parse_status,
};
pub use setup::{
    CreationExperience, FirstNovelMilestone, FirstNovelOnboarding, Genre, NarrativeForm,
    ProjectMode, QuickSetupStatus, StoryMode, World, WritingMode,
};
pub use workflow::{ArtifactStatus, ArtifactView};
