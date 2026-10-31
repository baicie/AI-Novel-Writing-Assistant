use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use novel_domain::{
    ChapterProtection, ControlEvent, PauseKind, RunPause, StopSignalKind, ensure_can_promote,
};
use novel_llm::{LlmProvider, LlmRequest, TokenUsage, redact_secrets};
use novel_workflow::{
    AttemptPhase, CHAPTER_PLAN_VERSION, NextAction, RecoveryDecision, StepScope, StepSpec,
    book_scope, chapter_scope, decide, find_step, next_action, plan_for_version, production_plan,
};
use tokio_util::sync::CancellationToken;

use crate::context::{DEFAULT_OUTPUT_RESERVE, assemble_director_context};
use crate::dto::{GenerationEvent, event_from_stored};
use crate::error::AppError;
use crate::hash::sha256_hex;
use crate::hub::EventHub;
use crate::ports::{
    ClaimedRun, Clock, ControlWrite, FaultInjector, Ids, LeaseFence, ModelSecrets, NewArtifact,
    NewAttempt, NewCandidate, NewInvocation, NewQualityDebt, NewReviewOccurrence, NovelRepository,
    PromoteOutcome, WorkflowRepository,
};

mod worker_validation;

pub(super) use worker_validation::{
    context_payload, latest_payload, truncate_chars, validate_chapter_plan_references,
    validate_structured_output,
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
    secrets: Arc<dyn ModelSecrets>,
    cancels: Arc<Mutex<HashMap<String, CancellationToken>>>,
    gate: Arc<ConcurrencyGate>,
    running: Arc<AtomicBool>,
    owner: String,
    lease_ms: i64,
    renewal_enabled: bool,
}

impl Clone for Worker {
    fn clone(&self) -> Self {
        Self {
            novels: self.novels.clone(),
            runs: self.runs.clone(),
            llm: self.llm.clone(),
            clock: self.clock.clone(),
            ids: self.ids.clone(),
            hub: self.hub.clone(),
            faults: self.faults.clone(),
            secrets: self.secrets.clone(),
            cancels: self.cancels.clone(),
            gate: self.gate.clone(),
            running: self.running.clone(),
            owner: self.owner.clone(),
            lease_ms: self.lease_ms,
            renewal_enabled: self.renewal_enabled,
        }
    }
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
        secrets: Arc<dyn ModelSecrets>,
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
            secrets,
            cancels: Arc::new(Mutex::new(HashMap::new())),
            gate: Arc::new(ConcurrencyGate::new(concurrency)),
            running: Arc::new(AtomicBool::new(true)),
            owner,
            lease_ms,
            renewal_enabled: true,
        }
    }

    /// Test-only hook for deterministic lease-expiry scenarios.
    pub fn disable_lease_renewal(&mut self) {
        self.renewal_enabled = false;
    }

    pub fn llm(&self) -> Arc<dyn LlmProvider> {
        self.llm.clone()
    }

    pub fn clock(&self) -> Arc<dyn Clock> {
        self.clock.clone()
    }

    pub fn ids(&self) -> Arc<dyn Ids> {
        self.ids.clone()
    }

    pub fn secrets(&self) -> Arc<dyn ModelSecrets> {
        self.secrets.clone()
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    pub fn cancel_generation(&self, run_id: &str) {
        if let Some(token) = self.cancels.lock().expect("cancel map").get(run_id) {
            token.cancel();
        }
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
        let renew_cancel = CancellationToken::new();
        let renew_handle = if self.lease_ms > 0 && self.renewal_enabled {
            let runs = self.runs.clone();
            let clock = self.clock.clone();
            let fence = claimed.fence.clone();
            let run_id = claimed.run_id.clone();
            let lease_ms = self.lease_ms;
            let token = renew_cancel.clone();
            Some(tokio::spawn(async move {
                let interval = Duration::from_millis((lease_ms / 3).max(10) as u64);
                loop {
                    tokio::select! {
                        _ = token.cancelled() => break,
                        _ = tokio::time::sleep(interval) => {}
                    }
                    let now = clock.now_ms();
                    if !matches!(
                        runs.renew_lease(&fence, &run_id, now, lease_ms).await,
                        Ok(true)
                    ) {
                        break;
                    }
                }
            }))
        } else {
            None
        };
        let result = self.advance(claimed).await;
        renew_cancel.cancel();
        if let Some(handle) = renew_handle {
            let _ = handle.await;
        }
        result
    }

    async fn advance(&self, claimed: ClaimedRun) -> Result<TickOutcome, AppError> {
        let attempts = self.runs.list_attempts(&claimed.run_id).await?;
        if let Some(outcome) = self
            .try_resume_repair_candidate(&claimed, &attempts)
            .await?
        {
            return Ok(outcome);
        }
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
        let Some(plan) = plan_for_version(&claimed.contract.plan_version) else {
            return self
                .pause(
                    claimed,
                    StopSignalKind::DataIntegrity,
                    "保存的创作计划版本无法继续，需要人工查看。",
                    None,
                )
                .await;
        };
        if claimed.contract.chapter_plan_version != CHAPTER_PLAN_VERSION {
            return self
                .pause(
                    claimed,
                    StopSignalKind::DataIntegrity,
                    "保存的章节计划版本无法继续，需要人工查看。",
                    None,
                )
                .await;
        }
        let artifacts = self.runs.list_artifacts(&claimed.novel_id).await?;
        match next_action(&plan, &artifacts, &claimed.contract.target_chapter_id) {
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
        let plan = plan_for_version(&claimed.contract.plan_version)
            .ok_or_else(|| AppError::integrity("创作计划版本不存在。"))?;
        let spec = find_step(&plan, step_id)
            .copied()
            .ok_or_else(|| AppError::integrity("创作步骤不在已冻结的计划里。"))?;
        match step_id {
            "draft" => self.execute_draft(claimed, resume_attempt).await,
            "commit" => self.execute_commit(claimed, &spec).await,
            "story_macro" | "character_cast" | "volume_strategy" | "chapter_list"
            | "consistency_review" => execute_structured(self, claimed, &spec, step_id).await,
            "context" => execute_context(self, claimed, &spec).await,
            "bounded_repair"
                if claimed.contract.plan_version == novel_workflow::PRODUCTION_PLAN_VERSION_V1 =>
            {
                self.pause(
                    claimed,
                    StopSignalKind::DataIntegrity,
                    "旧计划版本仍停在人工审查点；不会静默迁移到新的自动修复规则。",
                    Some(step_id.into()),
                )
                .await
            }
            "bounded_repair" => self.execute_bounded_repair(claimed, &spec).await,
            _ => {
                // The approval artifact is created by the gate transition; other
                // non-business plumbing steps remain explicit placeholders until
                // their original contract is ported.
                let payload =
                    serde_json::json!({ "step": step_id, "gate": spec.gateable }).to_string();
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
                .start_attempt_fenced(
                    &claimed.fence,
                    NewAttempt {
                        id: attempt_id.clone(),
                        run_id: claimed.run_id.clone(),
                        step_id: "draft".into(),
                        control_version: claimed.control.version,
                        created_at: self.clock.now_ms(),
                    },
                    self.clock.now_ms(),
                )
                .await?;
            attempt_id
        };
        let request_id = self.ids.new_id();
        if !self
            .runs
            .mark_remote_started_fenced(
                &claimed.fence,
                &claimed.run_id,
                &attempt_id,
                &request_id,
                self.clock.now_ms(),
            )
            .await?
        {
            return Ok(TickOutcome::LostLease {
                run_id: claimed.run_id.clone(),
            });
        }
        let prompt = draft_prompt(self, claimed, &chapter.title).await?;
        let summary = summarize(&prompt);
        let route = claimed.contract.effective_route("writer");
        let credential_ref = route
            .credential_ref_checked(
                &claimed.contract.provider,
                claimed.contract.credential_ref.as_deref(),
            )
            .map_err(|error| AppError::validation(error.message.clone()))?;
        let api_key = self
            .secrets
            .api_key(credential_ref)
            .await?
            .unwrap_or_default();
        // 没有密钥时只允许本机 Ollama；远程来源缺密钥必须在这里被挡住，不能带着空密钥发出去。
        crate::provider_service::ensure_credential(
            route.protocol(claimed.contract.protocol.as_deref()),
            route.endpoint(&claimed.contract.endpoint),
            &api_key,
        )
        .map_err(|error| AppError::validation(error.message.clone()))?;
        let cancel = CancellationToken::new();
        self.cancels
            .lock()
            .expect("cancel map")
            .insert(claimed.run_id.clone(), cancel.clone());
        let _cancel_guard = CancelRegistration {
            run_id: claimed.run_id.clone(),
            cancels: self.cancels.clone(),
        };
        let (sender, mut receiver) = tokio::sync::mpsc::channel(64);
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
            .generate_with_protocol(
                LlmRequest {
                    request_id: request_id.clone(),
                    model: route.model_id(&claimed.contract.model).to_string(),
                    prompt,
                    endpoint: route.endpoint(&claimed.contract.endpoint).to_string(),
                    api_key,
                    allow_localhost_http: route
                        .allow_localhost_http(claimed.contract.allow_localhost_http),
                    max_output_tokens: claimed.contract.token_budget,
                },
                route.protocol(claimed.contract.protocol.as_deref()),
                route.reasoning_effort(claimed.contract.reasoning_effort.as_deref()),
                sender,
                cancel,
            )
            .await;
        let _ = forward.await;
        let response = match generated {
            Ok(response) => response,
            Err(error) => {
                self.runs
                    .mark_attempt_fenced(
                        &claimed.fence,
                        &claimed.run_id,
                        &attempt_id,
                        AttemptPhase::Interrupted.as_str(),
                        None,
                        self.clock.now_ms(),
                    )
                    .await?;
                if matches!(error, novel_llm::LlmError::Cancelled) {
                    let reason = error.to_string();
                    return self
                        .write(
                            claimed,
                            ControlEvent::Cancel,
                            "run_cancelled",
                            Some("draft".into()),
                            &serde_json::json!({ "reason": reason }).to_string(),
                            None,
                            None,
                            None,
                        )
                        .await;
                }
                let signal = match error {
                    novel_llm::LlmError::RateLimit => StopSignalKind::Budget,
                    _ => StopSignalKind::ProviderUnavailable,
                };
                let reason = error.to_string();
                return self
                    .pause(claimed, signal, &reason, Some("draft".into()))
                    .await;
            }
        };
        let candidate_id = self.ids.new_id();
        let now = self.clock.now_ms();
        self.runs
            .save_candidate_checkpoint(
                &claimed.fence,
                NewCandidate {
                    id: candidate_id.clone(),
                    chapter_id: chapter.id,
                    run_id: claimed.run_id.clone(),
                    step_id: "draft".into(),
                    content: response.text.clone(),
                    content_sha256: sha256_hex(response.text.as_bytes()),
                    now_ms: now,
                },
                &attempt_id,
                invocation(
                    self,
                    claimed,
                    "draft",
                    &request_id,
                    &summary,
                    started,
                    &response.usage,
                ),
                now,
            )
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

    async fn execute_bounded_repair(
        &self,
        claimed: &ClaimedRun,
        spec: &StepSpec,
    ) -> Result<TickOutcome, AppError> {
        let artifacts = self.runs.list_artifacts(&claimed.novel_id).await?;
        let scope = chapter_scope(&claimed.contract.target_chapter_id);
        let review = latest_payload(&artifacts, "consistency_review", &scope)?;
        let issues = review
            .get("issueDetails")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        if issues.is_empty() {
            return self
                .finish_step(
                    claimed,
                    spec,
                    "confirmed",
                    false,
                    &serde_json::json!({ "status": "no_issues" }).to_string(),
                    None,
                    None,
                )
                .await;
        }
        let repair_budget = match claimed.contract.plan_version.as_str() {
            novel_workflow::PRODUCTION_PLAN_VERSION => claimed.contract.repair_budget,
            novel_workflow::PRODUCTION_PLAN_VERSION_V2 => claimed.contract.max_automatic_retries,
            _ => 0,
        };
        if repair_budget == 0 {
            return self
                .pause(
                    claimed,
                    StopSignalKind::Safety,
                    "一致性审校发现了问题，但冻结的修复预算为 0。已停在人工审查点，正文不会被自动提升。",
                    Some("bounded_repair".into()),
                )
                .await;
        }
        let candidate = self
            .runs
            .latest_candidate(&claimed.run_id)
            .await?
            .ok_or_else(|| AppError::integrity("修复缺少候选正文。"))?;
        let context = latest_payload(&artifacts, "context", &scope)?;
        let context_text = context
            .get("promptText")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| AppError::integrity("修复缺少上下文。"))?;
        let repair_asset = novel_prompt::resolve("chapter_repair", "chapter-repair-v1")
            .map_err(|_| AppError::validation("修订提示词版本不存在"))?;
        let mut vars = BTreeMap::new();
        vars.insert("context".into(), context_text.to_string());
        vars.insert(
            "issues".into(),
            serde_json::to_string(&issues)
                .map_err(|_| AppError::integrity("审校问题无法序列化"))?,
        );
        vars.insert("candidate".into(), candidate.content.clone());
        let prompt = novel_prompt::render(repair_asset.template, &vars)
            .map_err(|_| AppError::validation("修订提示词缺少上下文"))?;
        let summary = summarize(&prompt);
        let attempt_id = self.ids.new_id();
        self.runs
            .start_attempt_fenced(
                &claimed.fence,
                NewAttempt {
                    id: attempt_id.clone(),
                    run_id: claimed.run_id.clone(),
                    step_id: "bounded_repair".into(),
                    control_version: claimed.control.version,
                    created_at: self.clock.now_ms(),
                },
                self.clock.now_ms(),
            )
            .await?;
        let request_id = self.ids.new_id();
        if !self
            .runs
            .mark_remote_started_fenced(
                &claimed.fence,
                &claimed.run_id,
                &attempt_id,
                &request_id,
                self.clock.now_ms(),
            )
            .await?
        {
            return Ok(TickOutcome::LostLease {
                run_id: claimed.run_id.clone(),
            });
        }
        let route = claimed.contract.effective_route("repair");
        let credential_ref = route
            .credential_ref_checked(
                &claimed.contract.provider,
                claimed.contract.credential_ref.as_deref(),
            )
            .map_err(|error| AppError::validation(error.message.clone()))?;
        let api_key = self
            .secrets
            .api_key(credential_ref)
            .await?
            .unwrap_or_default();
        // 没有密钥时只允许本机 Ollama；远程来源缺密钥不能带着空密钥发出去。
        crate::provider_service::ensure_credential(
            route.protocol(claimed.contract.protocol.as_deref()),
            route.endpoint(&claimed.contract.endpoint),
            &api_key,
        )
        .map_err(|error| AppError::validation(error.message.clone()))?;
        let cancel = CancellationToken::new();
        self.cancels
            .lock()
            .expect("cancel map")
            .insert(claimed.run_id.clone(), cancel.clone());
        let _cancel_guard = CancelRegistration {
            run_id: claimed.run_id.clone(),
            cancels: self.cancels.clone(),
        };
        let (sender, mut receiver) = tokio::sync::mpsc::channel::<String>(64);
        let collect = tokio::spawn(async move {
            let mut text = String::new();
            while let Some(delta) = receiver.recv().await {
                text.push_str(&delta);
            }
            text
        });
        let started = self.clock.now_ms();
        let generated = self
            .llm
            .generate_with_protocol(
                LlmRequest {
                    request_id: request_id.clone(),
                    model: route.model_id(&claimed.contract.model).to_string(),
                    prompt,
                    endpoint: route.endpoint(&claimed.contract.endpoint).to_string(),
                    api_key,
                    allow_localhost_http: route
                        .allow_localhost_http(claimed.contract.allow_localhost_http),
                    max_output_tokens: claimed.contract.token_budget,
                },
                route.protocol(claimed.contract.protocol.as_deref()),
                route.reasoning_effort(claimed.contract.reasoning_effort.as_deref()),
                sender,
                cancel,
            )
            .await;
        let raw = collect.await.unwrap_or_default();
        let response = match generated {
            Ok(response) => response,
            Err(error) => {
                self.runs
                    .mark_attempt_fenced(
                        &claimed.fence,
                        &claimed.run_id,
                        &attempt_id,
                        AttemptPhase::Interrupted.as_str(),
                        None,
                        self.clock.now_ms(),
                    )
                    .await?;
                if matches!(error, novel_llm::LlmError::Cancelled) {
                    return self
                        .write(
                            claimed,
                            ControlEvent::Cancel,
                            "run_cancelled",
                            Some("bounded_repair".into()),
                            &serde_json::json!({ "reason": error.to_string() }).to_string(),
                            None,
                            None,
                            None,
                        )
                        .await;
                }
                return self
                    .pause(
                        claimed,
                        StopSignalKind::ProviderUnavailable,
                        &error.to_string(),
                        Some("bounded_repair".into()),
                    )
                    .await;
            }
        };
        let repaired = if raw.trim().is_empty() {
            response.text.clone()
        } else {
            raw
        };
        if repaired.trim().is_empty() {
            return self
                .pause(
                    claimed,
                    StopSignalKind::DataIntegrity,
                    "修复模型返回了空正文，未生成修复候选。",
                    Some("bounded_repair".into()),
                )
                .await;
        }
        let repaired_candidate_id = self.ids.new_id();
        let now = self.clock.now_ms();
        self.runs
            .save_candidate_checkpoint(
                &claimed.fence,
                NewCandidate {
                    id: repaired_candidate_id.clone(),
                    chapter_id: claimed.contract.target_chapter_id.clone(),
                    run_id: claimed.run_id.clone(),
                    step_id: "bounded_repair".into(),
                    content: repaired.clone(),
                    content_sha256: sha256_hex(repaired.as_bytes()),
                    now_ms: now,
                },
                &attempt_id,
                invocation(
                    self,
                    claimed,
                    "bounded_repair",
                    &request_id,
                    &summary,
                    started,
                    &response.usage,
                ),
                now,
            )
            .await?;
        self.faults.after_repair_candidate_persisted().await?;
        self.finish_repair_and_rereview(
            claimed,
            spec,
            attempt_id,
            &candidate.id,
            &repaired_candidate_id,
        )
        .await
    }

    /// Complete a repair whose candidate and invocation are already durable. The
    /// provider call and budget are never replayed on this path.
    async fn finish_repair_and_rereview(
        &self,
        claimed: &ClaimedRun,
        spec: &StepSpec,
        attempt_id: String,
        original_candidate_id: &str,
        repaired_candidate_id: &str,
    ) -> Result<TickOutcome, AppError> {
        let scope = chapter_scope(&claimed.contract.target_chapter_id);
        self.finish_step(
            claimed,
            spec,
            "confirmed",
            false,
            &serde_json::json!({
                "status": "repair_candidate_created",
                "originalCandidateId": original_candidate_id,
                "repairedCandidateId": repaired_candidate_id,
            })
            .to_string(),
            None,
            Some(attempt_id),
        )
        .await?;
        let control = self.runs.run_control(&claimed.run_id).await?;
        let fresh = ClaimedRun {
            run_id: claimed.run_id.clone(),
            novel_id: claimed.novel_id.clone(),
            contract: claimed.contract.clone(),
            fence: LeaseFence {
                owner: claimed.fence.owner.clone(),
                control_version: control.version,
            },
            control,
        };
        let review_spec = find_step(&production_plan(), "consistency_review")
            .copied()
            .expect("review step");
        let outcome = execute_structured(self, &fresh, &review_spec, "consistency_review").await?;
        if !matches!(outcome, TickOutcome::Advanced { .. }) {
            return Ok(outcome);
        }
        let after = self.runs.list_artifacts(&claimed.novel_id).await?;
        let latest_review = after
            .iter()
            .filter(|artifact| {
                artifact.artifact_type == "consistency_review" && artifact.scope == scope
            })
            .max_by_key(|artifact| artifact.version);
        let remaining = latest_review
            .and_then(|artifact| {
                serde_json::from_str::<serde_json::Value>(&artifact.payload_json).ok()
            })
            .and_then(|payload| payload.get("issues").and_then(serde_json::Value::as_i64))
            .unwrap_or(0);
        if remaining > 0 {
            let control = self.runs.run_control(&claimed.run_id).await?;
            let fresh = ClaimedRun {
                run_id: claimed.run_id.clone(),
                novel_id: claimed.novel_id.clone(),
                contract: claimed.contract.clone(),
                fence: LeaseFence {
                    owner: claimed.fence.owner.clone(),
                    control_version: control.version,
                },
                control,
            };
            return self
                .pause_safety(
                    &fresh,
                    "有界修复后仍有一致性问题，已停在上限并保留候选，等待人工决策。",
                    Some("bounded_repair".into()),
                )
                .await;
        }
        Ok(outcome)
    }

    /// Resume only when the repair result is fully durable and belongs to the
    /// current v3 contract. RemoteStarted and legacy plans keep the safe hold.
    async fn try_resume_repair_candidate(
        &self,
        claimed: &ClaimedRun,
        attempts: &[novel_workflow::AttemptView],
    ) -> Result<Option<TickOutcome>, AppError> {
        if claimed.contract.plan_version != novel_workflow::PRODUCTION_PLAN_VERSION
            || claimed.contract.repair_budget == 0
        {
            return Ok(None);
        }
        let Some(attempt) = attempts.last() else {
            return Ok(None);
        };
        if attempt.step_id != "bounded_repair" || attempt.phase != AttemptPhase::CandidateSaved {
            return Ok(None);
        }
        let (Some(candidate_id), Some(request_id)) = (
            attempt.candidate_id.as_deref(),
            attempt.request_id.as_deref(),
        ) else {
            return Ok(None);
        };
        if claimed
            .control
            .cursor_step_id
            .as_deref()
            .is_some_and(|step| step != "bounded_repair")
        {
            return Ok(None);
        }
        let Some(latest) = self.runs.latest_candidate(&claimed.run_id).await? else {
            return Ok(None);
        };
        if latest.id != candidate_id {
            return Ok(None);
        }
        if sha256_hex(latest.content.as_bytes()) != latest.content_sha256 {
            return self
                .pause(
                    claimed,
                    StopSignalKind::DataIntegrity,
                    "修复候选正文与保存的哈希不一致，未自动续跑。",
                    Some("bounded_repair".into()),
                )
                .await
                .map(Some);
        }
        if !self
            .runs
            .has_invocation(&claimed.run_id, "bounded_repair", request_id)
            .await?
        {
            return Ok(None);
        }
        let artifacts = self.runs.list_artifacts(&claimed.novel_id).await?;
        let scope = chapter_scope(&claimed.contract.target_chapter_id);
        if artifacts
            .iter()
            .any(|artifact| artifact.artifact_type == "repair_notes" && artifact.scope == scope)
        {
            return Ok(None);
        }
        let Some(original_candidate_id) = self
            .runs
            .previous_candidate_id(&claimed.run_id, candidate_id)
            .await?
        else {
            return Ok(None);
        };
        let spec = find_step(&production_plan(), "bounded_repair")
            .copied()
            .expect("bounded repair step");
        self.finish_repair_and_rereview(
            claimed,
            &spec,
            attempt.id.clone(),
            &original_candidate_id,
            candidate_id,
        )
        .await
        .map(Some)
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
                .promote_candidate_fenced(
                    &claimed.fence,
                    &claimed.run_id,
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

    /// Persist a safety pause exactly once. A concurrent/previous pause may have
    /// already won the control CAS; in that case return the paused outcome instead
    /// of surfacing a misleading worker error. Other conflicts still propagate.
    async fn pause_safety(
        &self,
        claimed: &ClaimedRun,
        reason: &str,
        step_id: Option<String>,
    ) -> Result<TickOutcome, AppError> {
        match self
            .pause(claimed, StopSignalKind::Safety, reason, step_id)
            .await
        {
            Ok(outcome) => Ok(outcome),
            Err(error) => {
                let control = self.runs.run_control(&claimed.run_id).await?;
                // Any persisted pause means the write already happened; do not
                // surface a second conflict for the same transition.
                if control.status == novel_domain::RunStatus::Paused {
                    Ok(TickOutcome::Advanced {
                        run_id: claimed.run_id.clone(),
                        status: "paused".into(),
                    })
                } else {
                    Err(error)
                }
            }
        }
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
                fence: Some(claimed.fence.clone()),
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
            step_id.clone(),
            payload,
            occurred_at,
        ) {
            self.hub.emit(event);
        }
        // `save_candidate` already persisted a checkpoint event before the step event. Re-emit
        // that exact stored event on the live channel so subscribers see the same seq they would
        // receive after reconnecting and replaying.
        if kind == "step_succeeded"
            && step_id.as_deref() == Some("draft")
            && let Ok(stored_events) = self.runs.events_after(&claimed.run_id, 0).await
            && let Some(stored) = stored_events
                .iter()
                .rev()
                .find(|event| event.kind == "checkpoint_saved")
            && let Some(event) = event_from_stored(
                &stored.run_id,
                stored.seq,
                &stored.kind,
                stored.step_id.clone(),
                &stored.payload_json,
                stored.occurred_at,
            )
        {
            self.hub.emit(event);
        }
        tracing::info!(run_id = %claimed.run_id, status = committed.control.status.as_str(), "run advanced");
        Ok(TickOutcome::Advanced {
            run_id: claimed.run_id.clone(),
            status: committed.control.status.as_str().to_string(),
        })
    }
}

async fn execute_structured(
    worker: &Worker,
    claimed: &ClaimedRun,
    spec: &StepSpec,
    step_id: &str,
) -> Result<TickOutcome, AppError> {
    let prompt = structured_prompt(worker, claimed, step_id).await?;
    let summary = summarize(&prompt);
    let attempt_id = worker.ids.new_id();
    worker
        .runs
        .start_attempt_fenced(
            &claimed.fence,
            NewAttempt {
                id: attempt_id.clone(),
                run_id: claimed.run_id.clone(),
                step_id: step_id.into(),
                control_version: claimed.control.version,
                created_at: worker.clock.now_ms(),
            },
            worker.clock.now_ms(),
        )
        .await?;
    let request_id = worker.ids.new_id();
    if !worker
        .runs
        .mark_remote_started_fenced(
            &claimed.fence,
            &claimed.run_id,
            &attempt_id,
            &request_id,
            worker.clock.now_ms(),
        )
        .await?
    {
        return Ok(TickOutcome::LostLease {
            run_id: claimed.run_id.clone(),
        });
    }
    let route = claimed
        .contract
        .effective_route(task_type_for_step(step_id));
    let credential_ref = route
        .credential_ref_checked(
            &claimed.contract.provider,
            claimed.contract.credential_ref.as_deref(),
        )
        .map_err(|error| AppError::validation(error.message.clone()))?;
    let api_key = worker
        .secrets
        .api_key(credential_ref)
        .await?
        .unwrap_or_default();
    crate::provider_service::ensure_credential(
        route.protocol(claimed.contract.protocol.as_deref()),
        route.endpoint(&claimed.contract.endpoint),
        &api_key,
    )
    .map_err(|error| AppError::validation(error.message.clone()))?;
    let cancel = CancellationToken::new();
    worker
        .cancels
        .lock()
        .expect("cancel map")
        .insert(claimed.run_id.clone(), cancel.clone());
    let _cancel_guard = CancelRegistration {
        run_id: claimed.run_id.clone(),
        cancels: worker.cancels.clone(),
    };
    let (sender, mut receiver) = tokio::sync::mpsc::channel::<String>(64);
    let collect = tokio::spawn(async move {
        let mut text = String::new();
        while let Some(delta) = receiver.recv().await {
            text.push_str(&delta);
        }
        text
    });
    let started = worker.clock.now_ms();
    let generated = worker
        .llm
        .generate_with_protocol(
            LlmRequest {
                request_id: request_id.clone(),
                model: route.model_id(&claimed.contract.model).to_string(),
                prompt,
                endpoint: route.endpoint(&claimed.contract.endpoint).to_string(),
                api_key,
                allow_localhost_http: route
                    .allow_localhost_http(claimed.contract.allow_localhost_http),
                max_output_tokens: claimed.contract.token_budget,
            },
            route.protocol(claimed.contract.protocol.as_deref()),
            route.reasoning_effort(claimed.contract.reasoning_effort.as_deref()),
            sender,
            cancel,
        )
        .await;
    let raw = collect.await.unwrap_or_default();
    let response = match generated {
        Ok(response) => response,
        Err(error) => {
            worker
                .runs
                .mark_attempt_fenced(
                    &claimed.fence,
                    &claimed.run_id,
                    &attempt_id,
                    AttemptPhase::Interrupted.as_str(),
                    None,
                    worker.clock.now_ms(),
                )
                .await?;
            if matches!(error, novel_llm::LlmError::Cancelled) {
                return worker
                    .write(
                        claimed,
                        ControlEvent::Cancel,
                        "run_cancelled",
                        Some(step_id.into()),
                        &serde_json::json!({ "reason": error.to_string() }).to_string(),
                        None,
                        None,
                        None,
                    )
                    .await;
            }
            return worker
                .pause(
                    claimed,
                    StopSignalKind::ProviderUnavailable,
                    &error.to_string(),
                    Some(step_id.into()),
                )
                .await;
        }
    };
    let raw = if raw.trim().is_empty() {
        response.text.clone()
    } else {
        raw
    };
    let payload = match validate_structured_output(step_id, &raw) {
        Ok(payload) => payload,
        Err(error) => {
            worker
                .runs
                .mark_attempt_fenced(
                    &claimed.fence,
                    &claimed.run_id,
                    &attempt_id,
                    AttemptPhase::Interrupted.as_str(),
                    None,
                    worker.clock.now_ms(),
                )
                .await?;
            return worker
                .pause(
                    claimed,
                    StopSignalKind::DataIntegrity,
                    &error,
                    Some(step_id.into()),
                )
                .await;
        }
    };
    let payload = if step_id == "chapter_list" {
        let artifacts = worker.runs.list_artifacts(&claimed.novel_id).await?;
        let cast = latest_payload(&artifacts, "character_cast", book_scope())?;
        let volume = latest_payload(&artifacts, "volume_strategy", book_scope())?;
        validate_chapter_plan_references(&payload, &cast, &volume)?
    } else {
        payload
    };
    worker
        .runs
        .record_invocation_fenced(
            &claimed.fence,
            invocation(
                worker,
                claimed,
                step_id,
                &request_id,
                &summary,
                started,
                &response.usage,
            ),
            worker.clock.now_ms(),
        )
        .await?;
    if step_id == "consistency_review" {
        let artifacts = worker.runs.list_artifacts(&claimed.novel_id).await?;
        let next_version = artifacts
            .iter()
            .filter(|artifact| {
                artifact.artifact_type == "consistency_review"
                    && artifact.scope == chapter_scope(&claimed.contract.target_chapter_id)
            })
            .map(|artifact| artifact.version)
            .max()
            .unwrap_or(0)
            + 1;
        record_review_debt(worker, claimed, &payload, next_version).await?;
    }
    let status = if spec.gateable { "draft" } else { "confirmed" };
    worker
        .finish_step(
            claimed,
            spec,
            status,
            false,
            &payload,
            None,
            Some(attempt_id),
        )
        .await
}

async fn record_review_debt(
    worker: &Worker,
    claimed: &ClaimedRun,
    payload: &str,
    review_version: i64,
) -> Result<(), AppError> {
    let value: serde_json::Value =
        serde_json::from_str(payload).map_err(|_| AppError::integrity("审校产物无法解析"))?;
    let issues = value
        .get("issueDetails")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    if issues.is_empty() {
        worker
            .runs
            .resolve_quality_debt_fenced(
                &claimed.fence,
                &claimed.run_id,
                &format!("审校版本 v{review_version} 通过，无遗留问题。"),
                worker.clock.now_ms(),
            )
            .await?;
        return Ok(());
    }
    let now = worker.clock.now_ms();
    let mut occurrences = Vec::new();
    let mut debts = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for issue in &issues {
        let issue_type = issue
            .get("type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown");
        let severity = issue
            .get("severity")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("medium");
        let description = issue
            .get("description")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        let evidence = issue
            .get("evidence")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        let suggestion = issue
            .get("suggestion")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        let source_ref = issue.get("sourceRef").and_then(serde_json::Value::as_str);
        let model_issue_id = issue.get("issueId").and_then(serde_json::Value::as_str);
        let stable_key = stable_issue_key(issue_type, source_ref, evidence);
        occurrences.push(NewReviewOccurrence {
            id: worker.ids.new_id(),
            novel_id: claimed.novel_id.clone(),
            chapter_id: claimed.contract.target_chapter_id.clone(),
            run_id: claimed.run_id.clone(),
            model_issue_id: model_issue_id.map(str::to_string),
            stable_issue_key: stable_key.clone(),
            issue_type: issue_type.to_string(),
            severity: severity.to_string(),
            description: description.to_string(),
            evidence: evidence.to_string(),
            suggestion: suggestion.to_string(),
            source_ref: source_ref.map(str::to_string),
            review_version,
            created_at: now,
        });
        if seen.insert(stable_key.clone()) {
            debts.push(NewQualityDebt {
                id: format!("debt-{}-{stable_key}", claimed.run_id),
                novel_id: claimed.novel_id.clone(),
                chapter_id: claimed.contract.target_chapter_id.clone(),
                run_id: claimed.run_id.clone(),
                issue_id: stable_key,
                issue_type: issue_type.to_string(),
                severity: severity.to_string(),
                description: description.to_string(),
                evidence: evidence.to_string(),
                suggestion: suggestion.to_string(),
                source_ref: source_ref.map(str::to_string),
                review_version,
                now_ms: now,
            });
        }
    }
    worker
        .runs
        .record_review_occurrences_fenced(&claimed.fence, &occurrences, now)
        .await?;
    worker
        .runs
        .record_quality_debt_fenced(&claimed.fence, &debts, now)
        .await?;
    Ok(())
}

/// Deterministic identity for tracking the same business issue across reviews.
/// If any of the stable inputs is absent the full normalized evidence is used;
/// uncertain matches remain separate rather than being auto-merged.
fn stable_issue_key(issue_type: &str, source_ref: Option<&str>, evidence: &str) -> String {
    let normalized = evidence
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    let material = format!(
        "{}\u{001f}{}\u{001f}{}",
        issue_type.trim().to_lowercase(),
        source_ref.unwrap_or("").trim().to_lowercase(),
        normalized
    );
    format!("issue-{}", &sha256_hex(material.as_bytes())[..32])
}

async fn execute_context(
    worker: &Worker,
    claimed: &ClaimedRun,
    spec: &StepSpec,
) -> Result<TickOutcome, AppError> {
    let artifacts = worker.runs.list_artifacts(&claimed.novel_id).await?;
    let assembly = assemble_director_context(
        worker.novels.as_ref(),
        &artifacts,
        &claimed.novel_id,
        &claimed.contract.target_chapter_id,
        claimed.contract.context_token_budget,
        claimed
            .contract
            .token_budget
            .unwrap_or(DEFAULT_OUTPUT_RESERVE),
    )
    .await?;
    let chapter_key = chapter_scope(&claimed.contract.target_chapter_id);
    let payload = serde_json::json!({
        "assemblyVersion": assembly.trace.assembly_version,
        "targetChapterId": claimed.contract.target_chapter_id,
        "scope": chapter_key,
        "promptText": assembly.prompt_text,
        "trace": assembly.trace,
    })
    .to_string();
    worker
        .finish_step(claimed, spec, "confirmed", false, &payload, None, None)
        .await
}

async fn structured_prompt(
    worker: &Worker,
    claimed: &ClaimedRun,
    step_id: &str,
) -> Result<String, AppError> {
    let artifacts = worker.runs.list_artifacts(&claimed.novel_id).await?;
    let (id, version, vars) = match step_id {
        "story_macro" => {
            let novel = worker.novels.get_novel(&claimed.novel_id).await?;
            let mut vars = BTreeMap::new();
            vars.insert(
                "story_input".into(),
                format!("{}\n{}", novel.title, novel.description),
            );
            ("story_macro", "story-macro-v1", vars)
        }
        "character_cast" => {
            let mut vars = BTreeMap::new();
            vars.insert(
                "context".into(),
                context_payload(&artifacts, &["story_macro"], 12_000)?,
            );
            ("character_cast", "character-cast-v1", vars)
        }
        "volume_strategy" => {
            let mut vars = BTreeMap::new();
            vars.insert(
                "context".into(),
                context_payload(&artifacts, &["story_macro", "character_cast"], 16_000)?,
            );
            ("volume_strategy", "volume-strategy-v1", vars)
        }
        "chapter_list" => {
            let mut vars = BTreeMap::new();
            vars.insert(
                "context".into(),
                context_payload(&artifacts, &["volume_strategy", "character_cast"], 20_000)?,
            );
            ("chapter_list", "chapter-list-v1", vars)
        }
        "consistency_review" => {
            let candidate = worker
                .runs
                .latest_candidate(&claimed.run_id)
                .await?
                .ok_or_else(|| AppError::integrity("一致性审校缺少候选正文。"))?;
            let mut context = context_payload(
                &artifacts,
                &["chapter_list", "character_cast", "story_macro"],
                16_000,
            )?;
            context.push_str("\n候选正文：");
            context.push_str(&truncate_chars(&candidate.content, 20_000));
            let mut vars = BTreeMap::new();
            vars.insert("context".into(), context);
            ("consistency_review", "consistency-review-v1", vars)
        }
        _ => return Err(AppError::validation("未知的结构化步骤")),
    };
    let asset =
        novel_prompt::resolve(id, version).map_err(|_| AppError::validation("提示词版本不存在"))?;
    novel_prompt::render(asset.template, &vars)
        .map_err(|_| AppError::validation("提示词缺少必要上下文"))
}

async fn draft_prompt(
    worker: &Worker,
    claimed: &ClaimedRun,
    title: &str,
) -> Result<String, AppError> {
    use std::collections::BTreeMap;
    let version = claimed
        .contract
        .prompt_versions
        .get("chapter_draft")
        .map(String::as_str)
        .unwrap_or("");
    let asset = novel_prompt::resolve("chapter_draft", version)
        .map_err(|_| AppError::validation("正文提示词版本不存在"))?;
    let mut vars = BTreeMap::new();
    vars.insert("brief".into(), title.to_string());
    if version == "chapter-draft-v2" {
        let artifacts = worker.runs.list_artifacts(&claimed.novel_id).await?;
        let context = latest_payload(
            &artifacts,
            "context",
            &chapter_scope(&claimed.contract.target_chapter_id),
        )?;
        let prompt_text = context
            .get("promptText")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| AppError::integrity("上下文产物缺少 promptText"))?;
        vars.insert("context".into(), prompt_text.to_string());
    }
    novel_prompt::render(asset.template, &vars)
        .map_err(|_| AppError::validation("正文提示词缺少章节上下文"))
}

fn summarize(prompt: &str) -> String {
    redact_secrets(prompt).chars().take(80).collect()
}

fn invocation(
    worker: &Worker,
    claimed: &ClaimedRun,
    step_id: &str,
    request_id: &str,
    summary: &str,
    started: i64,
    usage: &TokenUsage,
) -> NewInvocation {
    let route = claimed
        .contract
        .effective_route(task_type_for_step(step_id));
    let provider = match route {
        crate::provider_service::EffectiveRoute::Step(step) => step.provider_id.clone(),
        crate::provider_service::EffectiveRoute::Contract => claimed.contract.provider.clone(),
    };
    NewInvocation {
        id: worker.ids.new_id(),
        request_id: request_id.to_string(),
        run_id: claimed.run_id.clone(),
        step_id: step_id.to_string(),
        provider,
        model: route.model_id(&claimed.contract.model).to_string(),
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

struct CancelRegistration {
    run_id: String,
    cancels: Arc<Mutex<HashMap<String, CancellationToken>>>,
}

impl Drop for CancelRegistration {
    fn drop(&mut self) {
        self.cancels
            .lock()
            .expect("cancel map")
            .remove(&self.run_id);
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

/// 步骤 id 到绑定任务类型的唯一映射：前后端都用这套名字，不另造一套。
fn task_type_for_step(step_id: &str) -> &'static str {
    match step_id {
        "draft" => "writer",
        "consistency_review" => "review",
        "bounded_repair" => "repair",
        "story_macro" => "planner",
        "character_cast" | "volume_strategy" | "chapter_list" => "planner",
        "state_resolution" => "state_resolution",
        "summary" => "summary",
        "fact_extraction" => "fact_extraction",
        other => {
            let _ = other;
            "writer"
        }
    }
}

#[cfg(test)]
mod structured_validation_tests {
    use super::validate_structured_output;

    #[test]
    fn story_macro_requires_real_structure() {
        assert!(validate_structured_output("story_macro", r#"{"ok":true}"#).is_err());
        let valid = r#"{"expansion":{"expanded_premise":"前提","protagonist_core":"核心","conflict_engine":"引擎","conflict_layers":{"external":"外","internal":"内","relational":"关系"},"mystery_box":"未知","emotional_line":"情绪","setpiece_seeds":["场面"],"tone_reference":"气质"},"decomposition":{"selling_point":"卖点","core_conflict":"冲突","main_hook":"钩子","progression_loop":"循环","growth_path":"成长","major_payoffs":["回报"],"ending_flavor":"结局"},"constraints":["规则"],"issues":[]}"#;
        assert!(validate_structured_output("story_macro", valid).is_ok());
    }

    #[test]
    fn character_cast_rejects_too_few_members_or_relations() {
        let too_few = r#"{"members":[{"name":"甲","role":"主角","castRole":"protagonist","storyFunction":"作用"}],"relations":[]}"#;
        assert!(validate_structured_output("character_cast", too_few).is_err());
    }

    #[test]
    fn volume_and_chapter_plans_reject_duplicate_order() {
        let bad_volume = r#"{"volumes":[{"volumeOrder":1,"title":"卷","roleLabel":"开局","coreReward":"回报","planningMode":"hard"},{"volumeOrder":1,"title":"卷二","roleLabel":"推进","coreReward":"回报二","planningMode":"soft"}]}"#;
        assert!(validate_structured_output("volume_strategy", bad_volume).is_err());
        let bad_chapters = r#"{"chapters":[{"chapterOrder":1,"title":"章","volumeTitle":"卷","summary":"梗概"},{"chapterOrder":1,"title":"章二","volumeTitle":"卷","summary":"梗概"}]}"#;
        assert!(validate_structured_output("chapter_list", bad_chapters).is_err());
    }

    #[test]
    fn duplicate_review_ids_must_not_merge_different_issues() {
        let exact = r#"{"issues":[{"issueId":"i1","type":"timeline","severity":"high","evidence":"e","suggestion":"s"},{"issueId":"i1","type":"timeline","severity":"high","evidence":"e","suggestion":"s"}]}"#;
        let normalized = validate_structured_output("consistency_review", exact).unwrap();
        assert!(normalized.contains("\"issues\":1"));
        let conflicting = r#"{"issues":[{"issueId":"i1","type":"timeline","severity":"high","evidence":"e","suggestion":"s"},{"issueId":"i1","type":"character","severity":"high","evidence":"e2","suggestion":"s2"}]}"#;
        assert!(validate_structured_output("consistency_review", conflicting).is_err());
    }

    #[test]
    fn consistency_empty_is_clean_but_malformed_is_not_clean() {
        let clean = validate_structured_output("consistency_review", r#"{"issues":[]}"#).unwrap();
        assert!(clean.contains("\"issues\":0"));
        assert!(validate_structured_output("consistency_review", r#"{"issues":"none"}"#).is_err());
        assert!(
            validate_structured_output("consistency_review", r#"{"issues":[{"type":"x"}]}"#)
                .is_err()
        );
    }

    #[test]
    fn stable_issue_identity_ignores_model_issue_id_and_normalizes_evidence() {
        let a = super::stable_issue_key("timeline", Some("chapter-1"), "  Time    LINE ");
        let b = super::stable_issue_key("timeline", Some("chapter-1"), "time line");
        let c = super::stable_issue_key("timeline", Some("chapter-1"), "different evidence");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn adversarial_issue_identity_does_not_merge_uncertain_evidence() {
        let same = super::stable_issue_key("timeline", Some("ch-1"), "证据不同描述");
        let same_other_description =
            super::stable_issue_key("timeline", Some("ch-1"), "证据不同描述");
        let different_evidence = super::stable_issue_key("timeline", Some("ch-1"), "另一条证据");
        let different_source = super::stable_issue_key("timeline", Some("ch-2"), "证据不同描述");
        assert_eq!(same, same_other_description);
        assert_ne!(same, different_evidence);
        assert_ne!(same, different_source);
    }
}
