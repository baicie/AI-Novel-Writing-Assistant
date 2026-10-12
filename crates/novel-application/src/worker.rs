use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use novel_domain::{
    ChapterProtection, ControlEvent, PauseKind, RunPause, StopSignalKind, ensure_can_promote,
};
use novel_llm::{LlmProvider, LlmRequest, TokenUsage, redact_secrets};
use novel_workflow::{
    AttemptPhase, CHAPTER_PLAN_VERSION, NextAction, PRODUCTION_PLAN_VERSION, RecoveryDecision,
    StepScope, StepSpec, book_scope, chapter_scope, decide, find_step, next_action,
    production_plan,
};

use crate::dto::{GenerationEvent, event_from_stored};
use crate::error::AppError;
use crate::hash::sha256_hex;
use crate::hub::EventHub;
use crate::ports::{
    ClaimedRun, Clock, ControlWrite, FaultInjector, Ids, NewArtifact, NewAttempt, NewCandidate,
    NewInvocation, NovelRepository, PromoteOutcome, WorkflowRepository,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TickOutcome {
    Idle,
    Stopped,
    Saturated,
    Advanced { run_id: String, status: String },
    Interrupted { run_id: String },
    LostLease { run_id: String },
}

pub struct Worker {
    novels: Arc<dyn NovelRepository>,
    runs: Arc<dyn WorkflowRepository>,
    llm: Arc<dyn LlmProvider>,
    clock: Arc<dyn Clock>,
    ids: Arc<dyn Ids>,
    hub: Arc<EventHub>,
    faults: Arc<dyn FaultInjector>,
    gate: Arc<ConcurrencyGate>,
    running: Arc<AtomicBool>,
    owner: String,
    lease_ms: i64,
}

impl Worker {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        novels: Arc<dyn NovelRepository>,
        runs: Arc<dyn WorkflowRepository>,
        llm: Arc<dyn LlmProvider>,
        clock: Arc<dyn Clock>,
        ids: Arc<dyn Ids>,
        hub: Arc<EventHub>,
        faults: Arc<dyn FaultInjector>,
        owner: String,
        lease_ms: i64,
        concurrency: usize,
    ) -> Self {
        Self {
            novels,
            runs,
            llm,
            clock,
            ids,
            hub,
            faults,
            gate: Arc::new(ConcurrencyGate::new(concurrency)),
            running: Arc::new(AtomicBool::new(true)),
            owner,
            lease_ms,
        }
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    pub fn running_flag(&self) -> Arc<AtomicBool> {
        self.running.clone()
    }

    pub async fn tick(&self) -> Result<TickOutcome, AppError> {
        if !self.running.load(Ordering::SeqCst) {
            return Ok(TickOutcome::Stopped);
        }
        if !self.gate.try_acquire() {
            return Ok(TickOutcome::Saturated);
        }
        let _guard = GateGuard(self.gate.clone());
        let Some(claimed) = self
            .runs
            .claim_next(&self.owner, self.clock.now_ms(), self.lease_ms)
            .await?
        else {
            return Ok(TickOutcome::Idle);
        };
        self.advance(claimed).await
    }

    async fn advance(&self, claimed: ClaimedRun) -> Result<TickOutcome, AppError> {
        let attempts = self.runs.list_attempts(&claimed.run_id).await?;
        match decide(&attempts) {
            RecoveryDecision::HoldInterrupted { step_id, .. } => {
                self.pause(
                    &claimed,
                    StopSignalKind::ApprovalRequired,
                    "生成中断了，候选正文还在。请确认后继续，系统不会自动重新请求模型。",
                    Some(step_id),
                )
                .await
            }
            RecoveryDecision::FinalizeAccepted {
                attempt_id,
                candidate_id,
                ..
            } => {
                self.finalize_accepted(&claimed, attempt_id, candidate_id)
                    .await
            }
            RecoveryDecision::RetryStep { step_id } => {
                self.execute_step(&claimed, &step_id, None).await
            }
            RecoveryDecision::ResumeLocal {
                attempt_id,
                step_id,
            } => {
                self.execute_step(&claimed, &step_id, Some(attempt_id))
                    .await
            }
            RecoveryDecision::FollowPlan => self.follow_plan(&claimed).await,
        }
    }

    async fn follow_plan(&self, claimed: &ClaimedRun) -> Result<TickOutcome, AppError> {
        if claimed.contract.plan_version != PRODUCTION_PLAN_VERSION
            || claimed.contract.chapter_plan_version != CHAPTER_PLAN_VERSION
        {
            return self
                .pause(
                    claimed,
                    StopSignalKind::DataIntegrity,
                    "保存的创作计划版本无法继续，需要人工查看。",
                    None,
                )
                .await;
        }
        let artifacts = self.runs.list_artifacts(&claimed.novel_id).await?;
        match next_action(
            &production_plan(),
            &artifacts,
            &claimed.contract.target_chapter_id,
        ) {
            NextAction::Succeed => {
                self.transition(claimed, ControlEvent::Succeed, "run_completed", None, "{}")
                    .await
            }
            NextAction::Blocked { step_id } => {
                self.pause(
                    claimed,
                    StopSignalKind::DataIntegrity,
                    "创作步骤缺少前置结果，已停下等待查看。",
                    Some(step_id.to_string()),
                )
                .await
            }
            NextAction::OpenGate {
                step_id,
                artifact_type,
            } => {
                self.transition(
                    claimed,
                    ControlEvent::OpenGate {
                        gate_id: step_id.to_string(),
                        artifact_types: vec![artifact_type.to_string()],
                    },
                    "needs_approval",
                    Some(step_id.to_string()),
                    &serde_json::json!({ "stepId": step_id }).to_string(),
                )
                .await
            }
            NextAction::RunStep { step_id } => self.execute_step(claimed, step_id, None).await,
        }
    }

    async fn execute_step(
        &self,
        claimed: &ClaimedRun,
        step_id: &str,
        resume_attempt: Option<String>,
    ) -> Result<TickOutcome, AppError> {
        let spec = find_step(&production_plan(), step_id)
            .copied()
            .ok_or_else(|| AppError::integrity("创作步骤不在已冻结的计划里。"))?;
        match step_id {
            "draft" => self.execute_draft(claimed, resume_attempt).await,
            "commit" => self.execute_commit(claimed, &spec).await,
            _ => {
                let payload = if step_id == "consistency_review" {
                    serde_json::json!({ "issues": 0 }).to_string()
                } else {
                    serde_json::json!({ "ok": true, "step": step_id }).to_string()
                };
                let status = if spec.gateable { "draft" } else { "confirmed" };
                self.finish_step(claimed, &spec, status, false, &payload, None, None)
                    .await
            }
        }
    }

    async fn execute_draft(
        &self,
        claimed: &ClaimedRun,
        resume_attempt: Option<String>,
    ) -> Result<TickOutcome, AppError> {
        let chapter = self
            .novels
            .get_chapter(&claimed.novel_id, &claimed.contract.target_chapter_id)
            .await?;
        let protection = ChapterProtection::parse(&chapter.protection)
            .map_err(|_| AppError::integrity("章节保护状态无法识别。"))?;
        if ensure_can_promote(protection).is_err() {
            return self.finish_protected_draft(claimed).await;
        }
        let attempt_id = if let Some(attempt_id) = resume_attempt {
            attempt_id
        } else {
            let attempt_id = self.ids.new_id();
            self.runs
                .start_attempt(NewAttempt {
                    id: attempt_id.clone(),
                    run_id: claimed.run_id.clone(),
                    step_id: "draft".into(),
                    control_version: claimed.control.version,
                    created_at: self.clock.now_ms(),
                })
                .await?;
            attempt_id
        };
        let request_id = self.ids.new_id();
        if !self
            .runs
            .mark_remote_started(&attempt_id, &request_id, self.clock.now_ms())
            .await?
        {
            return Ok(TickOutcome::LostLease {
                run_id: claimed.run_id.clone(),
            });
        }
        let prompt = draft_prompt(&claimed.contract, &chapter.title)?;
        let summary = summarize(&prompt);
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let hub = self.hub.clone();
        let run_for_tokens = claimed.run_id.clone();
        let forward = tokio::spawn(async move {
            while let Some(text) = receiver.recv().await {
                hub.emit(GenerationEvent::TokenDelta {
                    run_id: run_for_tokens.clone(),
                    step_id: "draft".into(),
                    text,
                });
            }
        });
        let started = self.clock.now_ms();
        let generated = self
            .llm
            .generate(
                LlmRequest {
                    request_id: request_id.clone(),
                    model: claimed.contract.model.clone(),
                    prompt,
                    endpoint: claimed.contract.endpoint.clone(),
                    api_key: String::new(),
                    allow_localhost_http: claimed.contract.allow_localhost_http,
                },
                sender,
                tokio_util::sync::CancellationToken::new(),
            )
            .await;
        let _ = forward.await;
        let response = match generated {
            Ok(response) => response,
            Err(error) => {
                self.runs
                    .mark_attempt(
                        &attempt_id,
                        AttemptPhase::Interrupted.as_str(),
                        None,
                        self.clock.now_ms(),
                    )
                    .await?;
                let signal = match error {
                    novel_llm::LlmError::RateLimit => StopSignalKind::Budget,
                    novel_llm::LlmError::Cancelled => StopSignalKind::ApprovalRequired,
                    _ => StopSignalKind::ProviderUnavailable,
                };
                let reason = error.to_string();
                return self
                    .pause(claimed, signal, &reason, Some("draft".into()))
                    .await;
            }
        };
        let candidate_id = self.ids.new_id();
        self.runs
            .save_candidate(NewCandidate {
                id: candidate_id.clone(),
                chapter_id: chapter.id,
                run_id: claimed.run_id.clone(),
                step_id: "draft".into(),
                content: response.text.clone(),
                content_sha256: sha256_hex(response.text.as_bytes()),
                now_ms: self.clock.now_ms(),
            })
            .await?;
        self.runs
            .mark_attempt(
                &attempt_id,
                AttemptPhase::CandidateSaved.as_str(),
                Some(&candidate_id),
                self.clock.now_ms(),
            )
            .await?;
        self.runs
            .record_invocation(invocation(
                self,
                claimed,
                &request_id,
                &summary,
                started,
                &response.usage,
            ))
            .await?;
        if self.faults.fail_after_candidate("draft").await {
            return Ok(TickOutcome::Interrupted {
                run_id: claimed.run_id.clone(),
            });
        }
        let payload = serde_json::json!({ "candidateId": candidate_id }).to_string();
        let spec = find_step(&production_plan(), "draft")
            .copied()
            .expect("draft step");
        self.finish_step(
            claimed,
            &spec,
            "draft",
            false,
            &payload,
            None,
            Some(attempt_id),
        )
        .await
    }

    async fn finish_protected_draft(&self, claimed: &ClaimedRun) -> Result<TickOutcome, AppError> {
        let payload = serde_json::json!({ "skipped": "protected" }).to_string();
        let spec = find_step(&production_plan(), "draft")
            .copied()
            .expect("draft step");
        let artifacts = self.runs.list_artifacts(&claimed.novel_id).await?;
        let protected = artifacts.iter().any(|artifact| {
            artifact.artifact_type == "chapter_draft"
                && artifact.scope == chapter_scope(&claimed.contract.target_chapter_id)
                && artifact.protected_user_content
        });
        if protected {
            self.transition(
                claimed,
                ControlEvent::StepSucceeded,
                "step_succeeded",
                Some("draft".into()),
                &payload,
            )
            .await
        } else {
            self.finish_step(claimed, &spec, "user_edited", true, &payload, None, None)
                .await
        }
    }

    async fn finalize_accepted(
        &self,
        claimed: &ClaimedRun,
        attempt_id: String,
        candidate_id: Option<String>,
    ) -> Result<TickOutcome, AppError> {
        let artifacts = self.runs.list_artifacts(&claimed.novel_id).await?;
        let protected = artifacts.iter().any(|artifact| {
            artifact.artifact_type == "chapter_draft"
                && artifact.scope == chapter_scope(&claimed.contract.target_chapter_id)
                && (artifact.protected_user_content
                    || artifact.status == novel_domain::ArtifactStatus::UserEdited)
        });
        let payload =
            serde_json::json!({ "candidateId": candidate_id, "accepted": true }).to_string();
        if protected {
            self.transition_attempt(
                claimed,
                ControlEvent::StepSucceeded,
                "step_succeeded",
                Some("draft".into()),
                &payload,
                Some(attempt_id),
            )
            .await
        } else {
            let spec = find_step(&production_plan(), "draft")
                .copied()
                .expect("draft step");
            self.finish_step(
                claimed,
                &spec,
                "draft",
                false,
                &payload,
                None,
                Some(attempt_id),
            )
            .await
        }
    }

    async fn execute_commit(
        &self,
        claimed: &ClaimedRun,
        spec: &StepSpec,
    ) -> Result<TickOutcome, AppError> {
        let candidate = self.runs.latest_candidate(&claimed.run_id).await?;
        let outcome = if let Some(candidate) = candidate {
            self.novels
                .promote_candidate(
                    &claimed.contract.target_chapter_id,
                    &candidate.id,
                    &self.ids.new_id(),
                    self.clock.now_ms(),
                )
                .await?
        } else {
            PromoteOutcome::MissingCandidate
        };
        let payload = match &outcome {
            PromoteOutcome::Promoted { version } => {
                serde_json::json!({ "promoted": true, "version": version })
            }
            PromoteOutcome::Blocked => {
                serde_json::json!({ "promoted": false, "reason": "protected" })
            }
            PromoteOutcome::MissingCandidate => {
                serde_json::json!({ "promoted": false, "reason": "missing" })
            }
        };
        self.finish_step(
            claimed,
            spec,
            "confirmed",
            false,
            &payload.to_string(),
            None,
            None,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn finish_step(
        &self,
        claimed: &ClaimedRun,
        spec: &StepSpec,
        status: &str,
        protected: bool,
        payload: &str,
        invocation: Option<NewInvocation>,
        attempt_id: Option<String>,
    ) -> Result<TickOutcome, AppError> {
        let scope = match spec.scope {
            StepScope::Book => book_scope().to_string(),
            StepScope::Chapter => chapter_scope(&claimed.contract.target_chapter_id),
        };
        let artifact = NewArtifact {
            id: self.ids.new_id(),
            novel_id: claimed.novel_id.clone(),
            run_id: Some(claimed.run_id.clone()),
            artifact_type: spec.produces.to_string(),
            scope,
            status: status.to_string(),
            protected_user_content: protected,
            payload_json: payload.to_string(),
            content_sha256: sha256_hex(payload.as_bytes()),
            created_at: self.clock.now_ms(),
        };
        let (event, kind) = if spec.gateable {
            (
                ControlEvent::OpenGate {
                    gate_id: spec.id.to_string(),
                    artifact_types: vec![spec.produces.to_string()],
                },
                "needs_approval",
            )
        } else {
            (ControlEvent::StepSucceeded, "step_succeeded")
        };
        self.write(
            claimed,
            event,
            kind,
            Some(spec.id.to_string()),
            payload,
            Some(artifact),
            invocation,
            attempt_id,
        )
        .await
    }

    async fn pause(
        &self,
        claimed: &ClaimedRun,
        signal: StopSignalKind,
        reason: &str,
        step_id: Option<String>,
    ) -> Result<TickOutcome, AppError> {
        let pause = RunPause {
            kind: match signal {
                StopSignalKind::MigrationReviewRequired => PauseKind::MigrationReview,
                StopSignalKind::Replan => PauseKind::Replan,
                StopSignalKind::DataIntegrity | StopSignalKind::Safety => PauseKind::Safety,
                _ => PauseKind::ManualRecovery,
            },
            signal,
            reason: reason.to_string(),
        };
        self.write(
            claimed,
            ControlEvent::Pause { pause },
            "run_paused",
            step_id,
            &serde_json::json!({ "reason": reason }).to_string(),
            None,
            None,
            None,
        )
        .await
    }

    async fn transition(
        &self,
        claimed: &ClaimedRun,
        event: ControlEvent,
        kind: &str,
        step_id: Option<String>,
        payload: &str,
    ) -> Result<TickOutcome, AppError> {
        self.write(claimed, event, kind, step_id, payload, None, None, None)
            .await
    }

    async fn transition_attempt(
        &self,
        claimed: &ClaimedRun,
        event: ControlEvent,
        kind: &str,
        step_id: Option<String>,
        payload: &str,
        attempt_id: Option<String>,
    ) -> Result<TickOutcome, AppError> {
        self.write(
            claimed, event, kind, step_id, payload, None, None, attempt_id,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn write(
        &self,
        claimed: &ClaimedRun,
        event: ControlEvent,
        kind: &str,
        step_id: Option<String>,
        payload: &str,
        artifact: Option<NewArtifact>,
        invocation: Option<NewInvocation>,
        attempt_id: Option<String>,
    ) -> Result<TickOutcome, AppError> {
        let occurred_at = self.clock.now_ms();
        let attempt_phase = attempt_id
            .as_ref()
            .map(|_| AttemptPhase::Committed.as_str().to_string());
        let committed = self
            .runs
            .apply_control(ControlWrite {
                run_id: claimed.run_id.clone(),
                expected_version: claimed.control.version,
                event,
                artifact,
                event_kind: kind.to_string(),
                event_step_id: step_id.clone(),
                event_payload: payload.to_string(),
                occurred_at,
                invocation,
                attempt_id,
                attempt_phase,
                clear_lease: true,
            })
            .await?;
        if let Some(event) = event_from_stored(
            &claimed.run_id,
            committed.seq,
            kind,
            step_id,
            payload,
            occurred_at,
        ) {
            self.hub.emit(event);
        }
        tracing::info!(run_id = %claimed.run_id, status = committed.control.status.as_str(), "run advanced");
        Ok(TickOutcome::Advanced {
            run_id: claimed.run_id.clone(),
            status: committed.control.status.as_str().to_string(),
        })
    }
}

fn draft_prompt(contract: &crate::dto::RunContract, title: &str) -> Result<String, AppError> {
    use std::collections::BTreeMap;
    let version = contract
        .prompt_versions
        .get("chapter_draft")
        .map(String::as_str)
        .unwrap_or("");
    let asset = novel_prompt::resolve("chapter_draft", version)
        .map_err(|_| AppError::validation("正文提示词版本不存在"))?;
    let mut vars = BTreeMap::new();
    vars.insert("brief".into(), title.to_string());
    novel_prompt::render(asset.template, &vars)
        .map_err(|_| AppError::validation("正文提示词缺少章节目标"))
}

fn summarize(prompt: &str) -> String {
    redact_secrets(prompt).chars().take(80).collect()
}

fn invocation(
    worker: &Worker,
    claimed: &ClaimedRun,
    request_id: &str,
    summary: &str,
    started: i64,
    usage: &TokenUsage,
) -> NewInvocation {
    NewInvocation {
        id: worker.ids.new_id(),
        request_id: request_id.to_string(),
        run_id: claimed.run_id.clone(),
        step_id: "draft".into(),
        provider: claimed.contract.provider.clone(),
        model: claimed.contract.model.clone(),
        input_summary: summary.to_string(),
        started_at: started,
        finished_at: worker.clock.now_ms(),
        prompt_tokens: usage
            .prompt_tokens
            .map(|value| i64::try_from(value).unwrap_or(i64::MAX)),
        completion_tokens: usage
            .completion_tokens
            .map(|value| i64::try_from(value).unwrap_or(i64::MAX)),
        status: "succeeded".into(),
    }
}

pub struct ConcurrencyGate {
    limit: usize,
    current: AtomicUsize,
}

impl ConcurrencyGate {
    pub fn new(limit: usize) -> Self {
        Self {
            limit,
            current: AtomicUsize::new(0),
        }
    }

    pub fn try_acquire(&self) -> bool {
        loop {
            let current = self.current.load(Ordering::SeqCst);
            if current >= self.limit {
                return false;
            }
            if self
                .current
                .compare_exchange(current, current + 1, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                return true;
            }
        }
    }

    pub fn release(&self) {
        self.current.fetch_sub(1, Ordering::SeqCst);
    }
}

struct GateGuard(Arc<ConcurrencyGate>);

impl Drop for GateGuard {
    fn drop(&mut self) {
        self.0.release();
    }
}
