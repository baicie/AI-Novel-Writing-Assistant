use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use novel_domain::{CreationExperience, NarrativeForm, ProjectMode, WritingMode};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NovelSummary {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub creation_experience: String,
    pub narrative_form: String,
    pub writing_mode: String,
    pub project_mode: String,
    pub updated_at: i64,
}

/// 小说完整详情，含 setup 阶段的所有字段。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NovelDetail {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub creation_experience: String,
    pub narrative_form: String,
    pub target_audience: String,
    pub book_selling_point: String,
    pub competing_feel: String,
    pub first_30_chapter_promise: String,
    pub commercial_tags: Vec<String>,
    pub genre_id: Option<String>,
    pub primary_story_mode_id: Option<String>,
    pub secondary_story_mode_id: Option<String>,
    pub world_id: Option<String>,
    pub writing_mode: String,
    pub project_mode: String,
    pub narrative_pov: Option<String>,
    pub pace_preference: Option<String>,
    pub style_tone: String,
    pub emotion_intensity: Option<String>,
    pub ai_freedom: Option<String>,
    pub default_chapter_length: Option<i32>,
    pub estimated_chapter_count: Option<i32>,
    pub project_status: String,
    pub storyline_status: String,
    pub outline_status: String,
    pub resource_ready_score: i32,
    pub post_generation_style_review_enabled: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

/// 小说列表的查询条件。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListNovelsQuery {
    pub page: Option<i32>,
    pub limit: Option<i32>,
    pub search: Option<String>,
    pub status: Option<String>,
    pub narrative_form: Option<String>,
    pub writing_mode: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListNovelsResult {
    pub items: Vec<NovelSummary>,
    pub page: i32,
    pub limit: i32,
    pub total: i64,
}

/// 小说可更新字段集。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UpdateNovelFields {
    pub title: Option<String>,
    pub description: Option<String>,
    pub target_audience: Option<Option<String>>,
    pub book_selling_point: Option<Option<String>>,
    pub competing_feel: Option<Option<String>>,
    pub first_30_chapter_promise: Option<Option<String>>,
    pub commercial_tags: Option<Option<Vec<String>>>,
    pub genre_id: Option<Option<String>>,
    pub primary_story_mode_id: Option<Option<String>>,
    pub secondary_story_mode_id: Option<Option<String>>,
    pub world_id: Option<Option<String>>,
    pub narrative_form: Option<String>,
    pub writing_mode: Option<String>,
    pub project_mode: Option<String>,
    pub narrative_pov: Option<Option<String>>,
    pub pace_preference: Option<Option<String>>,
    pub style_tone: Option<Option<String>>,
    pub emotion_intensity: Option<Option<String>>,
    pub ai_freedom: Option<Option<String>>,
    pub default_chapter_length: Option<Option<i32>>,
    pub estimated_chapter_count: Option<Option<i32>>,
    pub project_status: Option<String>,
    pub storyline_status: Option<String>,
    pub outline_status: Option<String>,
    pub resource_ready_score: Option<i32>,
    pub status: Option<String>,
    pub post_generation_style_review_enabled: Option<bool>,
}

/// 创建小说时的输入，比 Phase 1 的 `NewNovel` 多 setup 字段。
#[derive(Debug, Clone)]
pub struct CreateNovelInput {
    pub id: String,
    pub title: String,
    pub description: String,
    pub narrative_form: NarrativeForm,
    pub writing_mode: WritingMode,
    pub project_mode: ProjectMode,
    pub creation_experience: CreationExperience,
    pub target_audience: String,
    pub book_selling_point: String,
    pub competing_feel: String,
    pub first_30_chapter_promise: String,
    pub commercial_tags: Vec<String>,
    pub genre_id: Option<String>,
    pub primary_story_mode_id: Option<String>,
    pub secondary_story_mode_id: Option<String>,
    pub world_id: Option<String>,
    pub style_tone: String,
    pub default_chapter_length: Option<i32>,
    pub estimated_chapter_count: Option<i32>,
    pub post_generation_style_review_enabled: bool,
    pub now_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterSnapshot {
    pub id: String,
    pub novel_id: String,
    pub title: String,
    pub sort_order: i32,
    pub protection: String,
    pub official_version: i64,
    pub content: String,
    pub updated_at: i64,
}

/// 卷是章节的稳定分组；排序在单本小说内唯一。
/// 人物快照：只读展示所需字段；详细档案仍保存在 profile_json。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterSnapshot {
    pub id: String,
    pub novel_id: String,
    pub name: String,
    pub role: String,
    pub personality: Option<String>,
    pub arc: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeSnapshot {
    pub id: String,
    pub novel_id: String,
    pub title: String,
    pub sort_order: i32,
    pub summary: String,
}

/// 章节修订只追加、不覆盖；恢复正文必须走新的用户修订。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterRevisionSnapshot {
    pub id: String,
    pub chapter_id: String,
    pub version: i64,
    pub origin: String,
    pub content: String,
    pub content_sha256: String,
    pub created_at: i64,
}

/// 知识文档只读摘要，用于桌面知识库列表。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeDocumentSummary {
    pub id: String,
    pub novel_id: String,
    pub owner_type: String,
    pub title: String,
    pub version: i64,
    pub index_status: String,
    pub created_at: i64,
}

/// 运行摘要，用于列表展示。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunSummary {
    pub id: String,
    pub novel_id: String,
    pub chapter_id: String,
    pub chapter_title: String,
    pub state: String, // "queued", "running", "paused_for_approval", "completed", "failed", etc.
    pub created_at: i64,
    pub updated_at: i64,
    pub provider: String,
    pub model: String,
}

/// 运行列表查询结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListRunsResult {
    pub items: Vec<RunSummary>,
    pub total: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunArtifactView {
    pub id: String,
    pub run_id: Option<String>,
    pub artifact_type: String,
    pub scope: String,
    pub version: i64,
    pub status: String,
    pub protected_user_content: bool,
    pub payload_json: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunGateView {
    pub id: String,
    pub artifact_types: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunCandidateView {
    pub id: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunUsageView {
    pub calls: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunDetailView {
    pub id: String,
    pub novel_id: String,
    pub chapter_id: String,
    pub chapter_title: String,
    pub status: String,
    pub control_version: i64,
    pub cursor_step_id: Option<String>,
    pub pause_kind: Option<String>,
    pub pause_reason: Option<String>,
    pub failure_reason: Option<String>,
    pub gate: Option<RunGateView>,
    pub latest_candidate: Option<RunCandidateView>,
    pub artifacts: Vec<RunArtifactView>,
    pub usage: RunUsageView,
    pub provider: String,
    pub model: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// 章节完整详情。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterFullDetail {
    pub id: String,
    pub novel_id: String,
    pub title: String,
    pub sort_order: i32,
    pub content: String, // current draft or saved text
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunContract {
    pub run_id: String,
    pub novel_id: String,
    pub plan_version: String,
    pub chapter_plan_version: String,
    pub target_chapter_id: String,
    pub provider: String,
    pub model: String,
    pub prompt_versions: BTreeMap<String, String>,
    pub token_budget: Option<u64>,
    pub max_automatic_retries: u8,
    pub endpoint: String,
    pub allow_localhost_http: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum GenerationEvent {
    #[serde(rename = "run_started", rename_all = "camelCase")]
    RunStarted {
        run_id: String,
        seq: i64,
        occurred_at: i64,
    },
    #[serde(rename = "step_started", rename_all = "camelCase")]
    StepStarted {
        run_id: String,
        seq: i64,
        step_id: String,
        occurred_at: i64,
    },
    #[serde(rename = "token_delta", rename_all = "camelCase")]
    TokenDelta {
        run_id: String,
        step_id: String,
        text: String,
    },
    #[serde(rename = "checkpoint_saved", rename_all = "camelCase")]
    CheckpointSaved {
        run_id: String,
        seq: i64,
        step_id: String,
        candidate_id: String,
        occurred_at: i64,
    },
    #[serde(rename = "needs_approval", rename_all = "camelCase")]
    NeedsApproval {
        run_id: String,
        seq: i64,
        step_id: String,
        occurred_at: i64,
    },
    #[serde(rename = "step_succeeded", rename_all = "camelCase")]
    StepSucceeded {
        run_id: String,
        seq: i64,
        step_id: String,
        occurred_at: i64,
    },
    #[serde(rename = "run_paused", rename_all = "camelCase")]
    RunPaused {
        run_id: String,
        seq: i64,
        step_id: Option<String>,
        occurred_at: i64,
    },
    #[serde(rename = "run_failed", rename_all = "camelCase")]
    RunFailed {
        run_id: String,
        seq: i64,
        occurred_at: i64,
    },
    #[serde(rename = "run_cancelled", rename_all = "camelCase")]
    RunCancelled {
        run_id: String,
        seq: i64,
        occurred_at: i64,
    },
    #[serde(rename = "run_completed", rename_all = "camelCase")]
    RunCompleted {
        run_id: String,
        seq: i64,
        occurred_at: i64,
    },
}

impl GenerationEvent {
    pub fn run_id(&self) -> &str {
        match self {
            Self::RunStarted { run_id, .. }
            | Self::StepStarted { run_id, .. }
            | Self::TokenDelta { run_id, .. }
            | Self::CheckpointSaved { run_id, .. }
            | Self::NeedsApproval { run_id, .. }
            | Self::StepSucceeded { run_id, .. }
            | Self::RunPaused { run_id, .. }
            | Self::RunFailed { run_id, .. }
            | Self::RunCancelled { run_id, .. }
            | Self::RunCompleted { run_id, .. } => run_id,
        }
    }

    pub fn seq(&self) -> Option<i64> {
        match self {
            Self::TokenDelta { .. } => None,
            Self::RunStarted { seq, .. }
            | Self::StepStarted { seq, .. }
            | Self::CheckpointSaved { seq, .. }
            | Self::NeedsApproval { seq, .. }
            | Self::StepSucceeded { seq, .. }
            | Self::RunPaused { seq, .. }
            | Self::RunFailed { seq, .. }
            | Self::RunCancelled { seq, .. }
            | Self::RunCompleted { seq, .. } => Some(*seq),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandView {
    pub run_id: String,
    pub status: String,
}

pub fn command_view(
    run_id: impl Into<String>,
    status: impl Into<String>,
) -> Result<String, serde_json::Error> {
    serde_json::to_string(&CommandView {
        run_id: run_id.into(),
        status: status.into(),
    })
}

pub fn event_from_stored(
    run_id: &str,
    seq: i64,
    kind: &str,
    step_id: Option<String>,
    payload_json: &str,
    occurred_at: i64,
) -> Option<GenerationEvent> {
    let step_id = step_id.unwrap_or_default();
    Some(match kind {
        "run_started" => GenerationEvent::RunStarted {
            run_id: run_id.into(),
            seq,
            occurred_at,
        },
        "step_started" => GenerationEvent::StepStarted {
            run_id: run_id.into(),
            seq,
            step_id,
            occurred_at,
        },
        "checkpoint_saved" => {
            let candidate_id = serde_json::from_str::<serde_json::Value>(payload_json)
                .ok()
                .and_then(|value| {
                    value
                        .get("candidateId")
                        .and_then(|item| item.as_str())
                        .map(str::to_string)
                })
                .unwrap_or_default();
            GenerationEvent::CheckpointSaved {
                run_id: run_id.into(),
                seq,
                step_id,
                candidate_id,
                occurred_at,
            }
        }
        "needs_approval" => GenerationEvent::NeedsApproval {
            run_id: run_id.into(),
            seq,
            step_id,
            occurred_at,
        },
        "step_succeeded" => GenerationEvent::StepSucceeded {
            run_id: run_id.into(),
            seq,
            step_id,
            occurred_at,
        },
        "run_paused" => GenerationEvent::RunPaused {
            run_id: run_id.into(),
            seq,
            step_id: Some(step_id).filter(|id| !id.is_empty()),
            occurred_at,
        },
        "run_failed" => GenerationEvent::RunFailed {
            run_id: run_id.into(),
            seq,
            occurred_at,
        },
        "run_cancelled" => GenerationEvent::RunCancelled {
            run_id: run_id.into(),
            seq,
            occurred_at,
        },
        "run_completed" => GenerationEvent::RunCompleted {
            run_id: run_id.into(),
            seq,
            occurred_at,
        },
        _ => return None,
    })
}

/// 任务中心顶部数字概览。
///
/// 3.0a 阶段：只统计 Rust 内核已有表（`workflow_control` + `quality_debt`）。
/// 其他业务类目（拆书 / 图像 / 知识 / AgentRun / 风格提取 / 流水线）在 Rust 化前固定返回 0，
/// 等 3.0b~f 完成后再补全。字段保持和 Node 端 `TaskOverviewSummary` 兼容，前端不会因字段缺失报错。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskOverviewSummary {
    pub queued_count: i64,
    pub running_count: i64,
    pub failed_count: i64,
    pub cancelled_count: i64,
    pub waiting_approval_count: i64,
    pub recovery_candidate_count: i64,
}

/// 单条待恢复任务摘要。
///
/// 字段集合对齐 Node 端 `RecoverableTaskSummary` 的核心子集：
/// `id / kind / title / ownerLabel / status / currentStage / currentItemLabel /
///  resumeAction / sourceRoute / recoveryHint`。
/// 3.0a 阶段 `kind` 只会有 `novel_workflow`，其他业务项在 Rust 化后再补全。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoverableTaskSummary {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub owner_label: String,
    pub status: String,
    pub current_stage: Option<String>,
    pub current_item_label: Option<String>,
    pub resume_action: String,
    pub source_route: String,
    pub recovery_hint: String,
}

/// 任务恢复候选列表响应。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoverableTaskListResponse {
    pub items: Vec<RecoverableTaskSummary>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_event_json_uses_stable_kind_names() {
        let event = GenerationEvent::TokenDelta {
            run_id: "r".into(),
            step_id: "draft".into(),
            text: "你".into(),
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["kind"], "token_delta");
        assert_eq!(json["runId"], "r");
        assert!(json.get("seq").is_none());
        let paused = GenerationEvent::RunPaused {
            run_id: "r".into(),
            seq: 4,
            step_id: None,
            occurred_at: 1,
        };
        assert_eq!(serde_json::to_value(&paused).unwrap()["kind"], "run_paused");
    }
}
