use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use novel_domain::{
    ChapterProtection, ControlEvent, InvocationOutcome, PauseKind, RunPause, StopSignalKind,
    ensure_can_promote,
};
use novel_llm::{LlmProvider, LlmRequest, TokenUsage, redact_secrets};
use novel_workflow::{
    AttemptPhase, CHAPTER_PLAN_VERSION, NextAction, RecoveryDecision, StepScope, StepSpec,
    book_scope, chapter_scope, decide, find_step, next_action, plan_for_version, production_plan,
};
use tokio_util::sync::CancellationToken;

use crate::context::{
    DEFAULT_OUTPUT_RESERVE, RunContextPins, assemble_director_context_for_run, choose_run_artifact,
    latest_run_context_payload, pinned_chapter_plan_version, target_obligations_from_artifacts,
};
use crate::dto::{GenerationEvent, event_from_stored};
use crate::error::AppError;
use crate::hash::sha256_hex;
use crate::hub::EventHub;
use crate::ports::{
    ClaimedRun, Clock, ControlWrite, FaultInjector, Ids, LeaseFence, ModelSecrets, NewArtifact,
    NewAttempt, NewCandidate, NewInvocation, NewQualityDebt, NewReviewOccurrence, NovelRepository,
    PromoteOutcome, WorkflowRepository,
};

mod context_budget;
mod worker_validation;

pub(super) use context_budget::{context_payload_for_run, payload_for_run, run_scoped_payload};
pub(super) use worker_validation::{
    validate_chapter_plan_references, validate_structured_output_with,
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
                    record_failed_invocation(
                        self,
                        claimed,
                        "draft",
                        &request_id,
                        &summary,
                        started,
                        &error,
                        Some(&attempt_id),
                    )
                    .await?;
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
                // 请求已经发出但结果不可用时，费用可能已经发生；先记账再暂停。
                record_failed_invocation(
                    self,
                    claimed,
                    "draft",
                    &request_id,
                    &summary,
                    started,
                    &error,
                    Some(&attempt_id),
                )
                .await?;
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
                    InvocationOutcome::Succeeded,
                    Some(&attempt_id),
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
        let review = run_scoped_payload(&artifacts, "consistency_review", &scope, &claimed.run_id)?;
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
        // advisory 只是建议，疑似缺席或证据不足需要作者确认：两者都不能触发
        // hard repair；问题仍保留在审校产物和 quality debt 中供作者查看。
        let issues: Vec<serde_json::Value> = issues
            .into_iter()
            .filter(|issue| {
                issue
                    .get("obligationEnforcement")
                    .and_then(serde_json::Value::as_str)
                    != Some("advisory")
                    && issue
                        .get("automationDisposition")
                        .and_then(serde_json::Value::as_str)
                        != Some("manual_confirmation")
            })
            .collect();
        if issues.is_empty() {
            return self
                .finish_step(
                    claimed,
                    spec,
                    "confirmed",
                    false,
                    &serde_json::json!({ "status": "advisory_only" }).to_string(),
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
        let context = latest_run_context_payload(
            &artifacts,
            &claimed.contract.target_chapter_id,
            &claimed.run_id,
        )?;
        let context_text = context
            .get("promptText")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| AppError::integrity("修复缺少上下文。"))?;
        let obligations_text = context
            .get("obligationsText")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let repair_version = claimed
            .contract
            .prompt_versions
            .get("chapter_repair")
            .map(String::as_str)
            .unwrap_or("chapter-repair-v2");
        let repair_asset = novel_prompt::resolve("chapter_repair", repair_version)
            .map_err(|_| AppError::validation("修订提示词版本不存在"))?;
        let mut vars = BTreeMap::new();
        vars.insert("context".into(), context_text.to_string());
        vars.insert(
            "obligations".into(),
            if obligations_text.trim().is_empty() {
                "（没有结构化义务；沿用已确认上下文，不得改写已确认事件）".to_string()
            } else {
                obligations_text.to_string()
            },
        );
        vars.insert(
            "issues".into(),
            serde_json::to_string(&issues)
                .map_err(|_| AppError::integrity("审校问题无法序列化"))?,
        );
        vars.insert("candidate".into(), candidate.content.clone());
        let prompt = novel_prompt::render(repair_asset.template, &vars)
            .map_err(|_| AppError::validation("修订提示词缺少上下文"))?;
        let summary = summarize(&prompt);
        let repair_max_output_tokens = repair_output_budget(claimed.contract.token_budget);
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
                    max_output_tokens: Some(repair_max_output_tokens),
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
                    record_failed_invocation(
                        self,
                        claimed,
                        "bounded_repair",
                        &request_id,
                        &summary,
                        started,
                        &error,
                        Some(&attempt_id),
                    )
                    .await?;
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
                record_failed_invocation(
                    self,
                    claimed,
                    "bounded_repair",
                    &request_id,
                    &summary,
                    started,
                    &error,
                    Some(&attempt_id),
                )
                .await?;
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
        if repair_usage_reached_cap(&response.usage, repair_max_output_tokens) {
            record_usage_before_stop(
                self,
                claimed,
                "bounded_repair",
                &request_id,
                &summary,
                started,
                &response.usage,
                InvocationOutcome::Succeeded,
                Some(&attempt_id),
            )
            .await?;
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
            return self
                .pause(
                    claimed,
                    StopSignalKind::DataIntegrity,
                    &format!(
                        "修复输出达到 {repair_max_output_tokens} token 上限，候选可能被截断，未保存。请提高修复输出预算后重试。"
                    ),
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
                    InvocationOutcome::Succeeded,
                    Some(&attempt_id),
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
        let latest_review = choose_run_artifact(
            &after,
            "consistency_review",
            &scope,
            &claimed.run_id,
            &BTreeMap::new(),
        );
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
        self.pause_with_artifact(claimed, signal, reason, step_id, None)
            .await
    }

    /// Pause and persist a diagnostic artifact in the same control write, so a
    /// failed structured step leaves its raw output for post-mortem analysis.
    async fn pause_with_artifact(
        &self,
        claimed: &ClaimedRun,
        signal: StopSignalKind,
        reason: &str,
        step_id: Option<String>,
        artifact: Option<NewArtifact>,
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
            artifact,
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
    let story_macro_v2 = claimed
        .contract
        .prompt_versions
        .get("story_macro")
        .map(String::as_str)
        == Some("story-macro-v2");
    let chapter_list_v3 = claimed
        .contract
        .prompt_versions
        .get("chapter_list")
        .map(String::as_str)
        == Some("chapter-list-v3");
    let prompt = match structured_prompt(worker, claimed, step_id).await {
        Ok(prompt) => prompt,
        Err(error) => {
            return worker
                .pause(
                    claimed,
                    StopSignalKind::DataIntegrity,
                    &error.to_string(),
                    Some(step_id.into()),
                )
                .await;
        }
    };
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
    let repair_cancel = cancel.clone();
    let original_prompt = prompt.clone();
    let generated = worker
        .llm
        .generate_with_protocol(
            LlmRequest {
                request_id: request_id.clone(),
                model: route.model_id(&claimed.contract.model).to_string(),
                prompt,
                endpoint: route.endpoint(&claimed.contract.endpoint).to_string(),
                api_key: api_key.clone(),
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
                record_failed_invocation(
                    worker,
                    claimed,
                    step_id,
                    &request_id,
                    &summary,
                    started,
                    &error,
                    Some(&attempt_id),
                )
                .await?;
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
            record_failed_invocation(
                worker,
                claimed,
                step_id,
                &request_id,
                &summary,
                started,
                &error,
                Some(&attempt_id),
            )
            .await?;
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
    // 首次物理请求到此结束。它自己的记录先落库，后续修复请求单独记账，
    // 不能把两次物理调用的用量合并成一条。
    let first_usage = response.usage.clone();
    // 首次请求成功返回但结构不可用：这条记录仍算「已计费且结果已知」，
    // 失败原因是结构校验，由 repair_failed / 诊断产物承载。
    let first_outcome = InvocationOutcome::Succeeded;
    let payload = match validate_structured_output_with(
        step_id,
        &raw,
        step_id == "story_macro" && story_macro_v2,
    ) {
        Ok(payload) => payload,
        Err(first_error) => {
            // 审校本身是质量门禁。无效的审校 JSON 不能被“修复”为空 issues，
            // 否则一次模型格式失败会伪装成零问题通过；这里直接 fail-closed。
            if step_id == "consistency_review" {
                record_usage_before_stop(
                    worker,
                    claimed,
                    step_id,
                    &request_id,
                    &summary,
                    started,
                    &first_usage,
                    first_outcome,
                    Some(&attempt_id),
                )
                .await?;
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
                let artifact = structured_failure_artifact(
                    worker,
                    claimed,
                    spec,
                    step_id,
                    &first_error,
                    None,
                    &raw,
                    None,
                    &first_usage,
                );
                return worker
                    .pause_with_artifact(
                        claimed,
                        StopSignalKind::DataIntegrity,
                        &format!(
                            "一致性审校输出无效，已停止，不能当作零问题通过：{first_error}；原始输出已保存为 structured_failure 产物。"
                        ),
                        Some(step_id.into()),
                        Some(artifact),
                    )
                    .await;
            }
            // 规划产物可能是被截断的 JSON，或字段类型/必填字段不符合契约。
            // 只做一次有界修复调用：把原始输出与校验错误交给模型，要求只返回完整 JSON。
            let repair_prompt =
                structured_repair_prompt(step_id, &original_prompt, &first_error, &raw)?;
            let repair_request_id = worker.ids.new_id();
            let (repair_sender, mut repair_receiver) = tokio::sync::mpsc::channel::<String>(64);
            let repair_collect = tokio::spawn(async move {
                let mut text = String::new();
                while let Some(delta) = repair_receiver.recv().await {
                    text.push_str(&delta);
                }
                text
            });
            let repair_started = worker.clock.now_ms();
            let repair_summary = summarize(&repair_prompt);
            let repair_generated = worker
                .llm
                .generate_with_protocol(
                    LlmRequest {
                        request_id: repair_request_id.clone(),
                        model: route.model_id(&claimed.contract.model).to_string(),
                        prompt: repair_prompt,
                        endpoint: route.endpoint(&claimed.contract.endpoint).to_string(),
                        api_key: api_key.clone(),
                        allow_localhost_http: route
                            .allow_localhost_http(claimed.contract.allow_localhost_http),
                        max_output_tokens: claimed.contract.token_budget,
                    },
                    route.protocol(claimed.contract.protocol.as_deref()),
                    route.reasoning_effort(claimed.contract.reasoning_effort.as_deref()),
                    repair_sender,
                    repair_cancel,
                )
                .await;
            let repair_raw = repair_collect.await.unwrap_or_default();
            match repair_generated {
                Ok(repair_response) => {
                    let repair_text = if repair_raw.trim().is_empty() {
                        repair_response.text
                    } else {
                        repair_raw
                    };
                    match validate_structured_output_with(
                        step_id,
                        &repair_text,
                        step_id == "story_macro" && story_macro_v2,
                    ) {
                        Ok(payload) => {
                            record_repair_invocation(
                                worker,
                                claimed,
                                step_id,
                                &repair_request_id,
                                &repair_summary,
                                repair_started,
                                &repair_response.usage,
                                InvocationOutcome::Repaired,
                                &attempt_id,
                            )
                            .await?;
                            payload
                        }
                        Err(repair_error) => {
                            record_usage_before_stop(
                                worker,
                                claimed,
                                step_id,
                                &request_id,
                                &summary,
                                started,
                                &first_usage,
                                first_outcome,
                                Some(&attempt_id),
                            )
                            .await?;
                            record_repair_invocation(
                                worker,
                                claimed,
                                step_id,
                                &repair_request_id,
                                &repair_summary,
                                repair_started,
                                &repair_response.usage,
                                InvocationOutcome::RepairFailed,
                                &attempt_id,
                            )
                            .await?;
                            let mut combined = first_usage.clone();
                            accumulate_usage(&mut combined, &repair_response.usage);
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
                            let artifact = structured_failure_artifact(
                                worker,
                                claimed,
                                spec,
                                step_id,
                                &first_error,
                                Some(&repair_error),
                                &raw,
                                Some(&repair_text),
                                &combined,
                            );
                            return worker
                                .pause_with_artifact(
                                    claimed,
                                    StopSignalKind::DataIntegrity,
                                    &format!(
                                        "{first_error}；修复重试仍失败：{repair_error}；原始输出已保存为 structured_failure 产物。"
                                    ),
                                    Some(step_id.into()),
                                    Some(artifact),
                                )
                                .await;
                        }
                    }
                }
                Err(repair_error) => {
                    record_usage_before_stop(
                        worker,
                        claimed,
                        step_id,
                        &request_id,
                        &summary,
                        started,
                        &first_usage,
                        first_outcome,
                        Some(&attempt_id),
                    )
                    .await?;
                    // 修复请求同样是一次物理调用；失败也要留下自己的记录。
                    record_failed_invocation(
                        worker,
                        claimed,
                        step_id,
                        &repair_request_id,
                        &repair_summary,
                        repair_started,
                        &repair_error,
                        Some(&attempt_id),
                    )
                    .await?;
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
                    // 修复调用同样受取消令牌控制：用户取消时必须进入 cancelled，
                    // 不能退化成可恢复的 ProviderUnavailable 暂停。
                    if matches!(repair_error, novel_llm::LlmError::Cancelled) {
                        return worker
                            .write(
                                claimed,
                                ControlEvent::Cancel,
                                "run_cancelled",
                                Some(step_id.into()),
                                &serde_json::json!({ "reason": repair_error.to_string() })
                                    .to_string(),
                                None,
                                None,
                                None,
                            )
                            .await;
                    }
                    let repair_error_text = repair_error.to_string();
                    let artifact = structured_failure_artifact(
                        worker,
                        claimed,
                        spec,
                        step_id,
                        &first_error,
                        Some(&repair_error_text),
                        &raw,
                        None,
                        &first_usage,
                    );
                    return worker
                        .pause_with_artifact(
                            claimed,
                            StopSignalKind::ProviderUnavailable,
                            &format!(
                                "结构化修复请求失败：{repair_error_text}；原始输出已保存为 structured_failure 产物。"
                            ),
                            Some(step_id.into()),
                            Some(artifact),
                        )
                        .await;
                }
            }
        }
    };
    let payload = if step_id == "consistency_review" {
        let artifacts = worker.runs.list_artifacts(&claimed.novel_id).await?;
        let target = worker
            .novels
            .get_chapter(&claimed.novel_id, &claimed.contract.target_chapter_id)
            .await?;
        let pinned = match pinned_chapter_plan_version(
            &artifacts,
            &claimed.contract.target_chapter_id,
            target.sort_order,
            &claimed.run_id,
        ) {
            Ok(version) => version,
            Err(error) => {
                return pause_review_validation_error(
                    worker,
                    claimed,
                    &attempt_id,
                    step_id,
                    &request_id,
                    &summary,
                    started,
                    &first_usage,
                    first_outcome,
                    &error.to_string(),
                )
                .await;
            }
        };
        let obligations =
            match target_obligations_from_artifacts(&artifacts, target.sort_order, Some(pinned)) {
                Ok(obligations) => obligations,
                Err(error) => {
                    return pause_review_validation_error(
                        worker,
                        claimed,
                        &attempt_id,
                        step_id,
                        &request_id,
                        &summary,
                        started,
                        &first_usage,
                        first_outcome,
                        &error.to_string(),
                    )
                    .await;
                }
            };
        let allowed: std::collections::BTreeSet<String> =
            obligations.iter().map(|item| item.id.clone()).collect();
        let payload = match crate::obligations::validate_review_obligation_ids(&payload, &allowed) {
            Ok(payload) => payload,
            Err(error) => {
                return pause_review_validation_error(
                    worker,
                    claimed,
                    &attempt_id,
                    step_id,
                    &request_id,
                    &summary,
                    started,
                    &first_usage,
                    first_outcome,
                    &error,
                )
                .await;
            }
        };
        let payload = match crate::obligations::annotate_review_obligations(&payload, &obligations)
        {
            Ok(payload) => payload,
            Err(error) => {
                return pause_review_validation_error(
                    worker,
                    claimed,
                    &attempt_id,
                    step_id,
                    &request_id,
                    &summary,
                    started,
                    &first_usage,
                    first_outcome,
                    &error,
                )
                .await;
            }
        };
        let candidate = match worker.runs.latest_candidate(&claimed.run_id).await? {
            Some(candidate) => candidate,
            None => {
                return pause_review_validation_error(
                    worker,
                    claimed,
                    &attempt_id,
                    step_id,
                    &request_id,
                    &summary,
                    started,
                    &first_usage,
                    first_outcome,
                    "一致性审校缺少候选正文，无法核验审校证据。",
                )
                .await;
            }
        };
        match crate::obligations::validate_review_evidence(
            &payload,
            &candidate.content,
            &obligations,
        ) {
            Ok(()) => payload,
            Err(error) => {
                return pause_review_validation_error(
                    worker,
                    claimed,
                    &attempt_id,
                    step_id,
                    &request_id,
                    &summary,
                    started,
                    &first_usage,
                    first_outcome,
                    &error,
                )
                .await;
            }
        }
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
                &first_usage,
                first_outcome,
                Some(&attempt_id),
            ),
            worker.clock.now_ms(),
        )
        .await?;
    let payload = if step_id == "chapter_list" {
        let artifacts = worker.runs.list_artifacts(&claimed.novel_id).await?;
        let cast = payload_for_run(
            &artifacts,
            "character_cast",
            book_scope(),
            &claimed.run_id,
            &claimed.contract.source_versions,
        )?;
        let volume = payload_for_run(
            &artifacts,
            "volume_strategy",
            book_scope(),
            &claimed.run_id,
            &claimed.contract.source_versions,
        )?;
        let macro_payload = payload_for_run(
            &artifacts,
            "story_macro",
            book_scope(),
            &claimed.run_id,
            &claimed.contract.source_versions,
        )?;
        match validate_chapter_plan_references(
            &payload,
            &cast,
            &volume,
            &macro_payload,
            chapter_list_v3,
        ) {
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
                        &error.to_string(),
                        Some(step_id.into()),
                    )
                    .await;
            }
        }
    } else {
        payload
    };
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
    let pins = RunContextPins {
        run_id: &claimed.run_id,
        source_versions: &claimed.contract.source_versions,
    };
    let assembly = match assemble_director_context_for_run(
        worker.novels.as_ref(),
        &artifacts,
        &claimed.novel_id,
        &claimed.contract.target_chapter_id,
        &pins,
        claimed.contract.context_token_budget,
        claimed
            .contract
            .token_budget
            .unwrap_or(DEFAULT_OUTPUT_RESERVE),
    )
    .await
    {
        Ok(assembly) => assembly,
        Err(error) => {
            return worker
                .pause(
                    claimed,
                    StopSignalKind::DataIntegrity,
                    &error.to_string(),
                    Some("context".into()),
                )
                .await;
        }
    };
    let chapter_key = chapter_scope(&claimed.contract.target_chapter_id);
    let payload = serde_json::json!({
        "assemblyVersion": assembly.trace.assembly_version,
        "runId": claimed.run_id,
        "targetChapterId": claimed.contract.target_chapter_id,
        "scope": chapter_key,
        "chapterPlanVersion": assembly.chapter_plan_version,
        "obligationIds": assembly.obligation_ids,
        "obligationsText": assembly.obligations_text,
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
            let story_input = if let Some(snapshot) = &claimed.contract.input_snapshot {
                format!("{}\n{}", snapshot.title, snapshot.description)
            } else {
                // 历史合同没有输入快照：明确兼容旧行为，继续读取当前详情；新 Run
                // 一律在 open_run 时冻结快照，不会进入这条回退路径。
                let novel = worker.novels.get_novel(&claimed.novel_id).await?;
                format!("{}\n{}", novel.title, novel.description)
            };
            let mut vars = BTreeMap::new();
            vars.insert("story_input".into(), story_input);
            let version = match claimed
                .contract
                .prompt_versions
                .get("story_macro")
                .map(String::as_str)
            {
                Some("story-macro-v2") => "story-macro-v2",
                Some("story-macro-v1") | None => "story-macro-v1",
                Some(_) => return Err(AppError::validation("不支持的 story_macro Prompt 版本")),
            };
            ("story_macro", version, vars)
        }
        "character_cast" => {
            let mut vars = BTreeMap::new();
            vars.insert(
                "context".into(),
                context_payload_for_run(
                    &artifacts,
                    &["story_macro"],
                    12_000,
                    &claimed.run_id,
                    &claimed.contract.source_versions,
                )?,
            );
            ("character_cast", "character-cast-v1", vars)
        }
        "volume_strategy" => {
            let mut vars = BTreeMap::new();
            vars.insert(
                "context".into(),
                context_payload_for_run(
                    &artifacts,
                    &["story_macro", "character_cast"],
                    16_000,
                    &claimed.run_id,
                    &claimed.contract.source_versions,
                )?,
            );
            ("volume_strategy", "volume-strategy-v1", vars)
        }
        "chapter_list" => {
            let mut vars = BTreeMap::new();
            vars.insert(
                "context".into(),
                context_payload_for_run(
                    &artifacts,
                    &["story_macro", "volume_strategy", "character_cast"],
                    24_000,
                    &claimed.run_id,
                    &claimed.contract.source_versions,
                )?,
            );
            let version = match claimed
                .contract
                .prompt_versions
                .get("chapter_list")
                .map(String::as_str)
            {
                Some("chapter-list-v3") => "chapter-list-v3",
                Some("chapter-list-v2") | None => "chapter-list-v2",
                Some(_) => return Err(AppError::validation("不支持的 chapter_list Prompt 版本")),
            };
            ("chapter_list", version, vars)
        }
        "consistency_review" => {
            let candidate = worker
                .runs
                .latest_candidate(&claimed.run_id)
                .await?
                .ok_or_else(|| AppError::integrity("一致性审校缺少候选正文。"))?;
            // 使用本 Run 冻结的 Writer 上下文，而不是重新读取可能已经变化的
            // story_macro / character_cast；这样审校和正文看到的是同一份计划与义务。
            let context_artifact = latest_run_context_payload(
                &artifacts,
                &claimed.contract.target_chapter_id,
                &claimed.run_id,
            )?;
            let prompt_text = context_artifact
                .get("promptText")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| AppError::integrity("上下文产物缺少 promptText"))?;
            let mut context = prompt_text.to_string();
            context.push_str(
                "\n审校要求：必须完整阅读下面的候选正文后再判断；违反 hard 章节义务时必须填写 obligationId，且只能使用上下文中的义务 ID；一般质量建议必须标记 enforcement=advisory；missing_element 只有在全文明确确认缺席时才可标记 confidence=confirmed_violation，否则标记 confidence=suspected_absence 并设置 needsAuthorConfirmation=true；证据不足时标记 confidence=insufficient_evidence 并设置 needsAuthorConfirmation=true；evidence 必须逐字引用当前候选正文并能在候选正文中定位；不得引用历史正文。\n",
            );
            context.push_str("\n完整候选正文：");
            context.push_str(&candidate.content);
            let mut vars = BTreeMap::new();
            vars.insert("context".into(), context);
            ("consistency_review", "consistency-review-v2", vars)
        }
        _ => return Err(AppError::validation("未知的结构化步骤")),
    };
    let asset =
        novel_prompt::resolve(id, version).map_err(|_| AppError::validation("提示词版本不存在"))?;
    novel_prompt::render(asset.template, &vars)
        .map_err(|_| AppError::validation("提示词缺少必要上下文"))
}

fn structured_step_requirement(step_id: &str) -> &'static str {
    match step_id {
        "story_macro" => {
            "expansion.mystery_box 必须是单个非空字符串，不能是数组，也不能省略；conflict_layers 必须是对象或数组；setpiece_seeds 必须是字符串数组；expansion 与 decomposition 的所有文本字段尽量精简，字段名使用 snake_case。keyEvents 必须是数组且不能为空，至少一条 hard；每项 id 唯一、chapterOrder 从 1 开始、type 合法、description 非空、participants 为字符串数组。"
        }
        "character_cast" => {
            "members 至少 3 人、relations 至少 2 条；字段名使用 name、role、castRole、storyFunction、sourceName、targetName、surfaceRelation。"
        }
        "volume_strategy" => {
            "volumes 至少 1 卷，volumeOrder 唯一；字段名使用 title、roleLabel、coreReward、planningMode。"
        }
        "chapter_list" => {
            "chapters 非空，chapterOrder 唯一；每章 obligations 必须是数组且不能省略，至少包含一条 hard 义务（只写 advisory 或空数组都不合格）；每项 id 唯一、type 在允许集合、description 非空、enforcement 为 hard/advisory、participants 为字符串数组。若 story_macro 有 keyEvents，本章每一条 hard keyEvent 必须有同类型 hard 义务，sourceEventId 与事件 id 一致；义务的 description、participants、type 必须与 keyEvent 完全一致；不得把其他章节的事件写进本章。"
        }
        "consistency_review" => {
            "只输出 issues 数组；每项包含 type、severity、evidence、suggestion；违反章节义务时 obligationId 必须来自上下文中的义务 ID。"
        }
        _ => "只输出完整 JSON。",
    }
}

fn structured_repair_prompt(
    step_id: &str,
    original_prompt: &str,
    error: &str,
    raw: &str,
) -> Result<String, AppError> {
    let asset = novel_prompt::resolve("structured_repair", "structured-repair-v1")
        .map_err(|_| AppError::integrity("结构化修复提示词版本不存在"))?;
    let mut vars = BTreeMap::new();
    vars.insert("step_id".to_string(), step_id.to_string());
    vars.insert(
        "step_requirement".to_string(),
        structured_step_requirement(step_id).to_string(),
    );
    vars.insert("original_prompt".to_string(), original_prompt.to_string());
    vars.insert("error".to_string(), error.to_string());
    vars.insert("raw".to_string(), raw.to_string());
    novel_prompt::render(asset.template, &vars)
        .map_err(|_| AppError::integrity("结构化修复提示词渲染失败"))
}

/// Bounded repair rewrites the whole chapter. The default 8k output reserve is
/// often consumed by reasoning tokens before the prose is complete, which used
/// to persist a truncated candidate mid-sentence. Give repair a higher floor and
/// fail closed when the provider still hits the cap.
const REPAIR_MIN_OUTPUT_TOKENS: u64 = 16_384;

fn repair_output_budget(contract_budget: Option<u64>) -> u64 {
    contract_budget
        .unwrap_or(DEFAULT_OUTPUT_RESERVE)
        .max(REPAIR_MIN_OUTPUT_TOKENS)
}

fn repair_usage_reached_cap(usage: &TokenUsage, budget: u64) -> bool {
    usage
        .completion_tokens
        .is_some_and(|tokens| tokens >= budget)
}

/// Structured planning failures used to pause with only a one-line reason; the
/// raw model output was lost, which made M1 failures impossible to diagnose
/// without spending another paid call. Persist the raw outputs as a diagnostic
/// artifact (redacted, bounded) in the same pause write.
const STRUCTURED_FAILURE_RAW_LIMIT: usize = 200_000;

fn truncate_diagnostic(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let mut out: String = text.chars().take(limit).collect();
    out.push_str("\n...[truncated]");
    out
}

#[allow(clippy::too_many_arguments)]
fn structured_failure_artifact(
    worker: &Worker,
    claimed: &ClaimedRun,
    spec: &StepSpec,
    step_id: &str,
    first_error: &str,
    repair_error: Option<&str>,
    first_raw: &str,
    repair_raw: Option<&str>,
    usage: &TokenUsage,
) -> NewArtifact {
    let now = worker.clock.now_ms();
    let scope = match spec.scope {
        StepScope::Book => book_scope().to_string(),
        StepScope::Chapter => chapter_scope(&claimed.contract.target_chapter_id),
    };
    let payload = serde_json::json!({
        "stepId": step_id,
        "firstError": first_error,
        "repairError": repair_error,
        "firstRaw": truncate_diagnostic(&redact_secrets(first_raw), STRUCTURED_FAILURE_RAW_LIMIT),
        "repairRaw": repair_raw
            .map(|text| truncate_diagnostic(&redact_secrets(text), STRUCTURED_FAILURE_RAW_LIMIT)),
        "promptTokens": usage.prompt_tokens,
        "completionTokens": usage.completion_tokens,
        "model": claimed.contract.model,
        "createdAt": now,
    })
    .to_string();
    NewArtifact {
        id: worker.ids.new_id(),
        novel_id: claimed.novel_id.clone(),
        run_id: Some(claimed.run_id.clone()),
        artifact_type: "structured_failure".into(),
        scope,
        status: "confirmed".into(),
        protected_user_content: false,
        payload_json: payload.clone(),
        content_sha256: sha256_hex(payload.as_bytes()),
        created_at: now,
    }
}

fn accumulate_usage(total: &mut TokenUsage, extra: &TokenUsage) {
    total.prompt_tokens = match (total.prompt_tokens, extra.prompt_tokens) {
        (Some(left), Some(right)) => Some(left.saturating_add(right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    };
    total.completion_tokens = match (total.completion_tokens, extra.completion_tokens) {
        (Some(left), Some(right)) => Some(left.saturating_add(right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    };
}

/// 结构化调用即使最终校验失败，也已经发生了模型请求；在进入暂停/取消前先记录
/// 已获得的 usage，避免失败路径把成本静默丢掉。使用非栅栏写入，保证取消竞争下
/// 不会因控制版本变化而丢失调用记录。
///
/// `outcome` 描述这次物理请求的结果；`usage` 只在成功或修复成功时带有数值。
#[allow(clippy::too_many_arguments)]
async fn record_usage_before_stop(
    worker: &Worker,
    claimed: &ClaimedRun,
    step_id: &str,
    request_id: &str,
    summary: &str,
    started: i64,
    usage: &TokenUsage,
    outcome: InvocationOutcome,
    attempt_id: Option<&str>,
) -> Result<(), AppError> {
    worker
        .runs
        .record_invocation(invocation(
            worker, claimed, step_id, request_id, summary, started, usage, outcome, attempt_id,
        ))
        .await?;
    Ok(())
}

/// 结构化格式修复是一次独立的物理调用，必须有自己的记录与 requestId。
#[allow(clippy::too_many_arguments)]
async fn record_repair_invocation(
    worker: &Worker,
    claimed: &ClaimedRun,
    step_id: &str,
    request_id: &str,
    summary: &str,
    started: i64,
    usage: &TokenUsage,
    outcome: InvocationOutcome,
    attempt_id: &str,
) -> Result<(), AppError> {
    worker
        .runs
        .record_invocation(invocation(
            worker,
            claimed,
            step_id,
            request_id,
            summary,
            started,
            usage,
            outcome,
            Some(attempt_id),
        ))
        .await?;
    Ok(())
}

/// 一次失败的物理调用同样需要留下记录。
///
/// 取消也记账：取消可能发生在请求已经发出之后，无法证明 Provider 没有计费，
/// 因此这条记录的用量状态是未知，而不是「没花钱」。
#[allow(clippy::too_many_arguments)]
async fn record_failed_invocation(
    worker: &Worker,
    claimed: &ClaimedRun,
    step_id: &str,
    request_id: &str,
    summary: &str,
    started: i64,
    error: &novel_llm::LlmError,
    attempt_id: Option<&str>,
) -> Result<(), AppError> {
    worker
        .runs
        .record_invocation(invocation(
            worker,
            claimed,
            step_id,
            request_id,
            summary,
            started,
            &TokenUsage::unknown(),
            invocation_outcome_for_error(error),
            attempt_id,
        ))
        .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn pause_review_validation_error(
    worker: &Worker,
    claimed: &ClaimedRun,
    attempt_id: &str,
    step_id: &str,
    request_id: &str,
    summary: &str,
    started: i64,
    usage: &TokenUsage,
    outcome: InvocationOutcome,
    error: &str,
) -> Result<TickOutcome, AppError> {
    record_usage_before_stop(
        worker,
        claimed,
        step_id,
        request_id,
        summary,
        started,
        usage,
        outcome,
        Some(attempt_id),
    )
    .await?;
    worker
        .runs
        .mark_attempt_fenced(
            &claimed.fence,
            &claimed.run_id,
            attempt_id,
            AttemptPhase::Interrupted.as_str(),
            None,
            worker.clock.now_ms(),
        )
        .await?;
    worker
        .pause(
            claimed,
            StopSignalKind::DataIntegrity,
            error,
            Some(step_id.into()),
        )
        .await
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
    let mut obligations_text = String::new();
    if version == "chapter-draft-v2" || version == "chapter-draft-v3" {
        let artifacts = worker.runs.list_artifacts(&claimed.novel_id).await?;
        let context = latest_run_context_payload(
            &artifacts,
            &claimed.contract.target_chapter_id,
            &claimed.run_id,
        )?;
        let prompt_text = context
            .get("promptText")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| AppError::integrity("上下文产物缺少 promptText"))?;
        obligations_text = context
            .get("obligationsText")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        vars.insert("context".into(), prompt_text.to_string());
    }
    if version == "chapter-draft-v3" {
        // v3 把义务块放进模板本身，Prompt 版本与实际内容一致，不再依赖渲染后追加。
        let block = if obligations_text.trim().is_empty() {
            "（本章没有结构化义务；沿用已确认上下文，不得改写已确认事件）".to_string()
        } else {
            obligations_text.clone()
        };
        vars.insert("obligations".into(), block);
    }
    let rendered = novel_prompt::render(asset.template, &vars)
        .map_err(|_| AppError::validation("正文提示词缺少章节上下文"))?;
    if version == "chapter-draft-v2" && !obligations_text.trim().is_empty() {
        return Ok(format!(
            "{rendered}\n\n章节义务（来自冻结章节计划；硬约束必须遵守，创作建议可以合理取舍）：\n{obligations_text}\n如果正文与硬约束冲突，必须调整正文，不能修改计划。"
        ));
    }
    Ok(rendered)
}

fn summarize(prompt: &str) -> String {
    redact_secrets(prompt).chars().take(80).collect()
}

/// 构造一条物理调用记录。
///
/// `outcome` 决定这次调用是否可能已计费；`usage` 只在成功路径才带有数值。
/// 调用结果与用量可见性保持分离，避免把「结果未知」压成「0 计费」。
#[allow(clippy::too_many_arguments)]
fn invocation(
    worker: &Worker,
    claimed: &ClaimedRun,
    step_id: &str,
    request_id: &str,
    summary: &str,
    started: i64,
    usage: &TokenUsage,
    outcome: InvocationOutcome,
    attempt_id: Option<&str>,
) -> NewInvocation {
    let route = claimed
        .contract
        .effective_route(task_type_for_step(step_id));
    let provider = match route {
        crate::provider_service::EffectiveRoute::Step(step) => step.provider_id.clone(),
        crate::provider_service::EffectiveRoute::Contract => claimed.contract.provider.clone(),
    };
    let finished_at = worker.clock.now_ms();
    // 明确的未计费结果不保留任何 token 数字，防止被下游当成真实用量累加。
    // 其余情况以 Provider 实际返回的用量为准：拿到了就是已知，没拿到就是未知。
    let prompt_tokens = usage
        .prompt_tokens
        .map(|value| i64::try_from(value).unwrap_or(i64::MAX));
    let completion_tokens = usage
        .completion_tokens
        .map(|value| i64::try_from(value).unwrap_or(i64::MAX));
    let usage_state = match outcome.usage_state() {
        novel_domain::InvocationUsageState::Unbilled => {
            novel_domain::InvocationUsageState::Unbilled
        }
        _ => novel_domain::InvocationUsageState::for_legacy_row(prompt_tokens, completion_tokens),
    };
    let billable = usage_state != novel_domain::InvocationUsageState::Unbilled;
    NewInvocation {
        id: worker.ids.new_id(),
        request_id: request_id.to_string(),
        run_id: claimed.run_id.clone(),
        step_id: step_id.to_string(),
        provider,
        model: route.model_id(&claimed.contract.model).to_string(),
        input_summary: summary.to_string(),
        started_at: started,
        finished_at,
        prompt_tokens: billable.then_some(prompt_tokens).flatten(),
        completion_tokens: billable.then_some(completion_tokens).flatten(),
        status: outcome.as_str().to_string(),
        attempt_id: attempt_id.map(str::to_string),
        outcome,
        usage_state,
    }
}

/// 把 Provider 错误映射成调用结果分类。
///
/// 只做计费语义归类，不参与重试、暂停或质量政策判断。
fn invocation_outcome_for_error(error: &novel_llm::LlmError) -> InvocationOutcome {
    match error {
        novel_llm::LlmError::RateLimit => InvocationOutcome::RateLimited,
        novel_llm::LlmError::Timeout => InvocationOutcome::TimedOut,
        novel_llm::LlmError::Cancelled => InvocationOutcome::Cancelled,
        novel_llm::LlmError::Unauthorized => InvocationOutcome::Unauthorized,
        // `billed` 是 Provider 层给出的「请求是否可能已经到达服务端」判断。
        novel_llm::LlmError::Unavailable { billed, .. } => {
            if *billed {
                InvocationOutcome::ProviderFailed
            } else {
                InvocationOutcome::Rejected
            }
        }
        novel_llm::LlmError::InvalidResponse(_)
        | novel_llm::LlmError::EndpointRejected(_)
        | novel_llm::LlmError::StructuredOutput(_)
        | novel_llm::LlmError::BudgetExceeded(_) => InvocationOutcome::Rejected,
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
    use super::worker_validation::validate_structured_output;

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

#[cfg(test)]
mod repair_budget_tests {
    use super::*;

    #[test]
    fn repair_budget_has_a_higher_floor_than_draft() {
        assert_eq!(repair_output_budget(Some(8_192)), REPAIR_MIN_OUTPUT_TOKENS);
        assert_eq!(repair_output_budget(Some(32_000)), 32_000);
        assert_eq!(
            repair_output_budget(None),
            DEFAULT_OUTPUT_RESERVE.max(REPAIR_MIN_OUTPUT_TOKENS)
        );
    }

    #[test]
    fn repair_usage_at_or_above_cap_is_treated_as_truncated() {
        let at_cap = TokenUsage {
            prompt_tokens: Some(10),
            completion_tokens: Some(REPAIR_MIN_OUTPUT_TOKENS),
        };
        let below = TokenUsage {
            prompt_tokens: Some(10),
            completion_tokens: Some(REPAIR_MIN_OUTPUT_TOKENS - 1),
        };
        assert!(repair_usage_reached_cap(&at_cap, REPAIR_MIN_OUTPUT_TOKENS));
        assert!(!repair_usage_reached_cap(&below, REPAIR_MIN_OUTPUT_TOKENS));
        assert!(!repair_usage_reached_cap(
            &TokenUsage::unknown(),
            REPAIR_MIN_OUTPUT_TOKENS
        ));
    }
}
