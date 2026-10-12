#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttemptPhase {
    Started,
    RemoteStarted,
    CandidateSaved,
    Committed,
    Interrupted,
}

impl AttemptPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::RemoteStarted => "remote_started",
            Self::CandidateSaved => "candidate_saved",
            Self::Committed => "committed",
            Self::Interrupted => "interrupted",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "started" => Some(Self::Started),
            "remote_started" => Some(Self::RemoteStarted),
            "candidate_saved" => Some(Self::CandidateSaved),
            "committed" => Some(Self::Committed),
            "interrupted" => Some(Self::Interrupted),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResumeIntent {
    AcceptCandidate,
    RetryRemote,
}

impl ResumeIntent {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AcceptCandidate => "accept",
            Self::RetryRemote => "retry",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "accept" => Some(Self::AcceptCandidate),
            "retry" => Some(Self::RetryRemote),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttemptView {
    pub id: String,
    pub step_id: String,
    pub phase: AttemptPhase,
    pub candidate_id: Option<String>,
    pub request_id: Option<String>,
    pub resolution: Option<ResumeIntent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryDecision {
    FollowPlan,
    ResumeLocal {
        attempt_id: String,
        step_id: String,
    },
    FinalizeAccepted {
        attempt_id: String,
        step_id: String,
        candidate_id: Option<String>,
    },
    RetryStep {
        step_id: String,
    },
    HoldInterrupted {
        attempt_id: String,
        step_id: String,
        candidate_id: Option<String>,
    },
}

pub fn decide(attempts: &[AttemptView]) -> RecoveryDecision {
    let Some(latest) = attempts.last() else {
        return RecoveryDecision::FollowPlan;
    };
    if latest.phase == AttemptPhase::Committed {
        return RecoveryDecision::FollowPlan;
    }
    match latest.resolution {
        Some(ResumeIntent::AcceptCandidate) => RecoveryDecision::FinalizeAccepted {
            attempt_id: latest.id.clone(),
            step_id: latest.step_id.clone(),
            candidate_id: latest.candidate_id.clone(),
        },
        Some(ResumeIntent::RetryRemote) => RecoveryDecision::RetryStep {
            step_id: latest.step_id.clone(),
        },
        None => match latest.phase {
            AttemptPhase::Started => RecoveryDecision::ResumeLocal {
                attempt_id: latest.id.clone(),
                step_id: latest.step_id.clone(),
            },
            AttemptPhase::RemoteStarted
            | AttemptPhase::CandidateSaved
            | AttemptPhase::Interrupted => RecoveryDecision::HoldInterrupted {
                attempt_id: latest.id.clone(),
                step_id: latest.step_id.clone(),
                candidate_id: latest.candidate_id.clone(),
            },
            AttemptPhase::Committed => RecoveryDecision::FollowPlan,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attempt(phase: AttemptPhase, resolution: Option<ResumeIntent>) -> AttemptView {
        AttemptView {
            id: "a1".into(),
            step_id: "draft".into(),
            phase,
            candidate_id: Some("cand".into()),
            request_id: Some("req".into()),
            resolution,
        }
    }

    #[test]
    fn remote_dispatch_without_commit_requires_author_review() {
        let decision = decide(&[attempt(AttemptPhase::CandidateSaved, None)]);
        assert!(matches!(decision, RecoveryDecision::HoldInterrupted { .. }));
    }

    #[test]
    fn local_start_can_resume_without_a_new_remote_call() {
        let mut local = attempt(AttemptPhase::Started, None);
        local.request_id = None;
        assert!(matches!(
            decide(&[local]),
            RecoveryDecision::ResumeLocal { .. }
        ));
    }

    #[test]
    fn accepted_candidate_is_finalized_without_retry() {
        let decision = decide(&[attempt(
            AttemptPhase::Interrupted,
            Some(ResumeIntent::AcceptCandidate),
        )]);
        assert!(matches!(
            decision,
            RecoveryDecision::FinalizeAccepted { .. }
        ));
    }
}
