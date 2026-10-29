use serde::{Deserialize, Serialize};

use crate::error::DomainError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Queued,
    Running,
    WaitingApproval,
    Paused,
    Succeeded,
    Failed,
    Cancelled,
}

impl RunStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::WaitingApproval => "waiting_approval",
            Self::Paused => "paused",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn is_active(self) -> bool {
        matches!(
            self,
            Self::Queued | Self::Running | Self::WaitingApproval | Self::Paused
        )
    }

    pub fn is_claimable(self) -> bool {
        matches!(self, Self::Queued | Self::Running)
    }
}

pub fn parse_status(value: &str) -> Result<RunStatus, DomainError> {
    match value {
        "queued" => Ok(RunStatus::Queued),
        "running" => Ok(RunStatus::Running),
        "waiting_approval" => Ok(RunStatus::WaitingApproval),
        "paused" => Ok(RunStatus::Paused),
        "succeeded" => Ok(RunStatus::Succeeded),
        "failed" => Ok(RunStatus::Failed),
        "cancelled" => Ok(RunStatus::Cancelled),
        _ => Err(DomainError::Invariant("unknown run status")),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PauseKind {
    ManualRecovery,
    Replan,
    Safety,
    MigrationReview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopSignalKind {
    Budget,
    DataIntegrity,
    ProviderUnavailable,
    ApprovalRequired,
    MigrationReviewRequired,
    Replan,
    Safety,
    NoUsableContent,
    ManualRecovery,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunPause {
    pub kind: PauseKind,
    pub signal: StopSignalKind,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunGate {
    pub id: String,
    pub artifact_types: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunControl {
    pub version: i64,
    pub status: RunStatus,
    pub pause: Option<RunPause>,
    pub gate: Option<RunGate>,
    pub cursor_step_id: Option<String>,
    pub failure_reason: Option<String>,
}

pub fn initial_control() -> RunControl {
    RunControl {
        version: 0,
        status: RunStatus::Queued,
        pause: None,
        gate: None,
        cursor_step_id: None,
        failure_reason: None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlEvent {
    Start,
    StepStarted {
        step_id: String,
    },
    StepSucceeded,
    OpenGate {
        gate_id: String,
        artifact_types: Vec<String>,
    },
    ResolveGate,
    Pause {
        pause: RunPause,
    },
    Resume,
    Succeed,
    Fail {
        reason: String,
    },
    Cancel,
}

impl ControlEvent {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::StepStarted { .. } => "step_started",
            Self::StepSucceeded => "step_succeeded",
            Self::OpenGate { .. } => "needs_approval",
            Self::ResolveGate => "resolve_approval",
            Self::Pause { .. } => "pause",
            Self::Resume => "resume",
            Self::Succeed => "succeed",
            Self::Fail { .. } => "fail",
            Self::Cancel => "cancel",
        }
    }
}

pub fn apply_event(
    control: &RunControl,
    event: ControlEvent,
    expected_version: Option<i64>,
) -> Result<RunControl, DomainError> {
    if let Some(expected) = expected_version
        && expected != control.version
    {
        return Err(DomainError::VersionConflict {
            expected,
            actual: control.version,
        });
    }
    let next_status =
        transition(control.status, &event).ok_or_else(|| DomainError::InvalidTransition {
            from: control.status.as_str().to_string(),
            event: event.name().to_string(),
        })?;
    let mut next = control.clone();
    next.version += 1;
    next.status = next_status;
    match event {
        ControlEvent::StepStarted { step_id } => {
            next.cursor_step_id = Some(step_id);
        }
        ControlEvent::StepSucceeded => {
            next.cursor_step_id = None;
        }
        ControlEvent::OpenGate {
            gate_id,
            artifact_types,
        } => {
            next.gate = Some(RunGate {
                id: gate_id,
                artifact_types,
            });
            next.cursor_step_id = None;
        }
        ControlEvent::ResolveGate => {
            next.gate = None;
        }
        ControlEvent::Pause { pause } => {
            next.pause = Some(pause);
            next.cursor_step_id = None;
        }
        ControlEvent::Resume => {
            next.pause = None;
        }
        ControlEvent::Fail { reason } => {
            next.failure_reason = Some(reason);
            next.cursor_step_id = None;
            next.gate = None;
        }
        ControlEvent::Succeed | ControlEvent::Cancel => {
            next.cursor_step_id = None;
            next.gate = None;
        }
        ControlEvent::Start => {}
    }
    Ok(next)
}

/// 领取运行时的版本推进。排队运行会进入 running；已经在运行的只更新租约版本。
pub fn note_lease(control: &RunControl) -> Result<RunControl, DomainError> {
    match control.status {
        RunStatus::Queued => apply_event(control, ControlEvent::Start, Some(control.version)),
        RunStatus::Running => {
            let mut next = control.clone();
            next.version += 1;
            Ok(next)
        }
        _ => Err(DomainError::Invariant(
            "only queued or running runs can be claimed",
        )),
    }
}

fn transition(status: RunStatus, event: &ControlEvent) -> Option<RunStatus> {
    use ControlEvent::*;
    use RunStatus::*;
    match (status, event) {
        (Queued, Start) => Some(Running),
        (Queued, Cancel) => Some(Cancelled),
        (Running, StepStarted { .. } | StepSucceeded) => Some(Running),
        (Running, OpenGate { .. }) => Some(WaitingApproval),
        (Running, Pause { .. }) => Some(Paused),
        (Running, Succeed) => Some(Succeeded),
        (Running, Fail { .. }) => Some(Failed),
        (Running, Cancel) => Some(Cancelled),
        (WaitingApproval, ResolveGate) => Some(Running),
        (WaitingApproval, Fail { .. }) => Some(Failed),
        (WaitingApproval, Cancel) => Some(Cancelled),
        (Paused, Resume) => Some(Running),
        (Paused, Cancel) => Some(Cancelled),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_claiming_a_run_that_waits_for_the_author() {
        let mut control = initial_control();
        control.status = RunStatus::WaitingApproval;
        assert!(note_lease(&control).is_err());
        control.status = RunStatus::Paused;
        assert!(note_lease(&control).is_err());
    }

    #[test]
    fn queued_claim_enters_running_and_bumps_version() {
        let next = note_lease(&initial_control()).unwrap();
        assert_eq!(next.status, RunStatus::Running);
        assert_eq!(next.version, 1);
    }

    #[test]
    fn cas_mismatch_does_not_change_status() {
        let control = initial_control();
        let err = apply_event(&control, ControlEvent::Start, Some(3)).unwrap_err();
        assert_eq!(
            err,
            DomainError::VersionConflict {
                expected: 3,
                actual: 0
            }
        );
    }

    #[test]
    fn approval_and_terminal_transitions_are_closed() {
        let running = apply_event(&initial_control(), ControlEvent::Start, Some(0)).unwrap();
        let waiting = apply_event(
            &running,
            ControlEvent::OpenGate {
                gate_id: "character_cast".into(),
                artifact_types: vec!["character_cast".into()],
            },
            Some(running.version),
        )
        .unwrap();
        assert_eq!(waiting.status, RunStatus::WaitingApproval);
        assert!(apply_event(&waiting, ControlEvent::Start, Some(waiting.version)).is_err());
        let resumed =
            apply_event(&waiting, ControlEvent::ResolveGate, Some(waiting.version)).unwrap();
        assert_eq!(resumed.status, RunStatus::Running);
        let done = apply_event(&resumed, ControlEvent::Succeed, Some(resumed.version)).unwrap();
        assert_eq!(done.status, RunStatus::Succeeded);
        assert!(apply_event(&done, ControlEvent::Cancel, Some(done.version)).is_err());
    }

    #[test]
    fn unknown_status_is_not_treated_as_success() {
        assert!(parse_status("completed").is_err());
        assert!(parse_status("waiting_gate").is_err());
    }
}
