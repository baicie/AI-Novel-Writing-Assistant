use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use novel_domain::{
    ArtifactView, ControlEvent, CreationExperience, FirstNovelOnboarding, Genre, RunControl,
    StoryMode, World,
};
use novel_workflow::AttemptView;

use crate::dto::{
    CandidateEditDraftView, ChapterDraftView, ChapterFullDetail, ChapterRevisionSnapshot,
    ChapterSnapshot, CharacterSnapshot, CreateNovelInput, KnowledgeDocumentSummary,
    ListNovelsQuery, ListNovelsResult, NovelDetail, NovelSummary, QualityDebtView,
    RecoverableTaskSummary, ReviewOccurrenceView, RunArtifactView, RunContract, RunSummary,
    RunUsageView, TaskOverviewSummary, UpdateNovelFields, VolumeSnapshot,
};
use crate::error::PortError;

#[derive(Debug, Clone)]
pub struct NewNovel {
    pub id: String,
    pub title: String,
    pub description: String,
    pub now_ms: i64,
}

#[derive(Debug, Clone)]
pub struct NewCharacter {
    pub id: String,
    pub novel_id: String,
    pub name: String,
    pub role: String,
    pub now_ms: i64,
}

#[derive(Debug, Clone)]
pub struct NewChapter {
    pub id: String,
    pub novel_id: String,
    pub title: String,
    pub sort_order: i32,
    pub volume_id: Option<String>,
    pub now_ms: i64,
}

#[derive(Debug, Clone)]
pub struct SaveChapterRecord {
    pub novel_id: String,
    pub chapter_id: String,
    pub expected_version: i64,
    pub title: String,
    pub content: String,
    pub revision_id: String,
    pub artifact_id: String,
    pub content_sha256: String,
    pub now_ms: i64,
}

#[derive(Debug, Clone)]
pub struct RestoreChapterRecord {
    pub novel_id: String,
    pub chapter_id: String,
    pub source_revision_id: String,
    pub expected_version: i64,
    pub revision_id: String,
    pub artifact_id: String,
    pub command_id: String,
    pub operator: String,
    pub reason: String,
    pub draft_decision: String,
    pub draft_client_id: Option<String>,
    pub expected_draft_revision: Option<i64>,
    pub now_ms: i64,
}

pub struct SaveChapterDraftRecord {
    pub novel_id: String,
    pub chapter_id: String,
    pub client_id: String,
    pub content: String,
    pub base_official_version: i64,
    pub base_updated_at: String,
    pub expected_revision: i64,
    pub now_ms: i64,
}

#[derive(Debug, Clone)]
pub struct NewVolume {
    pub id: String,
    pub novel_id: String,
    pub title: String,
    pub sort_order: i32,
    pub summary: String,
}

#[derive(Debug, Clone)]
pub struct UpdateVolumeRecord {
    pub novel_id: String,
    pub volume_id: String,
    pub title: String,
    pub sort_order: i32,
    pub summary: String,
}

#[derive(Debug, Clone)]
pub struct DeleteChapterRecord {
    pub novel_id: String,
    pub chapter_id: String,
    pub expected_version: i64,
}

#[derive(Debug, Clone)]
pub struct UpdateChapterMetaRecord {
    pub novel_id: String,
    pub chapter_id: String,
    pub expected_version: i64,
    pub title: String,
    pub sort_order: i32,
    pub volume_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromoteOutcome {
    Promoted { version: i64 },
    Blocked,
    MissingCandidate,
}

#[derive(Debug, Clone)]
pub struct OpenRunRecord {
    pub command_id: String,
    pub request_hash: String,
    pub run_id: String,
    pub novel_id: String,
    pub contract_json: String,
    pub plan_version: String,
    pub seed_artifact_id: String,
    pub seed_payload: String,
    pub seed_hash: String,
    pub now_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandOutcome {
    pub run_id: String,
    pub status: String,
    pub replayed: bool,
}

#[derive(Debug, Clone)]
pub struct LeaseFence {
    pub owner: String,
    pub control_version: i64,
}

#[derive(Debug, Clone)]
pub struct ClaimedRun {
    pub run_id: String,
    pub novel_id: String,
    pub contract: RunContract,
    pub control: RunControl,
    pub fence: LeaseFence,
}

#[derive(Debug, Clone)]
pub struct NewArtifact {
    pub id: String,
    pub novel_id: String,
    pub run_id: Option<String>,
    pub artifact_type: String,
    pub scope: String,
    pub status: String,
    pub protected_user_content: bool,
    pub payload_json: String,
    pub content_sha256: String,
    pub created_at: i64,
}

#[derive(Debug, Clone)]
pub struct NewAttempt {
    pub id: String,
    pub run_id: String,
    pub step_id: String,
    pub control_version: i64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryCandidateView {
    pub id: String,
    pub job_id: String,
    pub novel_id: String,
    pub chapter_id: String,
    pub chapter_version: i64,
    pub source_content_hash: String,
    pub fact_type: String,
    pub truthfulness: String,
    pub entity_name: Option<String>,
    pub entity_id: Option<String>,
    pub value: String,
    pub evidence_quote: String,
    pub evidence_start: Option<i64>,
    pub confidence: Option<f64>,
    pub status: String,
    pub ambiguous: bool,
    pub version: i64,
    pub decision_reason: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryExtractionJobView {
    pub id: String,
    pub command_id: String,
    pub novel_id: String,
    pub chapter_id: String,
    pub chapter_version: i64,
    pub status: String,
    pub error: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryConflictView {
    pub id: String,
    pub novel_id: String,
    pub conflict_type: String,
    pub entity_key: String,
    pub chapter_id: String,
    pub chapter_order: i32,
    pub candidate_ids: Vec<String>,
    pub fact_ids: Vec<String>,
    pub status: String,
    pub version: i64,
    pub description: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexJobView {
    pub id: String,
    pub novel_id: String,
    pub source_type: String,
    pub source_id: String,
    pub source_revision: String,
    pub index_version: i64,
    pub status: String,
    pub attempt_count: i64,
    pub max_attempts: i64,
    pub claim_epoch: i64,
    pub next_run_at: i64,
    pub last_error_category: Option<String>,
    pub last_error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone)]
pub struct NewIndexJob {
    pub id: String,
    pub command_id: String,
    pub novel_id: String,
    pub source_type: String,
    pub source_id: String,
    pub source_revision: String,
    pub index_version: i64,
    pub max_attempts: i64,
    pub now_ms: i64,
}

#[derive(Debug, Clone)]
pub struct NewMemoryJob {
    pub id: String,
    pub command_id: String,
    pub novel_id: String,
    pub chapter_id: String,
    pub chapter_version: i64,
    pub source_content_hash: String,
    pub extraction_version: String,
    pub provider: String,
    pub model: String,
    pub endpoint: String,
    pub now_ms: i64,
}

#[derive(Debug, Clone)]
pub struct NewMemoryCandidate {
    pub id: String,
    pub job_id: String,
    pub novel_id: String,
    pub chapter_id: String,
    pub chapter_version: i64,
    pub source_content_hash: String,
    pub fact_type: String,
    pub truthfulness: String,
    pub entity_name: Option<String>,
    pub entity_id: Option<String>,
    pub value: String,
    pub evidence_quote: String,
    pub evidence_start: Option<i64>,
    pub confidence: Option<f64>,
    pub ambiguous: bool,
    pub now_ms: i64,
}

#[derive(Debug, Clone)]
pub struct NewCharacterState {
    pub id: String,
    pub character_id: String,
    pub chapter_order: i32,
    pub body: String,
    pub source_chapter_id: String,
    pub source_revision: String,
    pub status: String,
    pub now_ms: i64,
}

#[derive(Debug, Clone)]
pub struct NewTimelineEvent {
    pub id: String,
    pub novel_id: String,
    pub chapter_order: i32,
    pub body: String,
    pub source_chapter_id: String,
    pub source_revision: String,
    pub status: String,
    pub now_ms: i64,
}

#[derive(Debug, Clone)]
pub struct NewPlotThread {
    pub id: String,
    pub novel_id: String,
    pub title: String,
    pub body: String,
    pub opened_chapter_order: i32,
    pub resolved_chapter_order: Option<i32>,
    pub status: String,
    pub source_chapter_id: String,
    pub source_revision: String,
    pub now_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryFactView {
    pub id: String,
    pub fact_type: String,
    pub body: String,
    pub chapter_order: i32,
    pub source_revision: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct NewCandidate {
    pub id: String,
    pub chapter_id: String,
    pub run_id: String,
    pub step_id: String,
    pub content: String,
    pub content_sha256: String,
    pub now_ms: i64,
}

#[derive(Debug, Clone)]
pub struct NewInvocation {
    pub id: String,
    pub request_id: String,
    pub run_id: String,
    pub step_id: String,
    pub provider: String,
    pub model: String,
    pub input_summary: String,
    pub started_at: i64,
    pub finished_at: i64,
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct ControlCommit {
    pub control: RunControl,
    pub seq: i64,
}

#[derive(Debug, Clone)]
pub struct ControlWrite {
    pub run_id: String,
    pub expected_version: i64,
    pub fence: Option<LeaseFence>,
    pub event: ControlEvent,
    pub artifact: Option<NewArtifact>,
    pub event_kind: String,
    pub event_step_id: Option<String>,
    pub event_payload: String,
    pub occurred_at: i64,
    pub invocation: Option<NewInvocation>,
    pub attempt_id: Option<String>,
    pub attempt_phase: Option<String>,
    pub clear_lease: bool,
}

#[derive(Debug, Clone)]
pub struct StoredEvent {
    pub run_id: String,
    pub seq: i64,
    pub kind: String,
    pub step_id: Option<String>,
    pub payload_json: String,
    pub occurred_at: i64,
}

#[derive(Debug, Clone)]
pub struct CandidateBody {
    pub id: String,
    pub content: String,
    pub content_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeDocumentBody {
    pub id: String,
    pub novel_id: String,
    pub owner_type: String,
    pub title: String,
    pub body: String,
    pub source_revision: Option<String>,
    pub index_status: String,
    pub index_version: i64,
}

#[derive(Debug, Clone)]
pub struct NewDocument {
    pub id: String,
    pub novel_id: String,
    pub owner_type: String,
    pub title: String,
    pub body: String,
    pub terms: String,
    pub created_at: i64,
    pub embedding: Option<NewEmbedding>,
    /// Authoritative source revision, e.g. a chapter revision version.
    pub source_revision: Option<String>,
    /// Hash of the authoritative source content.
    pub source_content_hash: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewEmbedding {
    pub model: String,
    pub normalized: bool,
    pub vector: Vec<f32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub chunk_id: String,
    pub owner_type: String,
    pub body: String,
    pub source_revision: Option<String>,
    pub chunk_hash: Option<String>,
    pub index_version: i64,
}

#[derive(Debug, Clone)]
pub struct RetrievalTrace {
    pub id: String,
    pub novel_id: String,
    pub query_summary: String,
    pub chunk_ids: Vec<String>,
    pub created_at: i64,
}

pub trait Clock: Send + Sync {
    fn now_ms(&self) -> i64;
}

pub trait Ids: Send + Sync {
    fn new_id(&self) -> String;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> i64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis() as i64)
            .unwrap_or(0)
    }
}

pub struct UuidIds;

impl Ids for UuidIds {
    fn new_id(&self) -> String {
        uuid::Uuid::new_v4().to_string()
    }
}

#[async_trait]
pub trait FaultInjector: Send + Sync {
    async fn fail_after_candidate(&self, step_id: &str) -> bool;

    /// Test-only recovery checkpoint. Production implementations keep the default no-op.
    async fn after_repair_candidate_persisted(&self) -> Result<(), PortError> {
        Ok(())
    }
}

pub struct NoFault;

#[async_trait]
impl FaultInjector for NoFault {
    async fn fail_after_candidate(&self, _step_id: &str) -> bool {
        false
    }
}

#[async_trait]
pub trait MemoryRepository: Send + Sync {
    async fn memory_job_by_command(
        &self,
        command_id: &str,
    ) -> Result<Option<MemoryExtractionJobView>, PortError>;
    async fn create_memory_job(
        &self,
        job: NewMemoryJob,
        candidates: &[NewMemoryCandidate],
    ) -> Result<MemoryExtractionJobView, PortError>;
    async fn fail_memory_job(
        &self,
        job_id: &str,
        error: &str,
        now_ms: i64,
    ) -> Result<(), PortError>;
    async fn list_memory_candidates(
        &self,
        novel_id: &str,
        chapter_id: &str,
    ) -> Result<Vec<MemoryCandidateView>, PortError>;
    async fn confirm_memory_candidate(
        &self,
        candidate_id: &str,
        expected_version: i64,
        now_ms: i64,
    ) -> Result<MemoryCandidateView, PortError>;
    async fn reject_memory_candidate(
        &self,
        candidate_id: &str,
        expected_version: i64,
        reason: &str,
        now_ms: i64,
    ) -> Result<MemoryCandidateView, PortError>;
    async fn defer_memory_candidate(
        &self,
        candidate_id: &str,
        expected_version: i64,
        now_ms: i64,
    ) -> Result<MemoryCandidateView, PortError>;
    async fn update_memory_candidate_value(
        &self,
        candidate_id: &str,
        expected_version: i64,
        value: &str,
        now_ms: i64,
    ) -> Result<MemoryCandidateView, PortError>;
    async fn sync_memory_conflicts(
        &self,
        novel_id: &str,
        now_ms: i64,
    ) -> Result<Vec<MemoryConflictView>, PortError>;
    async fn list_memory_conflicts(
        &self,
        novel_id: &str,
    ) -> Result<Vec<MemoryConflictView>, PortError>;
    async fn get_memory_conflict(&self, conflict_id: &str)
    -> Result<MemoryConflictView, PortError>;
    async fn resolve_memory_conflict(
        &self,
        conflict_id: &str,
        expected_version: i64,
        command_id: &str,
        request_hash: &str,
        decision: &str,
        chosen_candidate_id: Option<&str>,
        reason: &str,
        operator: &str,
        now_ms: i64,
    ) -> Result<MemoryConflictView, PortError>;
    async fn memory_conflict_decision_by_command(
        &self,
        command_id: &str,
    ) -> Result<Option<String>, PortError>;
}

#[async_trait]
pub trait IndexJobRepository: Send + Sync {
    async fn enqueue_index_job(&self, job: NewIndexJob) -> Result<IndexJobView, PortError>;
    async fn list_index_jobs(&self, novel_id: &str) -> Result<Vec<IndexJobView>, PortError>;
    async fn claim_index_job(
        &self,
        job_id: &str,
        owner: &str,
        lease_until: i64,
        now_ms: i64,
    ) -> Result<Option<IndexJobView>, PortError>;
    async fn claim_next_due_index_job(
        &self,
        owner: &str,
        lease_until: i64,
        now_ms: i64,
    ) -> Result<Option<IndexJobView>, PortError>;
    async fn complete_index_job(
        &self,
        job_id: &str,
        owner: &str,
        claim_epoch: i64,
        now_ms: i64,
    ) -> Result<(), PortError>;
    async fn fail_index_job(
        &self,
        job_id: &str,
        owner: &str,
        claim_epoch: i64,
        category: &str,
        error: &str,
        next_run_at: i64,
        now_ms: i64,
    ) -> Result<IndexJobView, PortError>;
    async fn mark_index_job_obsolete(
        &self,
        job_id: &str,
        owner: &str,
        claim_epoch: i64,
        reason: &str,
        now_ms: i64,
    ) -> Result<IndexJobView, PortError>;
    async fn reset_index_job_for_retry(
        &self,
        job_id: &str,
        now_ms: i64,
    ) -> Result<IndexJobView, PortError>;
    async fn recover_expired_index_jobs(&self, now_ms: i64)
    -> Result<Vec<IndexJobView>, PortError>;
}

#[async_trait]
pub trait ModelSecrets: Send + Sync {
    async fn api_key(&self, provider: &str) -> Result<Option<String>, PortError>;
}

pub struct NoModelSecrets;

#[async_trait]
impl ModelSecrets for NoModelSecrets {
    async fn api_key(&self, _provider: &str) -> Result<Option<String>, PortError> {
        Ok(None)
    }
}

#[async_trait]
pub trait NovelRepository: Send + Sync {
    async fn list_novels(&self) -> Result<Vec<NovelSummary>, PortError>;
    async fn list_novels_paginated(
        &self,
        query: &ListNovelsQuery,
    ) -> Result<ListNovelsResult, PortError>;
    async fn create_novel(&self, novel: NewNovel) -> Result<NovelSummary, PortError>;
    async fn create_novel_with_setup(
        &self,
        input: CreateNovelInput,
    ) -> Result<NovelDetail, PortError>;
    async fn get_novel(&self, id: &str) -> Result<NovelDetail, PortError>;
    async fn update_novel(
        &self,
        id: &str,
        fields: &UpdateNovelFields,
        now_ms: i64,
    ) -> Result<NovelDetail, PortError>;
    async fn delete_novel(&self, id: &str) -> Result<(), PortError>;
    async fn set_creation_experience(
        &self,
        id: &str,
        experience: CreationExperience,
        now_ms: i64,
    ) -> Result<NovelDetail, PortError>;
    async fn create_chapter(&self, chapter: NewChapter) -> Result<ChapterSnapshot, PortError>;
    async fn list_chapters(&self, novel_id: &str) -> Result<Vec<ChapterSnapshot>, PortError>;
    async fn list_characters(&self, novel_id: &str) -> Result<Vec<CharacterSnapshot>, PortError>;
    async fn create_character(
        &self,
        character: NewCharacter,
    ) -> Result<CharacterSnapshot, PortError>;
    async fn list_worlds_for_context(
        &self,
        novel_id: &str,
    ) -> Result<Vec<novel_domain::World>, PortError>;
    async fn list_story_facts(
        &self,
        novel_id: &str,
        before_chapter_order: i32,
    ) -> Result<Vec<StoryFactView>, PortError>;
    /// Confirmed structured facts visible at the target chapter. Rejected,
    /// disputed and stale-source facts must never be returned by this method.
    async fn list_structured_facts_for_context(
        &self,
        novel_id: &str,
        before_chapter_order: i64,
    ) -> Result<Vec<crate::structured_memory::StructuredFactView>, PortError>;
    async fn upsert_character_state(&self, state: NewCharacterState) -> Result<(), PortError>;
    async fn upsert_timeline_event(&self, event: NewTimelineEvent) -> Result<(), PortError>;
    async fn upsert_plot_thread(&self, thread: NewPlotThread) -> Result<(), PortError>;
    async fn get_chapter(
        &self,
        novel_id: &str,
        chapter_id: &str,
    ) -> Result<ChapterSnapshot, PortError>;
    async fn save_user_chapter(
        &self,
        input: SaveChapterRecord,
    ) -> Result<ChapterSnapshot, PortError>;
    async fn save_chapter_draft(
        &self,
        record: SaveChapterDraftRecord,
    ) -> Result<ChapterDraftView, PortError>;
    async fn get_chapter_draft(
        &self,
        novel_id: &str,
        chapter_id: &str,
        client_id: &str,
    ) -> Result<Option<ChapterDraftView>, PortError>;
    async fn list_chapter_drafts(
        &self,
        novel_id: &str,
        chapter_id: &str,
    ) -> Result<Vec<ChapterDraftView>, PortError>;
    async fn clear_chapter_draft(
        &self,
        novel_id: &str,
        chapter_id: &str,
        client_id: &str,
        submitted_content: &str,
        local_revision: i64,
    ) -> Result<bool, PortError>;
    async fn save_candidate_edit_draft(
        &self,
        record: SaveCandidateEditDraftRecord,
    ) -> Result<CandidateEditDraftView, PortError>;
    async fn get_candidate_edit_draft(
        &self,
        run_id: &str,
        gate_id: &str,
        client_id: &str,
    ) -> Result<Option<CandidateEditDraftView>, PortError>;
    async fn list_candidate_edit_drafts(
        &self,
        novel_id: &str,
        run_id: &str,
    ) -> Result<Vec<CandidateEditDraftView>, PortError>;
    async fn clear_candidate_edit_draft(
        &self,
        run_id: &str,
        gate_id: &str,
        client_id: &str,
        content: &str,
        local_revision: i64,
    ) -> Result<bool, PortError>;
    async fn delete_chapter(&self, input: DeleteChapterRecord) -> Result<(), PortError>;
    async fn update_chapter_meta(
        &self,
        input: UpdateChapterMetaRecord,
    ) -> Result<ChapterSnapshot, PortError>;
    async fn restore_chapter_revision(
        &self,
        input: RestoreChapterRecord,
    ) -> Result<ChapterSnapshot, PortError>;

    async fn list_chapter_revisions(
        &self,
        novel_id: &str,
        chapter_id: &str,
    ) -> Result<Vec<ChapterRevisionSnapshot>, PortError>;
    async fn create_volume(&self, volume: NewVolume) -> Result<VolumeSnapshot, PortError>;
    async fn list_volumes(&self, novel_id: &str) -> Result<Vec<VolumeSnapshot>, PortError>;
    async fn update_volume(&self, input: UpdateVolumeRecord) -> Result<VolumeSnapshot, PortError>;
    async fn delete_volume(&self, novel_id: &str, volume_id: &str) -> Result<(), PortError>;
    async fn promote_candidate(
        &self,
        chapter_id: &str,
        candidate_id: &str,
        revision_id: &str,
        now_ms: i64,
    ) -> Result<PromoteOutcome, PortError>;
    async fn promote_candidate_fenced(
        &self,
        fence: &LeaseFence,
        run_id: &str,
        chapter_id: &str,
        candidate_id: &str,
        revision_id: &str,
        now_ms: i64,
    ) -> Result<PromoteOutcome, PortError>;
}

#[derive(Debug, Clone)]
pub struct SaveCandidateEditDraftRecord {
    pub novel_id: String,
    pub chapter_id: String,
    pub run_id: String,
    pub gate_id: String,
    pub client_id: String,
    pub base_candidate_id: String,
    pub base_content_sha256: String,
    pub content: String,
    pub control_version: i64,
    pub expected_revision: i64,
    pub now_ms: i64,
}

#[derive(Debug, Clone)]
pub struct NewReviewOccurrence {
    pub id: String,
    pub novel_id: String,
    pub chapter_id: String,
    pub run_id: String,
    pub model_issue_id: Option<String>,
    pub stable_issue_key: String,
    pub issue_type: String,
    pub severity: String,
    pub description: String,
    pub evidence: String,
    pub suggestion: String,
    pub source_ref: Option<String>,
    pub review_version: i64,
    pub created_at: i64,
}

#[derive(Debug, Clone)]
pub struct NewQualityDebt {
    pub id: String,
    pub novel_id: String,
    pub chapter_id: String,
    pub run_id: String,
    pub issue_id: String,
    pub issue_type: String,
    pub severity: String,
    pub description: String,
    pub evidence: String,
    pub suggestion: String,
    pub source_ref: Option<String>,
    pub review_version: i64,
    pub now_ms: i64,
}

#[derive(Debug, Clone)]
pub struct NewWorld {
    pub id: String,
    pub name: String,
    pub description: String,
    pub overview_summary: String,
    pub source_template_id: Option<String>,
    pub owner_novel_id: Option<String>,
    pub now_ms: i64,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateWorldFields {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
    pub overview_summary: Option<Option<String>>,
    pub source_template_id: Option<Option<String>>,
    pub owner_novel_id: Option<Option<String>>,
    pub status: Option<String>,
}

#[async_trait]
pub trait WorldRepository: Send + Sync {
    async fn list_worlds(&self) -> Result<Vec<World>, PortError>;
    async fn get_world(&self, id: &str) -> Result<Option<World>, PortError>;
    async fn create_world(&self, world: NewWorld) -> Result<World, PortError>;
    async fn update_world(
        &self,
        id: &str,
        fields: &UpdateWorldFields,
        now_ms: i64,
    ) -> Result<World, PortError>;
    async fn delete_world(&self, id: &str) -> Result<(), PortError>;
}

#[async_trait]
pub trait LookupRepository: Send + Sync {
    async fn list_genres(&self) -> Result<Vec<Genre>, PortError>;
    async fn list_story_modes(&self) -> Result<Vec<StoryMode>, PortError>;
    async fn get_first_novel_onboarding(&self) -> Result<FirstNovelOnboarding, PortError>;
}

#[async_trait]
pub trait WorkflowRepository: Send + Sync {
    async fn open_run(&self, record: OpenRunRecord) -> Result<CommandOutcome, PortError>;
    async fn claim_next(
        &self,
        owner: &str,
        now_ms: i64,
        lease_ms: i64,
    ) -> Result<Option<ClaimedRun>, PortError>;
    async fn list_artifacts(&self, novel_id: &str) -> Result<Vec<ArtifactView>, PortError>;
    async fn list_attempts(&self, run_id: &str) -> Result<Vec<AttemptView>, PortError>;
    /// 列出某小说的所有运行（按时间倒序）。
    async fn list_runs(&self, novel_id: &str) -> Result<Vec<RunSummary>, PortError>;
    /// 列出本地内核中的全部运行，供只读任务中心展示。
    async fn list_all_runs(&self) -> Result<Vec<RunSummary>, PortError>;
    /// 读取持久化控制状态，供 Director 投影使用。
    async fn run_control(&self, run_id: &str) -> Result<RunControl, PortError>;
    /// Resolve an idempotent open-run command to its committed Run, if any.
    async fn run_id_for_command(&self, command_id: &str) -> Result<Option<String>, PortError>;
    /// 读取某个 Run 的产物，按版本倒序。
    async fn run_artifacts(&self, run_id: &str) -> Result<Vec<RunArtifactView>, PortError>;
    /// 读取某个 Run 的模型调用汇总。
    async fn run_usage(&self, run_id: &str) -> Result<RunUsageView, PortError>;
    async fn record_quality_debt(&self, items: &[NewQualityDebt]) -> Result<(), PortError>;
    async fn record_quality_debt_fenced(
        &self,
        fence: &LeaseFence,
        items: &[NewQualityDebt],
        now_ms: i64,
    ) -> Result<(), PortError>;
    async fn record_review_occurrences_fenced(
        &self,
        fence: &LeaseFence,
        occurrences: &[NewReviewOccurrence],
        now_ms: i64,
    ) -> Result<(), PortError>;
    async fn accept_quality_debt(
        &self,
        run_id: &str,
        debt_id: &str,
        expected_version: i64,
        reason: &str,
        now_ms: i64,
    ) -> Result<QualityDebtView, PortError>;
    async fn list_quality_debt(
        &self,
        novel_id: &str,
        chapter_id: &str,
    ) -> Result<Vec<QualityDebtView>, PortError>;
    async fn list_review_occurrences(
        &self,
        run_id: &str,
        stable_issue_key: &str,
    ) -> Result<Vec<ReviewOccurrenceView>, PortError>;
    async fn resolve_quality_debt(
        &self,
        run_id: &str,
        evidence: &str,
        now_ms: i64,
    ) -> Result<u64, PortError>;
    async fn resolve_quality_debt_fenced(
        &self,
        fence: &LeaseFence,
        run_id: &str,
        evidence: &str,
        now_ms: i64,
    ) -> Result<u64, PortError>;
    async fn reject_gate(
        &self,
        command_id: &str,
        request_hash: &str,
        run_id: &str,
        expected_version: Option<i64>,
        regenerate: bool,
        now_ms: i64,
    ) -> Result<CommandOutcome, PortError>;
    async fn confirm_after_edit(
        &self,
        command_id: &str,
        request_hash: &str,
        run_id: &str,
        expected_version: Option<i64>,
        artifact_id: &str,
        payload_json: &str,
        now_ms: i64,
    ) -> Result<CommandOutcome, PortError>;
    async fn confirm_candidate_after_edit(
        &self,
        command_id: &str,
        request_hash: &str,
        run_id: &str,
        expected_version: Option<i64>,
        base_candidate_id: &str,
        base_content_sha256: &str,
        content: &str,
        new_candidate_id: &str,
        artifact_id: &str,
        now_ms: i64,
    ) -> Result<CommandOutcome, PortError>;
    /// 获取章节的完整详情。
    async fn get_chapter_full(
        &self,
        novel_id: &str,
        chapter_id: &str,
    ) -> Result<ChapterFullDetail, PortError>;
    async fn insert_external_artifact(&self, artifact: NewArtifact) -> Result<(), PortError>;
    async fn start_attempt(&self, attempt: NewAttempt) -> Result<(), PortError>;
    async fn start_attempt_fenced(
        &self,
        fence: &LeaseFence,
        attempt: NewAttempt,
        now_ms: i64,
    ) -> Result<(), PortError>;
    async fn mark_remote_started(
        &self,
        attempt_id: &str,
        request_id: &str,
        now_ms: i64,
    ) -> Result<bool, PortError>;
    async fn mark_remote_started_fenced(
        &self,
        fence: &LeaseFence,
        run_id: &str,
        attempt_id: &str,
        request_id: &str,
        now_ms: i64,
    ) -> Result<bool, PortError>;
    async fn save_candidate(&self, candidate: NewCandidate) -> Result<(), PortError>;
    async fn save_candidate_fenced(
        &self,
        fence: &LeaseFence,
        candidate: NewCandidate,
        now_ms: i64,
    ) -> Result<(), PortError>;
    async fn record_invocation(&self, invocation: NewInvocation) -> Result<(), PortError>;
    async fn record_invocation_fenced(
        &self,
        fence: &LeaseFence,
        invocation: NewInvocation,
        now_ms: i64,
    ) -> Result<(), PortError>;
    /// Persist Candidate + Attempt CandidateSaved + Invocation in one transaction.
    async fn save_candidate_checkpoint(
        &self,
        fence: &LeaseFence,
        candidate: NewCandidate,
        attempt_id: &str,
        invocation: NewInvocation,
        now_ms: i64,
    ) -> Result<(), PortError>;
    async fn mark_attempt(
        &self,
        attempt_id: &str,
        phase: &str,
        candidate_id: Option<&str>,
        now_ms: i64,
    ) -> Result<(), PortError>;
    async fn mark_attempt_fenced(
        &self,
        fence: &LeaseFence,
        run_id: &str,
        attempt_id: &str,
        phase: &str,
        candidate_id: Option<&str>,
        now_ms: i64,
    ) -> Result<(), PortError>;
    async fn latest_candidate(&self, run_id: &str) -> Result<Option<CandidateBody>, PortError>;
    async fn previous_candidate_id(
        &self,
        run_id: &str,
        before_id: &str,
    ) -> Result<Option<String>, PortError>;
    async fn has_invocation(
        &self,
        run_id: &str,
        step_id: &str,
        request_id: &str,
    ) -> Result<bool, PortError>;
    async fn apply_control(&self, write: ControlWrite) -> Result<ControlCommit, PortError>;
    /// Extend a live lease without changing Control Version. Returns false when the
    /// caller no longer owns the current execution right.
    async fn renew_lease(
        &self,
        fence: &LeaseFence,
        run_id: &str,
        now_ms: i64,
        lease_ms: i64,
    ) -> Result<bool, PortError>;
    async fn approve(
        &self,
        command_id: &str,
        request_hash: &str,
        run_id: &str,
        expected_version: Option<i64>,
        now_ms: i64,
    ) -> Result<CommandOutcome, PortError>;
    async fn resume(
        &self,
        command_id: &str,
        request_hash: &str,
        run_id: &str,
        intent: &str,
        now_ms: i64,
    ) -> Result<CommandOutcome, PortError>;
    async fn cancel(
        &self,
        command_id: &str,
        request_hash: &str,
        run_id: &str,
        now_ms: i64,
    ) -> Result<CommandOutcome, PortError>;
    async fn events_after(
        &self,
        run_id: &str,
        after_seq: i64,
    ) -> Result<Vec<StoredEvent>, PortError>;
    async fn remote_call_count(&self, run_id: &str, step_id: &str) -> Result<i64, PortError>;
    #[allow(clippy::too_many_arguments)]
    async fn append_debt(
        &self,
        id: &str,
        novel_id: &str,
        run_id: &str,
        chapter_order: i32,
        code: &str,
        detail: &str,
        now_ms: i64,
    ) -> Result<(), PortError>;
}

#[async_trait]
pub trait KnowledgeRepository: Send + Sync {
    async fn index_document(&self, document: NewDocument) -> Result<(), PortError>;
    async fn delete_document(&self, document_id: &str) -> Result<bool, PortError>;
    /// Rebuild a document whose previous index attempt failed or was interrupted.
    async fn retry_index_document(&self, document_id: &str) -> Result<bool, PortError>;
    async fn get_document_body(
        &self,
        document_id: &str,
    ) -> Result<Option<KnowledgeDocumentBody>, PortError>;
    async fn list_documents(&self) -> Result<Vec<KnowledgeDocumentSummary>, PortError>;
    async fn search_keyword(
        &self,
        novel_id: &str,
        query: &str,
        limit: i64,
    ) -> Result<Vec<SearchHit>, PortError>;
    async fn search_vector(
        &self,
        novel_id: &str,
        model: &str,
        vector: &[f32],
        normalized: bool,
        limit: i64,
    ) -> Result<Vec<SearchHit>, PortError>;
    async fn record_trace(&self, trace: RetrievalTrace) -> Result<(), PortError>;
    async fn latest_trace(&self, novel_id: &str) -> Result<Option<RetrievalTrace>, PortError>;
}

/// 任务中心只读数据源。
///
/// 3.0a 阶段：只覆盖 `workflow_run` / `workflow_control` / `quality_debt` 三张表，
/// 用于构建运行记录页的顶部统计和"待恢复任务"清单。其他 5 类业务（拆书 / 流水线 /
/// 图像 / 知识 / 风格提取 / AgentRun）的统计和恢复候选项在 3.0b~f 阶段补全。
#[async_trait]
pub trait TaskCenterRepository: Send + Sync {
    /// 顶部数字概览：auto_director run 的状态分组计数 + 质量债待恢复数。
    async fn task_overview(&self) -> Result<TaskOverviewSummary, PortError>;
    /// 待恢复任务清单：auto_director run 中 `status='paused'` 且有 `failure_reason` 的项。
    async fn list_recovery_candidates(&self) -> Result<Vec<RecoverableTaskSummary>, PortError>;
}
