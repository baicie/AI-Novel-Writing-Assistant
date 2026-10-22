use async_trait::async_trait;
use novel_application::{
    CandidateBody, ChapterFullDetail, ClaimedRun, CommandOutcome, ControlCommit, ControlWrite,
    NewArtifact, NewAttempt, NewCandidate, NewInvocation, OpenRunRecord, PortError, RunContract,
    RunSummary, StoredEvent, WorkflowRepository,
};
use novel_domain::{ControlEvent, DomainError, RunControl, apply_event, note_lease, parse_status};
use novel_workflow::{AttemptPhase, AttemptView, ResumeIntent, book_scope};
use serde::de::DeserializeOwned;
use sqlx::Row;
use std::ops::DerefMut;

use crate::db::{SqliteKernel, Tx, insert_event, map_sql, next_seq};
use crate::novels::insert_artifact;

struct ControlRow {
    novel_id: String,
    control: RunControl,
    lease_owner: Option<String>,
    lease_until: Option<i64>,
}

#[async_trait]
impl WorkflowRepository for SqliteKernel {
    async fn open_run(&self, record: OpenRunRecord) -> Result<CommandOutcome, PortError> {
        let mut tx = self.pool.begin().await.map_err(map_sql)?;
        if let Some(existing) =
            lookup_command(&mut tx, &record.command_id, &record.request_hash).await?
        {
            return Ok(existing);
        }
        let active: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM workflow_control WHERE novel_id = ? AND status IN ('queued','running','waiting_approval','paused')",
        )
        .bind(&record.novel_id)
        .fetch_one(tx.deref_mut())
        .await
        .map_err(map_sql)?;
        if active > 0 {
            return Err(PortError::Conflict(
                "这本书已经有一个进行中的创作任务。".into(),
            ));
        }
        sqlx::query("INSERT INTO workflow_run(id, novel_id, plan_version, contract_json, created_at) VALUES (?, ?, ?, ?, ?)")
            .bind(&record.run_id)
            .bind(&record.novel_id)
            .bind(&record.plan_version)
            .bind(&record.contract_json)
            .bind(record.now_ms)
            .execute(tx.deref_mut())
            .await
            .map_err(map_sql)?;
        sqlx::query(
            "INSERT INTO workflow_control(run_id, novel_id, status, version, pause_json, gate_json, cursor_step_id, failure_reason)
             VALUES (?, ?, 'queued', 0, NULL, NULL, NULL, NULL)",
        )
        .bind(&record.run_id)
        .bind(&record.novel_id)
        .execute(tx.deref_mut())
        .await
        .map_err(map_sql)?;
        let seed_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM workflow_artifact WHERE novel_id = ? AND type = 'novel_seed' AND scope = 'book' AND status != 'stale'",
        )
        .bind(&record.novel_id)
        .fetch_one(tx.deref_mut())
        .await
        .map_err(map_sql)?;
        if seed_count == 0 {
            insert_artifact(
                &mut tx,
                &record.seed_artifact_id,
                &record.novel_id,
                Some(&record.run_id),
                "novel_seed",
                book_scope(),
                "confirmed",
                false,
                &record.seed_payload,
                &record.seed_hash,
                record.now_ms,
            )
            .await?;
        }
        let seq = next_seq(&mut tx, &record.run_id).await?;
        insert_event(
            &mut tx,
            &record.run_id,
            seq,
            "run_started",
            None,
            "{}",
            record.now_ms,
        )
        .await?;
        let outcome = CommandOutcome {
            run_id: record.run_id.clone(),
            status: "queued".into(),
            replayed: false,
        };
        insert_command(
            &mut tx,
            &record.command_id,
            &record.run_id,
            "open_run",
            &record.request_hash,
            &outcome,
            record.now_ms,
        )
        .await?;
        tx.commit().await.map_err(map_sql)?;
        Ok(outcome)
    }

    async fn claim_next(
        &self,
        owner: &str,
        now_ms: i64,
        lease_ms: i64,
    ) -> Result<Option<ClaimedRun>, PortError> {
        let rows = sqlx::query(
            "SELECT c.run_id FROM workflow_control c
             JOIN workflow_run r ON r.id = c.run_id
             WHERE c.status IN ('queued','running')
               AND (c.lease_until IS NULL OR c.lease_until < ? OR c.lease_owner = ?)
             ORDER BY r.created_at
             LIMIT 8",
        )
        .bind(now_ms)
        .bind(owner)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sql)?;
        for row in rows {
            let run_id: String = row.try_get("run_id").map_err(map_sql)?;
            let mut tx = self.pool.begin().await.map_err(map_sql)?;
            let Some(current) = load_control(&mut tx, &run_id).await? else {
                continue;
            };
            let expired = current.lease_until.is_none_or(|until| until < now_ms);
            let owned = current.lease_owner.as_deref() == Some(owner);
            if !current.control.status.is_claimable() || (!expired && !owned) {
                continue;
            }
            let next = note_lease(&current.control).map_err(map_domain)?;
            let updated = sqlx::query(
                "UPDATE workflow_control
                 SET status = ?, version = ?, cursor_step_id = ?, lease_owner = ?, lease_until = ?
                 WHERE run_id = ? AND version = ?",
            )
            .bind(next.status.as_str())
            .bind(next.version)
            .bind(&next.cursor_step_id)
            .bind(owner)
            .bind(now_ms + lease_ms)
            .bind(&run_id)
            .bind(current.control.version)
            .execute(tx.deref_mut())
            .await
            .map_err(map_sql)?;
            if updated.rows_affected() != 1 {
                continue;
            }
            let contract = load_contract(&mut tx, &run_id).await?;
            tx.commit().await.map_err(map_sql)?;
            return Ok(Some(ClaimedRun {
                run_id,
                novel_id: current.novel_id,
                contract,
                control: next,
            }));
        }
        Ok(None)
    }

    async fn list_artifacts(
        &self,
        novel_id: &str,
    ) -> Result<Vec<novel_domain::ArtifactView>, PortError> {
        let rows = sqlx::query(
            "SELECT type, scope, version, status, protected_user_content, payload_json
             FROM workflow_artifact WHERE novel_id = ? ORDER BY version",
        )
        .bind(novel_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sql)?;
        let mut artifacts = Vec::new();
        for row in rows {
            let status: String = row.try_get("status").map_err(map_sql)?;
            artifacts.push(novel_domain::ArtifactView {
                artifact_type: row.try_get("type").map_err(map_sql)?,
                scope: row.try_get("scope").map_err(map_sql)?,
                version: row.try_get("version").map_err(map_sql)?,
                status: novel_domain::ArtifactStatus::parse(&status)
                    .map_err(|_| PortError::Integrity("产物状态无法识别。".into()))?,
                protected_user_content: row
                    .try_get::<i64, _>("protected_user_content")
                    .map_err(map_sql)?
                    != 0,
                payload_json: row.try_get("payload_json").map_err(map_sql)?,
            });
        }
        Ok(artifacts)
    }

    async fn list_attempts(&self, run_id: &str) -> Result<Vec<AttemptView>, PortError> {
        let rows = sqlx::query(
            "SELECT id, step_id, phase, candidate_id, request_id, resolution
             FROM workflow_step_attempt WHERE run_id = ? ORDER BY created_at, rowid",
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sql)?;
        let mut attempts = Vec::new();
        for row in rows {
            let phase: String = row.try_get("phase").map_err(map_sql)?;
            let resolution: Option<String> = row.try_get("resolution").map_err(map_sql)?;
            attempts.push(AttemptView {
                id: row.try_get("id").map_err(map_sql)?,
                step_id: row.try_get("step_id").map_err(map_sql)?,
                phase: AttemptPhase::parse(&phase)
                    .ok_or_else(|| PortError::Integrity("步骤尝试状态无法识别。".into()))?,
                candidate_id: row.try_get("candidate_id").map_err(map_sql)?,
                request_id: row.try_get("request_id").map_err(map_sql)?,
                resolution: match resolution {
                    None => None,
                    Some(value) => Some(
                        ResumeIntent::parse(&value)
                            .ok_or_else(|| PortError::Integrity("恢复决定无法识别。".into()))?,
                    ),
                },
            });
        }
        Ok(attempts)
    }

    async fn insert_external_artifact(&self, artifact: NewArtifact) -> Result<(), PortError> {
        let mut tx = self.pool.begin().await.map_err(map_sql)?;
        let protected: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM workflow_artifact
             WHERE novel_id = ? AND type = ? AND scope = ? AND protected_user_content = 1",
        )
        .bind(&artifact.novel_id)
        .bind(&artifact.artifact_type)
        .bind(&artifact.scope)
        .fetch_one(tx.deref_mut())
        .await
        .map_err(map_sql)?;
        if protected > 0 {
            return Err(PortError::Conflict("已保存的正文不能被自动覆盖。".into()));
        }
        insert_artifact(
            &mut tx,
            &artifact.id,
            &artifact.novel_id,
            artifact.run_id.as_deref(),
            &artifact.artifact_type,
            &artifact.scope,
            &artifact.status,
            artifact.protected_user_content,
            &artifact.payload_json,
            &artifact.content_sha256,
            artifact.created_at,
        )
        .await?;
        tx.commit().await.map_err(map_sql)?;
        Ok(())
    }

    async fn start_attempt(&self, attempt: NewAttempt) -> Result<(), PortError> {
        sqlx::query(
            "INSERT INTO workflow_step_attempt(id, run_id, step_id, phase, control_version, created_at, updated_at)
             VALUES (?, ?, ?, 'started', ?, ?, ?)",
        )
        .bind(&attempt.id)
        .bind(&attempt.run_id)
        .bind(&attempt.step_id)
        .bind(attempt.control_version)
        .bind(attempt.created_at)
        .bind(attempt.created_at)
        .execute(&self.pool)
        .await
        .map_err(map_sql)?;
        Ok(())
    }

    async fn mark_remote_started(
        &self,
        attempt_id: &str,
        request_id: &str,
        now_ms: i64,
    ) -> Result<bool, PortError> {
        let result = sqlx::query(
            "UPDATE workflow_step_attempt
             SET phase = 'remote_started', request_id = ?, updated_at = ?
             WHERE id = ? AND phase = 'started' AND request_id IS NULL",
        )
        .bind(request_id)
        .bind(now_ms)
        .bind(attempt_id)
        .execute(&self.pool)
        .await
        .map_err(map_sql)?;
        Ok(result.rows_affected() == 1)
    }

    async fn save_candidate(&self, candidate: NewCandidate) -> Result<(), PortError> {
        let mut tx = self.pool.begin().await.map_err(map_sql)?;
        sqlx::query(
            "INSERT INTO chapter_candidate(id, chapter_id, run_id, step_id, content, content_sha256, interrupted, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, 0, ?, ?)",
        )
        .bind(&candidate.id)
        .bind(&candidate.chapter_id)
        .bind(&candidate.run_id)
        .bind(&candidate.step_id)
        .bind(&candidate.content)
        .bind(&candidate.content_sha256)
        .bind(candidate.now_ms)
        .bind(candidate.now_ms)
        .execute(tx.deref_mut())
        .await
        .map_err(map_sql)?;
        let seq = next_seq(&mut tx, &candidate.run_id).await?;
        let payload = serde_json::json!({ "candidateId": candidate.id }).to_string();
        insert_event(
            &mut tx,
            &candidate.run_id,
            seq,
            "checkpoint_saved",
            Some(&candidate.step_id),
            &payload,
            candidate.now_ms,
        )
        .await?;
        tx.commit().await.map_err(map_sql)?;
        Ok(())
    }

    async fn record_invocation(&self, invocation: NewInvocation) -> Result<(), PortError> {
        let mut tx = self.pool.begin().await.map_err(map_sql)?;
        insert_invocation(&mut tx, &invocation).await?;
        tx.commit().await.map_err(map_sql)?;
        Ok(())
    }

    async fn mark_attempt(
        &self,
        attempt_id: &str,
        phase: &str,
        candidate_id: Option<&str>,
        now_ms: i64,
    ) -> Result<(), PortError> {
        sqlx::query("UPDATE workflow_step_attempt SET phase = ?, candidate_id = COALESCE(?, candidate_id), updated_at = ? WHERE id = ?")
            .bind(phase)
            .bind(candidate_id)
            .bind(now_ms)
            .bind(attempt_id)
            .execute(&self.pool)
            .await
            .map_err(map_sql)?;
        Ok(())
    }

    async fn latest_candidate(&self, run_id: &str) -> Result<Option<CandidateBody>, PortError> {
        let row = sqlx::query("SELECT id, content FROM chapter_candidate WHERE run_id = ? ORDER BY updated_at DESC LIMIT 1")
            .bind(run_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_sql)?;
        match row {
            None => Ok(None),
            Some(row) => Ok(Some(CandidateBody {
                id: row.try_get("id").map_err(map_sql)?,
                content: row.try_get("content").map_err(map_sql)?,
            })),
        }
    }

    async fn apply_control(&self, write: ControlWrite) -> Result<ControlCommit, PortError> {
        let mut tx = self.pool.begin().await.map_err(map_sql)?;
        let current = load_control(&mut tx, &write.run_id)
            .await?
            .ok_or_else(|| PortError::NotFound("没有找到这次创作。".into()))?;
        if current.control.version != write.expected_version {
            return Err(PortError::Conflict(
                "创作进度已经变化，请重新打开后再继续。".into(),
            ));
        }
        let next = apply_event(&current.control, write.event, Some(write.expected_version))
            .map_err(map_domain)?;
        persist_control(
            &mut tx,
            &next,
            &write.run_id,
            current.control.version,
            write.clear_lease,
        )
        .await?;
        if let Some(artifact) = &write.artifact {
            insert_artifact(
                &mut tx,
                &artifact.id,
                &artifact.novel_id,
                artifact.run_id.as_deref(),
                &artifact.artifact_type,
                &artifact.scope,
                &artifact.status,
                artifact.protected_user_content,
                &artifact.payload_json,
                &artifact.content_sha256,
                artifact.created_at,
            )
            .await?;
        }
        let seq = next_seq(&mut tx, &write.run_id).await?;
        insert_event(
            &mut tx,
            &write.run_id,
            seq,
            &write.event_kind,
            write.event_step_id.as_deref(),
            &write.event_payload,
            write.occurred_at,
        )
        .await?;
        if let Some(invocation) = &write.invocation {
            insert_invocation(&mut tx, invocation).await?;
        }
        if let (Some(attempt_id), Some(phase)) = (&write.attempt_id, &write.attempt_phase) {
            sqlx::query("UPDATE workflow_step_attempt SET phase = ?, updated_at = ? WHERE id = ?")
                .bind(phase)
                .bind(write.occurred_at)
                .bind(attempt_id)
                .execute(tx.deref_mut())
                .await
                .map_err(map_sql)?;
        }
        tx.commit().await.map_err(map_sql)?;
        Ok(ControlCommit { control: next, seq })
    }

    async fn approve(
        &self,
        command_id: &str,
        request_hash: &str,
        run_id: &str,
        now_ms: i64,
    ) -> Result<CommandOutcome, PortError> {
        let mut tx = self.pool.begin().await.map_err(map_sql)?;
        if let Some(existing) = lookup_command(&mut tx, command_id, request_hash).await? {
            return Ok(existing);
        }
        let current = load_control(&mut tx, run_id)
            .await?
            .ok_or_else(|| PortError::NotFound("没有找到这次创作。".into()))?;
        if current.control.status != novel_domain::RunStatus::WaitingApproval {
            return Err(PortError::Conflict("这一步还没有到需要确认的时候。".into()));
        }
        let gate = current
            .control
            .gate
            .clone()
            .ok_or_else(|| PortError::Integrity("缺少等待确认的内容。".into()))?;
        for artifact_type in &gate.artifact_types {
            sqlx::query(
                "UPDATE workflow_artifact SET status = 'confirmed'
                 WHERE novel_id = ? AND run_id = ? AND type = ? AND status = 'draft' AND protected_user_content = 0",
            )
            .bind(&current.novel_id)
            .bind(run_id)
            .bind(artifact_type)
            .execute(tx.deref_mut())
            .await
            .map_err(map_sql)?;
        }
        let outcome = transition_command(
            &mut tx,
            &current,
            run_id,
            ControlEvent::ResolveGate,
            "step_succeeded",
            Some(&gate.id),
            "{}",
            command_id,
            request_hash,
            "approve",
            now_ms,
            true,
        )
        .await?;
        tx.commit().await.map_err(map_sql)?;
        Ok(outcome)
    }

    async fn resume(
        &self,
        command_id: &str,
        request_hash: &str,
        run_id: &str,
        intent: &str,
        now_ms: i64,
    ) -> Result<CommandOutcome, PortError> {
        let mut tx = self.pool.begin().await.map_err(map_sql)?;
        if let Some(existing) = lookup_command(&mut tx, command_id, request_hash).await? {
            return Ok(existing);
        }
        let current = load_control(&mut tx, run_id)
            .await?
            .ok_or_else(|| PortError::NotFound("没有找到这次创作。".into()))?;
        if current.control.status != novel_domain::RunStatus::Paused {
            return Err(PortError::Conflict("现在还不能继续这次创作。".into()));
        }
        let intent = ResumeIntent::parse(intent)
            .ok_or_else(|| PortError::Integrity("恢复方式无法识别。".into()))?;
        let attempt = latest_attempt(&mut tx, run_id)
            .await?
            .ok_or_else(|| PortError::Conflict("没有可以确认的候选正文。".into()))?;
        if intent == ResumeIntent::RetryRemote {
            let contract = load_contract(&mut tx, run_id).await?;
            let calls: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM workflow_step_attempt WHERE run_id = ? AND step_id = ? AND request_id IS NOT NULL",
            )
            .bind(run_id)
            .bind(&attempt.step_id)
            .fetch_one(tx.deref_mut())
            .await
            .map_err(map_sql)?;
            if calls > i64::from(contract.max_automatic_retries) {
                return Err(PortError::Conflict(
                    "已经达到自动重试上限，可以接受现有候选或停止这次创作。".into(),
                ));
            }
        }
        sqlx::query("UPDATE workflow_step_attempt SET resolution = ?, phase = 'interrupted', updated_at = ? WHERE id = ?")
            .bind(intent.as_str())
            .bind(now_ms)
            .bind(&attempt.id)
            .execute(tx.deref_mut())
            .await
            .map_err(map_sql)?;
        let outcome = transition_command(
            &mut tx,
            &current,
            run_id,
            ControlEvent::Resume,
            "step_started",
            Some(&attempt.step_id),
            &serde_json::json!({ "intent": intent.as_str() }).to_string(),
            command_id,
            request_hash,
            "resume",
            now_ms,
            true,
        )
        .await?;
        tx.commit().await.map_err(map_sql)?;
        Ok(outcome)
    }

    async fn cancel(
        &self,
        command_id: &str,
        request_hash: &str,
        run_id: &str,
        now_ms: i64,
    ) -> Result<CommandOutcome, PortError> {
        let mut tx = self.pool.begin().await.map_err(map_sql)?;
        if let Some(existing) = lookup_command(&mut tx, command_id, request_hash).await? {
            return Ok(existing);
        }
        let current = load_control(&mut tx, run_id)
            .await?
            .ok_or_else(|| PortError::NotFound("没有找到这次创作。".into()))?;
        let outcome = transition_command(
            &mut tx,
            &current,
            run_id,
            ControlEvent::Cancel,
            "",
            None,
            "{}",
            command_id,
            request_hash,
            "cancel",
            now_ms,
            true,
        )
        .await?;
        tx.commit().await.map_err(map_sql)?;
        Ok(outcome)
    }

    async fn events_after(
        &self,
        run_id: &str,
        after_seq: i64,
    ) -> Result<Vec<StoredEvent>, PortError> {
        let rows = sqlx::query(
            "SELECT run_id, seq, kind, step_id, payload_json, occurred_at FROM workflow_event WHERE run_id = ? AND seq > ? ORDER BY seq",
        )
        .bind(run_id)
        .bind(after_seq)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sql)?;
        rows.into_iter()
            .map(|row| {
                Ok(StoredEvent {
                    run_id: row.try_get("run_id").map_err(map_sql)?,
                    seq: row.try_get("seq").map_err(map_sql)?,
                    kind: row.try_get("kind").map_err(map_sql)?,
                    step_id: row.try_get("step_id").map_err(map_sql)?,
                    payload_json: row.try_get("payload_json").map_err(map_sql)?,
                    occurred_at: row.try_get("occurred_at").map_err(map_sql)?,
                })
            })
            .collect()
    }

    async fn remote_call_count(&self, run_id: &str, step_id: &str) -> Result<i64, PortError> {
        sqlx::query_scalar("SELECT COUNT(*) FROM workflow_step_attempt WHERE run_id = ? AND step_id = ? AND request_id IS NOT NULL")
            .bind(run_id)
            .bind(step_id)
            .fetch_one(&self.pool)
            .await
            .map_err(map_sql)
    }

    async fn append_debt(
        &self,
        id: &str,
        novel_id: &str,
        run_id: &str,
        chapter_order: i32,
        code: &str,
        detail: &str,
        now_ms: i64,
    ) -> Result<(), PortError> {
        sqlx::query("INSERT INTO quality_debt(id, novel_id, run_id, chapter_order, code, detail, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(id)
            .bind(novel_id)
            .bind(run_id)
            .bind(chapter_order)
            .bind(code)
            .bind(detail)
            .bind(now_ms)
            .execute(&self.pool)
            .await
            .map_err(map_sql)?;
        Ok(())
    }

    /// Phase 2.1: 列出某小说的所有运行（按时间倒序）。
    async fn list_runs(&self, novel_id: &str) -> Result<Vec<RunSummary>, PortError> {
        let rows = sqlx::query(
            "SELECT r.id, r.novel_id, r.contract_json, wc.status, r.created_at,
                    COALESCE((SELECT title FROM chapter WHERE id = json_extract(r.contract_json, '$.targetChapterId')), '') as chapter_title,
                    COALESCE((SELECT MAX(occurred_at) FROM workflow_event WHERE run_id = r.id), r.created_at) as updated_at
             FROM workflow_run r
             JOIN workflow_control wc ON r.id = wc.run_id
             WHERE r.novel_id = ?
             ORDER BY r.created_at DESC",
        )
        .bind(novel_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sql)?;

        let mut results = Vec::new();
        for row in rows {
            let contract_json: String = row.try_get("contract_json").map_err(map_sql)?;
            let contract: RunContract = serde_json::from_str(&contract_json)
                .map_err(|_| PortError::Integrity("运行合同无法解析。".into()))?;

            results.push(RunSummary {
                id: row.try_get("id").map_err(map_sql)?,
                novel_id: row.try_get("novel_id").map_err(map_sql)?,
                chapter_id: contract.target_chapter_id.clone(),
                chapter_title: row.try_get("chapter_title").map_err(map_sql)?,
                state: row.try_get::<String, _>("status").map_err(map_sql)?,
                created_at: row.try_get("created_at").map_err(map_sql)?,
                updated_at: row.try_get("updated_at").map_err(map_sql)?,
                provider: contract.provider.clone(),
                model: contract.model.clone(),
            });
        }
        Ok(results)
    }

    /// Phase 2.1: 获取章节的完整详情。
    async fn get_chapter_full(
        &self,
        novel_id: &str,
        chapter_id: &str,
    ) -> Result<ChapterFullDetail, PortError> {
        let row = sqlx::query(
            "SELECT c.id, c.novel_id, c.title, c.sort_order, c.created_at, c.updated_at,
                    COALESCE(cr.content, '') as content
             FROM chapter c
             LEFT JOIN chapter_revision cr ON c.official_revision_id = cr.id
             WHERE c.id = ? AND c.novel_id = ?",
        )
        .bind(chapter_id)
        .bind(novel_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sql)?;

        let row = row.ok_or_else(|| PortError::NotFound("章节不存在。".into()))?;
        Ok(ChapterFullDetail {
            id: row.try_get("id").map_err(map_sql)?,
            novel_id: row.try_get("novel_id").map_err(map_sql)?,
            title: row.try_get("title").map_err(map_sql)?,
            sort_order: row.try_get("sort_order").map_err(map_sql)?,
            content: row.try_get("content").map_err(map_sql)?,
            status: "active".to_string(), // 简化：后续可根据 protection 字段细化
            created_at: row.try_get("created_at").map_err(map_sql)?,
            updated_at: row.try_get("updated_at").map_err(map_sql)?,
        })
    }
}

fn map_domain(error: DomainError) -> PortError {
    match error {
        DomainError::InvalidTransition { .. } => {
            PortError::Conflict("当前状态不能做这个操作。".into())
        }
        DomainError::VersionConflict { .. } => {
            PortError::Conflict("创作进度已经变化，请重新打开后再继续。".into())
        }
        DomainError::ProtectedContent => PortError::Conflict("已保存的正文不能被自动覆盖。".into()),
        DomainError::Invariant(message) => PortError::Integrity(message.into()),
    }
}

async fn load_control(tx: &mut Tx<'_>, run_id: &str) -> Result<Option<ControlRow>, PortError> {
    let row = sqlx::query(
        "SELECT novel_id, status, version, pause_json, gate_json, cursor_step_id, failure_reason, lease_owner, lease_until
         FROM workflow_control WHERE run_id = ?",
    )
    .bind(run_id)
    .fetch_optional(tx.deref_mut())
    .await
    .map_err(map_sql)?;
    let Some(row) = row else { return Ok(None) };
    let status_text: String = row.try_get("status").map_err(map_sql)?;
    Ok(Some(ControlRow {
        novel_id: row.try_get("novel_id").map_err(map_sql)?,
        control: RunControl {
            version: row.try_get("version").map_err(map_sql)?,
            status: parse_status(&status_text)
                .map_err(|_| PortError::Integrity("运行状态无法识别。".into()))?,
            pause: json_opt(row.try_get("pause_json").map_err(map_sql)?)?,
            gate: json_opt(row.try_get("gate_json").map_err(map_sql)?)?,
            cursor_step_id: row.try_get("cursor_step_id").map_err(map_sql)?,
            failure_reason: row.try_get("failure_reason").map_err(map_sql)?,
        },
        lease_owner: row.try_get("lease_owner").map_err(map_sql)?,
        lease_until: row.try_get("lease_until").map_err(map_sql)?,
    }))
}

async fn load_contract(
    tx: &mut Tx<'_>,
    run_id: &str,
) -> Result<novel_application::RunContract, PortError> {
    let json: String = sqlx::query_scalar("SELECT contract_json FROM workflow_run WHERE id = ?")
        .bind(run_id)
        .fetch_one(tx.deref_mut())
        .await
        .map_err(map_sql)?;
    serde_json::from_str(&json)
        .map_err(|_| PortError::Integrity("创作合同无法读取，不能改用别的计划继续。".into()))
}

async fn persist_control(
    tx: &mut Tx<'_>,
    next: &RunControl,
    run_id: &str,
    expected: i64,
    clear_lease: bool,
) -> Result<(), PortError> {
    let pause = next
        .pause
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|_| PortError::Integrity("暂停原因无法保存。".into()))?;
    let gate = next
        .gate
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|_| PortError::Integrity("确认点无法保存。".into()))?;
    let updated = sqlx::query(
        "UPDATE workflow_control
         SET status = ?, version = ?, pause_json = ?, gate_json = ?, cursor_step_id = ?, failure_reason = ?,
             lease_owner = CASE WHEN ? THEN NULL ELSE lease_owner END,
             lease_until = CASE WHEN ? THEN NULL ELSE lease_until END
         WHERE run_id = ? AND version = ?",
    )
    .bind(next.status.as_str())
    .bind(next.version)
    .bind(pause)
    .bind(gate)
    .bind(&next.cursor_step_id)
    .bind(&next.failure_reason)
    .bind(i64::from(u8::from(clear_lease)))
    .bind(i64::from(u8::from(clear_lease)))
    .bind(run_id)
    .bind(expected)
    .execute(tx.deref_mut())
    .await
    .map_err(map_sql)?;
    if updated.rows_affected() != 1 {
        return Err(PortError::Conflict(
            "创作进度已经变化，请重新打开后再继续。".into(),
        ));
    }
    Ok(())
}

async fn lookup_command(
    tx: &mut Tx<'_>,
    command_id: &str,
    request_hash: &str,
) -> Result<Option<CommandOutcome>, PortError> {
    let row =
        sqlx::query("SELECT request_hash, result_json FROM workflow_command WHERE command_id = ?")
            .bind(command_id)
            .fetch_optional(tx.deref_mut())
            .await
            .map_err(map_sql)?;
    let Some(row) = row else { return Ok(None) };
    let stored_hash: String = row.try_get("request_hash").map_err(map_sql)?;
    if stored_hash != request_hash {
        return Err(PortError::Conflict(
            "这条操作已经执行过，但不能用不同的内容重放。".into(),
        ));
    }
    let result: String = row.try_get("result_json").map_err(map_sql)?;
    let mut outcome: CommandOutcome = serde_json::from_str(&result)
        .map_err(|_| PortError::Integrity("命令结果无法读取。".into()))?;
    outcome.replayed = true;
    Ok(Some(outcome))
}

async fn insert_command(
    tx: &mut Tx<'_>,
    command_id: &str,
    run_id: &str,
    kind: &str,
    request_hash: &str,
    outcome: &CommandOutcome,
    now_ms: i64,
) -> Result<(), PortError> {
    let result = serde_json::to_string(outcome)
        .map_err(|_| PortError::Integrity("命令结果无法保存。".into()))?;
    sqlx::query("INSERT INTO workflow_command(command_id, run_id, kind, request_hash, result_json, created_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(command_id)
        .bind(run_id)
        .bind(kind)
        .bind(request_hash)
        .bind(result)
        .bind(now_ms)
        .execute(tx.deref_mut())
        .await
        .map_err(map_sql)?;
    Ok(())
}

async fn insert_invocation(tx: &mut Tx<'_>, invocation: &NewInvocation) -> Result<(), PortError> {
    sqlx::query(
        "INSERT INTO llm_invocation(id, request_id, run_id, step_id, provider, model, input_summary, started_at, finished_at, prompt_tokens, completion_tokens, status)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&invocation.id)
    .bind(&invocation.request_id)
    .bind(&invocation.run_id)
    .bind(&invocation.step_id)
    .bind(&invocation.provider)
    .bind(&invocation.model)
    .bind(&invocation.input_summary)
    .bind(invocation.started_at)
    .bind(invocation.finished_at)
    .bind(invocation.prompt_tokens)
    .bind(invocation.completion_tokens)
    .bind(&invocation.status)
    .execute(tx.deref_mut())
    .await
    .map_err(map_sql)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn transition_command(
    tx: &mut Tx<'_>,
    current: &ControlRow,
    run_id: &str,
    event: ControlEvent,
    event_kind: &str,
    step_id: Option<&str>,
    payload: &str,
    command_id: &str,
    request_hash: &str,
    command_kind: &str,
    now_ms: i64,
    clear_lease: bool,
) -> Result<CommandOutcome, PortError> {
    let next =
        apply_event(&current.control, event, Some(current.control.version)).map_err(map_domain)?;
    persist_control(tx, &next, run_id, current.control.version, clear_lease).await?;
    if !event_kind.is_empty() {
        let seq = next_seq(tx, run_id).await?;
        insert_event(tx, run_id, seq, event_kind, step_id, payload, now_ms).await?;
    }
    let outcome = CommandOutcome {
        run_id: run_id.to_string(),
        status: next.status.as_str().to_string(),
        replayed: false,
    };
    insert_command(
        tx,
        command_id,
        run_id,
        command_kind,
        request_hash,
        &outcome,
        now_ms,
    )
    .await?;
    Ok(outcome)
}

async fn latest_attempt(tx: &mut Tx<'_>, run_id: &str) -> Result<Option<AttemptView>, PortError> {
    let row = sqlx::query(
        "SELECT id, step_id, phase, candidate_id, request_id, resolution
         FROM workflow_step_attempt WHERE run_id = ? ORDER BY created_at DESC, rowid DESC LIMIT 1",
    )
    .bind(run_id)
    .fetch_optional(tx.deref_mut())
    .await
    .map_err(map_sql)?;
    let Some(row) = row else { return Ok(None) };
    let phase: String = row.try_get("phase").map_err(map_sql)?;
    let resolution: Option<String> = row.try_get("resolution").map_err(map_sql)?;
    Ok(Some(AttemptView {
        id: row.try_get("id").map_err(map_sql)?,
        step_id: row.try_get("step_id").map_err(map_sql)?,
        phase: AttemptPhase::parse(&phase)
            .ok_or_else(|| PortError::Integrity("步骤尝试状态无法识别。".into()))?,
        candidate_id: row.try_get("candidate_id").map_err(map_sql)?,
        request_id: row.try_get("request_id").map_err(map_sql)?,
        resolution: match resolution {
            None => None,
            Some(value) => Some(
                ResumeIntent::parse(&value)
                    .ok_or_else(|| PortError::Integrity("恢复决定无法识别。".into()))?,
            ),
        },
    }))
}

fn json_opt<T: DeserializeOwned>(value: Option<String>) -> Result<Option<T>, PortError> {
    match value {
        None => Ok(None),
        Some(text) => serde_json::from_str(&text)
            .map(Some)
            .map_err(|_| PortError::Integrity("运行控制记录无法读取。".into())),
    }
}
