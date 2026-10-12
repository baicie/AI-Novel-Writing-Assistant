use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NovelSummary {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub updated_at: i64,
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
        "run_completed" => GenerationEvent::RunCompleted {
            run_id: run_id.into(),
            seq,
            occurred_at,
        },
        _ => return None,
    })
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
