use std::ops::DerefMut;

use async_trait::async_trait;
use novel_application::{
    MemoryCandidateView, MemoryConflictView, MemoryExtractionJobView, MemoryRepository,
    NewMemoryCandidate, NewMemoryJob, PortError,
};
use sqlx::Row;

use crate::db::{SqliteKernel, map_sql};

#[async_trait]
impl MemoryRepository for SqliteKernel {
    async fn memory_job_by_command(
        &self,
        command_id: &str,
    ) -> Result<Option<MemoryExtractionJobView>, PortError> {
        let row = sqlx::query(
            "SELECT id, command_id, novel_id, chapter_id, chapter_version, status, error, created_at
             FROM memory_extraction_job WHERE command_id = ?",
        ).bind(command_id).fetch_optional(&self.pool).await.map_err(map_sql)?;
        row.as_ref().map(job_from_row).transpose()
    }

    async fn create_memory_job(
        &self,
        job: NewMemoryJob,
        candidates: &[NewMemoryCandidate],
    ) -> Result<MemoryExtractionJobView, PortError> {
        let mut tx = self.pool.begin().await.map_err(map_sql)?;
        sqlx::query(
            "INSERT INTO memory_extraction_job
               (id, command_id, novel_id, chapter_id, chapter_version, source_content_hash,
                extraction_version, provider, model, endpoint, status, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'succeeded', ?, ?)",
        )
        .bind(&job.id)
        .bind(&job.command_id)
        .bind(&job.novel_id)
        .bind(&job.chapter_id)
        .bind(job.chapter_version)
        .bind(&job.source_content_hash)
        .bind(&job.extraction_version)
        .bind(&job.provider)
        .bind(&job.model)
        .bind(&job.endpoint)
        .bind(job.now_ms)
        .bind(job.now_ms)
        .execute(tx.deref_mut())
        .await
        .map_err(map_sql)?;
        for candidate in candidates {
            sqlx::query(
                "INSERT INTO memory_extraction_candidate
                   (id, job_id, novel_id, chapter_id, chapter_version, source_content_hash,
                    fact_type, truthfulness, entity_name, entity_id, value, evidence_quote,
                    evidence_start, confidence, status, ambiguous, version, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'proposed', ?, 0, ?, ?)",
            )
            .bind(&candidate.id)
            .bind(&candidate.job_id)
            .bind(&candidate.novel_id)
            .bind(&candidate.chapter_id)
            .bind(candidate.chapter_version)
            .bind(&candidate.source_content_hash)
            .bind(&candidate.fact_type)
            .bind(&candidate.truthfulness)
            .bind(&candidate.entity_name)
            .bind(&candidate.entity_id)
            .bind(&candidate.value)
            .bind(&candidate.evidence_quote)
            .bind(candidate.evidence_start)
            .bind(candidate.confidence)
            .bind(i64::from(u8::from(candidate.ambiguous)))
            .bind(candidate.now_ms)
            .bind(candidate.now_ms)
            .execute(tx.deref_mut())
            .await
            .map_err(map_sql)?;
        }
        tx.commit().await.map_err(map_sql)?;
        Ok(MemoryExtractionJobView {
            id: job.id,
            command_id: job.command_id,
            novel_id: job.novel_id,
            chapter_id: job.chapter_id,
            chapter_version: job.chapter_version,
            status: "succeeded".into(),
            error: None,
            created_at: job.now_ms,
        })
    }

    async fn fail_memory_job(
        &self,
        job_id: &str,
        error: &str,
        now_ms: i64,
    ) -> Result<(), PortError> {
        sqlx::query(
            "UPDATE memory_extraction_job SET status = 'failed', error = ?, updated_at = ? WHERE id = ?",
        ).bind(error).bind(now_ms).bind(job_id).execute(&self.pool).await.map_err(map_sql)?;
        Ok(())
    }

    async fn list_memory_candidates(
        &self,
        novel_id: &str,
        chapter_id: &str,
    ) -> Result<Vec<MemoryCandidateView>, PortError> {
        let rows = sqlx::query(
            "SELECT id, job_id, novel_id, chapter_id, chapter_version, source_content_hash, fact_type, truthfulness,
                    entity_name, entity_id, value, evidence_quote, evidence_start, confidence,
                    status, ambiguous, version, decision_reason, created_at, updated_at
             FROM memory_extraction_candidate
             WHERE novel_id = ? AND chapter_id = ?
             ORDER BY created_at DESC, id",
        )
        .bind(novel_id)
        .bind(chapter_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sql)?;
        rows.iter().map(candidate_from_row).collect()
    }

    async fn confirm_memory_candidate(
        &self,
        candidate_id: &str,
        expected_version: i64,
        now_ms: i64,
    ) -> Result<MemoryCandidateView, PortError> {
        let mut tx = self.pool.begin().await.map_err(map_sql)?;
        let row = sqlx::query(
            "SELECT id, job_id, novel_id, chapter_id, chapter_version, source_content_hash,
                    fact_type, truthfulness, entity_name, entity_id, value, evidence_quote,
                    evidence_start, confidence, status, ambiguous, version, decision_reason,
                    created_at, updated_at
             FROM memory_extraction_candidate WHERE id = ?",
        )
        .bind(candidate_id)
        .fetch_optional(tx.deref_mut())
        .await
        .map_err(map_sql)?
        .ok_or_else(|| PortError::NotFound("没有找到这条记忆候选。".into()))?;
        let candidate = candidate_from_row(&row)?;
        if candidate.version != expected_version {
            return Err(PortError::Conflict(
                "记忆候选已经变化，请刷新后再确认。".into(),
            ));
        }
        if !matches!(candidate.status.as_str(), "proposed" | "deferred") {
            return Err(PortError::Conflict(
                "记忆候选已经处理，不能重复确认。".into(),
            ));
        }
        if candidate.ambiguous
            || candidate.entity_id.is_none() && candidate.fact_type == "character_state"
        {
            return Err(PortError::Conflict(
                "人物身份存在歧义，需要人工指定后再确认。".into(),
            ));
        }
        let current = sqlx::query(
            "SELECT r.version AS version, r.content AS content FROM chapter c
             JOIN chapter_revision r ON r.id = c.official_revision_id WHERE c.id = ?",
        )
        .bind(&candidate.chapter_id)
        .fetch_optional(tx.deref_mut())
        .await
        .map_err(map_sql)?
        .ok_or_else(|| PortError::Conflict("来源章节没有正式正文。".into()))?;
        let version: i64 = current.try_get("version").map_err(map_sql)?;
        if version != candidate.chapter_version {
            return Err(PortError::Conflict(
                "来源章节已经被修改，候选事实已过期。".into(),
            ));
        }
        let content: String = current.try_get("content").map_err(map_sql)?;
        use sha2::{Digest, Sha256};
        if hex::encode(Sha256::digest(content.as_bytes())) != candidate.source_content_hash {
            return Err(PortError::Conflict(
                "来源正文哈希不一致，候选事实已过期。".into(),
            ));
        }
        let chapter_order: i32 = sqlx::query_scalar("SELECT sort_order FROM chapter WHERE id = ?")
            .bind(&candidate.chapter_id)
            .fetch_one(tx.deref_mut())
            .await
            .map_err(map_sql)?;
        let fact_status = if matches!(
            candidate.truthfulness.as_str(),
            "narrator_fact" | "author_asserted"
        ) {
            "confirmed"
        } else {
            "disputed"
        };
        let fact_id = format!("memory-{}", candidate.id);
        if candidate.fact_type == "character_state"
            && let Some(entity_id) = candidate.entity_id.as_deref()
        {
            let conflict: Option<String> = sqlx::query_scalar(
                "SELECT body FROM character_state
                 WHERE character_id = ? AND chapter_order = ? AND status = 'confirmed' AND body != ?
                 LIMIT 1",
            )
            .bind(entity_id)
            .bind(chapter_order)
            .bind(&candidate.value)
            .fetch_optional(tx.deref_mut())
            .await
            .map_err(map_sql)?;
            if conflict.is_some() {
                return Err(PortError::Conflict(
                    "同一人物在同一剧情位置存在不兼容状态，需人工裁决后再确认。".into(),
                ));
            }
        }
        match candidate.fact_type.as_str() {
            "character_state" => {
                sqlx::query(
                    "INSERT INTO character_state
                       (id, character_id, chapter_order, body, created_at, source_revision, status,
                        updated_at, source_chapter_id)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                     ON CONFLICT(id) DO UPDATE SET body = excluded.body, chapter_order = excluded.chapter_order,
                       source_revision = excluded.source_revision, status = excluded.status,
                       updated_at = excluded.updated_at, source_chapter_id = excluded.source_chapter_id",
                ).bind(&fact_id).bind(candidate.entity_id.as_deref().unwrap_or(""))
                .bind(chapter_order).bind(&candidate.value).bind(now_ms)
                .bind(format!("memory:{}", candidate.chapter_version)).bind(fact_status)
                .bind(now_ms).bind(&candidate.chapter_id)
                .execute(tx.deref_mut()).await.map_err(map_sql)?;
            }
            "timeline_event" => {
                sqlx::query(
                    "INSERT INTO timeline_event
                       (id, novel_id, chapter_order, body, created_at, source_revision, status,
                        updated_at, source_chapter_id)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                     ON CONFLICT(id) DO UPDATE SET body = excluded.body, chapter_order = excluded.chapter_order,
                       source_revision = excluded.source_revision, status = excluded.status,
                       updated_at = excluded.updated_at, source_chapter_id = excluded.source_chapter_id",
                ).bind(&fact_id).bind(&candidate.novel_id).bind(chapter_order)
                .bind(&candidate.value).bind(now_ms)
                .bind(format!("memory:{}", candidate.chapter_version)).bind(fact_status)
                .bind(now_ms).bind(&candidate.chapter_id)
                .execute(tx.deref_mut()).await.map_err(map_sql)?;
            }
            "plot_thread" => {
                let title = candidate
                    .entity_name
                    .clone()
                    .unwrap_or_else(|| "未命名伏笔".into());
                sqlx::query(
                    "INSERT INTO plot_thread
                       (id, novel_id, title, body, opened_chapter_order, resolved_chapter_order,
                        status, source_revision, evidence_json, created_at, updated_at, source_chapter_id)
                     VALUES (?, ?, ?, ?, ?, NULL, ?, ?, ?, ?, ?, ?)
                     ON CONFLICT(id) DO UPDATE SET title = excluded.title, body = excluded.body,
                       opened_chapter_order = excluded.opened_chapter_order, status = excluded.status,
                       source_revision = excluded.source_revision, updated_at = excluded.updated_at,
                       source_chapter_id = excluded.source_chapter_id",
                ).bind(&fact_id).bind(&candidate.novel_id).bind(title).bind(&candidate.value)
                .bind(chapter_order).bind(if fact_status == "confirmed" { "open" } else { "disputed" })
                .bind(format!("memory:{}", candidate.chapter_version)).bind(&candidate.evidence_quote)
                .bind(now_ms).bind(now_ms).bind(&candidate.chapter_id)
                .execute(tx.deref_mut()).await.map_err(map_sql)?;
            }
            other => return Err(PortError::Integrity(format!("未知记忆类型：{other}"))),
        }
        sqlx::query(
            "UPDATE memory_extraction_candidate SET status = 'confirmed', version = version + 1,
                    updated_at = ? WHERE id = ? AND version = ?",
        )
        .bind(now_ms)
        .bind(candidate_id)
        .bind(expected_version)
        .execute(tx.deref_mut())
        .await
        .map_err(map_sql)?;
        tx.commit().await.map_err(map_sql)?;
        let after = sqlx::query(
            "SELECT id, job_id, novel_id, chapter_id, chapter_version, source_content_hash, fact_type, truthfulness,
                    entity_name, entity_id, value, evidence_quote, evidence_start, confidence,
                    status, ambiguous, version, decision_reason, created_at, updated_at
             FROM memory_extraction_candidate WHERE id = ?",
        )
        .bind(candidate_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sql)?;
        candidate_from_row(&after)
    }

    async fn reject_memory_candidate(
        &self,
        candidate_id: &str,
        expected_version: i64,
        reason: &str,
        now_ms: i64,
    ) -> Result<MemoryCandidateView, PortError> {
        self.decide_candidate(
            candidate_id,
            expected_version,
            "rejected",
            Some(reason),
            now_ms,
        )
        .await
    }

    async fn defer_memory_candidate(
        &self,
        candidate_id: &str,
        expected_version: i64,
        now_ms: i64,
    ) -> Result<MemoryCandidateView, PortError> {
        self.decide_candidate(candidate_id, expected_version, "deferred", None, now_ms)
            .await
    }

    async fn update_memory_candidate_value(
        &self,
        candidate_id: &str,
        expected_version: i64,
        value: &str,
        now_ms: i64,
    ) -> Result<MemoryCandidateView, PortError> {
        let result = sqlx::query(
            "UPDATE memory_extraction_candidate SET value = ?, version = version + 1, updated_at = ?
             WHERE id = ? AND version = ? AND status IN ('proposed','deferred')",
        ).bind(value).bind(now_ms).bind(candidate_id).bind(expected_version)
        .execute(&self.pool).await.map_err(map_sql)?;
        if result.rows_affected() != 1 {
            return Err(PortError::Conflict(
                "记忆候选已经变化，请刷新后再编辑。".into(),
            ));
        }
        let row = sqlx::query(
            "SELECT id, job_id, novel_id, chapter_id, chapter_version, source_content_hash, fact_type, truthfulness,
                    entity_name, entity_id, value, evidence_quote, evidence_start, confidence,
                    status, ambiguous, version, decision_reason, created_at, updated_at
             FROM memory_extraction_candidate WHERE id = ?",
        )
        .bind(candidate_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sql)?;
        candidate_from_row(&row)
    }

    async fn sync_memory_conflicts(
        &self,
        novel_id: &str,
        now_ms: i64,
    ) -> Result<Vec<MemoryConflictView>, PortError> {
        let groups = sqlx::query(
            "SELECT c.chapter_id, c.fact_type,
                    COALESCE(c.entity_id, c.entity_name, '') AS entity_key,
                    MIN(ch.sort_order) AS chapter_order,
                    GROUP_CONCAT(c.id) AS candidate_ids,
                    GROUP_CONCAT(c.value, ' || ') AS values_text
             FROM memory_extraction_candidate c
             JOIN chapter ch ON ch.id = c.chapter_id
             WHERE c.novel_id = ? AND c.status IN ('proposed','deferred')
             GROUP BY c.chapter_id, c.fact_type, entity_key
             HAVING COUNT(DISTINCT c.value) > 1",
        )
        .bind(novel_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sql)?;
        for row in &groups {
            let chapter_id: String = row.try_get("chapter_id").map_err(map_sql)?;
            let fact_type: String = row.try_get("fact_type").map_err(map_sql)?;
            let entity_key: String = row.try_get("entity_key").map_err(map_sql)?;
            let chapter_order: i32 = row.try_get("chapter_order").map_err(map_sql)?;
            let ids: String = row.try_get("candidate_ids").map_err(map_sql)?;
            let values: String = row.try_get("values_text").map_err(map_sql)?;
            let conflict_id = conflict_id(
                "candidate",
                novel_id,
                &chapter_id,
                &fact_type,
                &entity_key,
                &ids,
            );
            sqlx::query(
                "INSERT INTO memory_conflict
                   (id, novel_id, conflict_type, entity_key, chapter_id, chapter_order,
                    candidate_ids_json, fact_ids_json, status, version, description, created_at, updated_at)
                 VALUES (?, ?, 'candidate_value', ?, ?, ?, ?, '[]', 'open', 0, ?, ?, ?)
                 ON CONFLICT(id) DO NOTHING",
            ).bind(&conflict_id).bind(novel_id).bind(&entity_key).bind(&chapter_id)
            .bind(chapter_order).bind(serde_json::json!(ids.split(',').collect::<Vec<_>>()).to_string())
            .bind(format!("{fact_type} 在同一章节存在不一致候选：{values}"))
            .bind(now_ms).bind(now_ms).execute(&self.pool).await.map_err(map_sql)?;
        }
        let stale = sqlx::query(
            "SELECT c.chapter_id, c.fact_type, COALESCE(c.entity_id, c.entity_name, '') AS entity_key,
                    MIN(ch.sort_order) AS chapter_order, GROUP_CONCAT(c.id) AS candidate_ids,
                    GROUP_CONCAT(c.value, ' || ') AS values_text
             FROM memory_extraction_candidate c
             JOIN chapter ch ON ch.id = c.chapter_id
             JOIN chapter_revision r ON r.id = ch.official_revision_id
             WHERE c.novel_id = ? AND c.status IN ('proposed','deferred')
               AND c.chapter_version != r.version
             GROUP BY c.chapter_id, c.fact_type, entity_key",
        ).bind(novel_id).fetch_all(&self.pool).await.map_err(map_sql)?;
        for row in &stale {
            let chapter_id: String = row.try_get("chapter_id").map_err(map_sql)?;
            let fact_type: String = row.try_get("fact_type").map_err(map_sql)?;
            let entity_key: String = row.try_get("entity_key").map_err(map_sql)?;
            let chapter_order: i32 = row.try_get("chapter_order").map_err(map_sql)?;
            let ids: String = row.try_get("candidate_ids").map_err(map_sql)?;
            let values: String = row.try_get("values_text").map_err(map_sql)?;
            let conflict_id = conflict_id(
                "stale",
                novel_id,
                &chapter_id,
                &fact_type,
                &entity_key,
                &ids,
            );
            sqlx::query(
                "INSERT INTO memory_conflict
                   (id, novel_id, conflict_type, entity_key, chapter_id, chapter_order,
                    candidate_ids_json, fact_ids_json, status, version, description, created_at, updated_at)
                 VALUES (?, ?, 'stale_revision', ?, ?, ?, ?, '[]', 'open', 0, ?, ?, ?)
                 ON CONFLICT(id) DO NOTHING",
            ).bind(&conflict_id).bind(novel_id).bind(&entity_key).bind(&chapter_id)
            .bind(chapter_order).bind(serde_json::json!(ids.split(',').collect::<Vec<_>>()).to_string())
            .bind(format!("{fact_type} 的候选来自旧章节修订：{values}"))
            .bind(now_ms).bind(now_ms).execute(&self.pool).await.map_err(map_sql)?;
        }
        self.list_memory_conflicts(novel_id).await
    }

    async fn list_memory_conflicts(
        &self,
        novel_id: &str,
    ) -> Result<Vec<MemoryConflictView>, PortError> {
        let rows = sqlx::query(
            "SELECT id, novel_id, conflict_type, entity_key, chapter_id, chapter_order,
                    candidate_ids_json, fact_ids_json, status, version, description, created_at, updated_at
             FROM memory_conflict WHERE novel_id = ? ORDER BY status = 'open' DESC, created_at DESC, id",
        ).bind(novel_id).fetch_all(&self.pool).await.map_err(map_sql)?;
        rows.iter().map(conflict_from_row).collect()
    }

    async fn get_memory_conflict(
        &self,
        conflict_id: &str,
    ) -> Result<MemoryConflictView, PortError> {
        let row = sqlx::query(
            "SELECT id, novel_id, conflict_type, entity_key, chapter_id, chapter_order,
                    candidate_ids_json, fact_ids_json, status, version, description, created_at, updated_at
             FROM memory_conflict WHERE id = ?",
        ).bind(conflict_id).fetch_optional(&self.pool).await.map_err(map_sql)?
            .ok_or_else(|| PortError::NotFound("没有找到这条记忆冲突。".into()))?;
        conflict_from_row(&row)
    }

    async fn memory_conflict_decision_by_command(
        &self,
        command_id: &str,
    ) -> Result<Option<String>, PortError> {
        sqlx::query_scalar("SELECT conflict_id FROM memory_conflict_decision WHERE command_id = ?")
            .bind(command_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_sql)
    }

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
    ) -> Result<MemoryConflictView, PortError> {
        let mut tx = self.pool.begin().await.map_err(map_sql)?;
        let row = sqlx::query(
            "SELECT id, novel_id, conflict_type, entity_key, chapter_id, chapter_order,
                    candidate_ids_json, fact_ids_json, status, version, description, created_at, updated_at
             FROM memory_conflict WHERE id = ?",
        ).bind(conflict_id).fetch_optional(tx.deref_mut()).await.map_err(map_sql)?
            .ok_or_else(|| PortError::NotFound("没有找到这条记忆冲突。".into()))?;
        let conflict = conflict_from_row(&row)?;
        if let Some((existing_conflict, existing_hash)) = sqlx::query_as::<_, (String, String)>(
            "SELECT conflict_id, request_hash FROM memory_conflict_decision WHERE command_id = ?",
        )
        .bind(command_id)
        .fetch_optional(tx.deref_mut())
        .await
        .map_err(map_sql)?
        {
            if existing_conflict == conflict_id && existing_hash == request_hash {
                tx.commit().await.map_err(map_sql)?;
                return self.get_memory_conflict(conflict_id).await;
            }
            return Err(PortError::Conflict(
                "commandId 已用于不同的冲突裁决请求。".into(),
            ));
        }
        if conflict.status != "open" {
            return Err(PortError::Conflict("这条冲突已经裁决。".into()));
        }
        if conflict.version != expected_version {
            return Err(PortError::Conflict("冲突已经变化，请刷新后再裁决。".into()));
        }
        let chosen = if conflict.candidate_ids.is_empty() {
            if conflict.fact_ids.is_empty() {
                return Err(PortError::Integrity("冲突没有可裁决事实。".into()));
            }
            resolve_structured_fact_conflict(&mut tx, &conflict, decision, now_ms).await?;
            None
        } else {
            let chosen = chosen_candidate_id
                .or_else(|| conflict.candidate_ids.first().map(String::as_str))
                .ok_or_else(|| PortError::Integrity("冲突没有可裁决候选。".into()))?;
            if !conflict.candidate_ids.iter().any(|id| id == chosen) {
                return Err(PortError::Integrity("裁决候选不属于这条冲突。".into()));
            }
            Some(chosen)
        };
        if let Some(chosen) = chosen {
            let candidate = load_candidate(&mut tx, chosen).await?;
            match decision {
                "confirm_new" => {
                    validate_candidate_source(&mut tx, &candidate, false).await?;
                    let chapter_order = chapter_order_for(&mut tx, &candidate.chapter_id).await?;
                    write_candidate_fact(&mut tx, &candidate, chapter_order, now_ms).await?;
                    set_candidate_status(&mut tx, &candidate, "confirmed", Some(reason), now_ms)
                        .await?;
                    reject_other_candidates(
                        &mut tx,
                        &conflict.candidate_ids,
                        &candidate.id,
                        now_ms,
                    )
                    .await?;
                }
                "supersede_with_evidence" => {
                    if conflict.fact_ids.is_empty() {
                        return Err(PortError::Conflict(
                            "替代裁决必须关联至少一个旧事实。".into(),
                        ));
                    }
                    validate_candidate_source(&mut tx, &candidate, true).await?;
                    let chapter_order = chapter_order_for(&mut tx, &candidate.chapter_id).await?;
                    write_candidate_fact(&mut tx, &candidate, chapter_order, now_ms).await?;
                    set_candidate_status(&mut tx, &candidate, "confirmed", Some(reason), now_ms)
                        .await?;
                    reject_other_candidates(
                        &mut tx,
                        &conflict.candidate_ids,
                        &candidate.id,
                        now_ms,
                    )
                    .await?;
                    supersede_old_facts(&mut tx, &conflict.novel_id, &conflict.fact_ids, now_ms)
                        .await?;
                }
                "keep_existing" | "reject_extraction" => {
                    reject_candidates(&mut tx, &conflict.candidate_ids, reason, now_ms).await?;
                }
                "mark_disputed" => {
                    defer_candidates(&mut tx, &conflict.candidate_ids, reason, now_ms).await?;
                }
                _ => return Err(PortError::Integrity("未知冲突裁决动作。".into())),
            }
        }
        sqlx::query(
            "INSERT INTO memory_conflict_decision
               (id, conflict_id, command_id, request_hash, decision, chosen_candidate_id, reason,
                operator, conflict_version, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(format!("decision-{command_id}"))
        .bind(conflict_id)
        .bind(command_id)
        .bind(request_hash)
        .bind(decision)
        .bind(chosen)
        .bind(reason)
        .bind(operator)
        .bind(expected_version)
        .bind(now_ms)
        .execute(tx.deref_mut())
        .await
        .map_err(map_sql)?;
        let updated = sqlx::query(
            "UPDATE memory_conflict SET status = 'resolved', version = version + 1, updated_at = ?
             WHERE id = ? AND version = ? AND status = 'open'",
        )
        .bind(now_ms)
        .bind(conflict_id)
        .bind(expected_version)
        .execute(tx.deref_mut())
        .await
        .map_err(map_sql)?;
        if updated.rows_affected() != 1 {
            return Err(PortError::Conflict("冲突已经变化，裁决未写入。".into()));
        }
        tx.commit().await.map_err(map_sql)?;
        self.get_memory_conflict(conflict_id).await
    }
}

impl SqliteKernel {
    async fn decide_candidate(
        &self,
        candidate_id: &str,
        expected_version: i64,
        status: &str,
        reason: Option<&str>,
        now_ms: i64,
    ) -> Result<MemoryCandidateView, PortError> {
        let result = sqlx::query(
            "UPDATE memory_extraction_candidate
             SET status = ?, decision_reason = ?, version = version + 1, updated_at = ?
             WHERE id = ? AND version = ? AND status IN ('proposed','deferred')",
        )
        .bind(status)
        .bind(reason)
        .bind(now_ms)
        .bind(candidate_id)
        .bind(expected_version)
        .execute(&self.pool)
        .await
        .map_err(map_sql)?;
        if result.rows_affected() != 1 {
            return Err(PortError::Conflict(
                "记忆候选已经变化，请刷新后再处理。".into(),
            ));
        }
        let row = sqlx::query(
            "SELECT id, job_id, novel_id, chapter_id, chapter_version, source_content_hash, fact_type, truthfulness,
                    entity_name, entity_id, value, evidence_quote, evidence_start, confidence,
                    status, ambiguous, version, decision_reason, created_at, updated_at
             FROM memory_extraction_candidate WHERE id = ?",
        )
        .bind(candidate_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sql)?;
        candidate_from_row(&row)
    }
}

fn job_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<MemoryExtractionJobView, PortError> {
    Ok(MemoryExtractionJobView {
        id: row.try_get("id").map_err(map_sql)?,
        command_id: row.try_get("command_id").map_err(map_sql)?,
        novel_id: row.try_get("novel_id").map_err(map_sql)?,
        chapter_id: row.try_get("chapter_id").map_err(map_sql)?,
        chapter_version: row.try_get("chapter_version").map_err(map_sql)?,
        status: row.try_get("status").map_err(map_sql)?,
        error: row.try_get("error").map_err(map_sql)?,
        created_at: row.try_get("created_at").map_err(map_sql)?,
    })
}

fn candidate_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<MemoryCandidateView, PortError> {
    Ok(MemoryCandidateView {
        id: row.try_get("id").map_err(map_sql)?,
        job_id: row.try_get("job_id").map_err(map_sql)?,
        novel_id: row.try_get("novel_id").map_err(map_sql)?,
        chapter_id: row.try_get("chapter_id").map_err(map_sql)?,
        chapter_version: row.try_get("chapter_version").map_err(map_sql)?,
        source_content_hash: row.try_get("source_content_hash").map_err(map_sql)?,
        fact_type: row.try_get("fact_type").map_err(map_sql)?,
        truthfulness: row.try_get("truthfulness").map_err(map_sql)?,
        entity_name: row.try_get("entity_name").map_err(map_sql)?,
        entity_id: row.try_get("entity_id").map_err(map_sql)?,
        value: row.try_get("value").map_err(map_sql)?,
        evidence_quote: row.try_get("evidence_quote").map_err(map_sql)?,
        evidence_start: row.try_get("evidence_start").map_err(map_sql)?,
        confidence: row.try_get("confidence").map_err(map_sql)?,
        status: row.try_get("status").map_err(map_sql)?,
        ambiguous: row.try_get::<i64, _>("ambiguous").map_err(map_sql)? != 0,
        version: row.try_get("version").map_err(map_sql)?,
        decision_reason: row.try_get("decision_reason").map_err(map_sql)?,
        created_at: row.try_get("created_at").map_err(map_sql)?,
        updated_at: row.try_get("updated_at").map_err(map_sql)?,
    })
}

async fn load_candidate(
    tx: &mut crate::db::Tx<'_>,
    candidate_id: &str,
) -> Result<MemoryCandidateView, PortError> {
    let row = sqlx::query(
        "SELECT id, job_id, novel_id, chapter_id, chapter_version, source_content_hash,
                fact_type, truthfulness, entity_name, entity_id, value, evidence_quote,
                evidence_start, confidence, status, ambiguous, version, decision_reason,
                created_at, updated_at
         FROM memory_extraction_candidate WHERE id = ?",
    )
    .bind(candidate_id)
    .fetch_optional(tx.deref_mut())
    .await
    .map_err(map_sql)?
    .ok_or_else(|| PortError::NotFound("没有找到这条记忆候选。".into()))?;
    candidate_from_row(&row)
}

async fn chapter_order_for(tx: &mut crate::db::Tx<'_>, chapter_id: &str) -> Result<i32, PortError> {
    sqlx::query_scalar("SELECT sort_order FROM chapter WHERE id = ?")
        .bind(chapter_id)
        .fetch_one(tx.deref_mut())
        .await
        .map_err(map_sql)
}

async fn validate_candidate_source(
    tx: &mut crate::db::Tx<'_>,
    candidate: &MemoryCandidateView,
    allow_existing_state: bool,
) -> Result<(), PortError> {
    if candidate.version < 0 || !matches!(candidate.status.as_str(), "proposed" | "deferred") {
        return Err(PortError::Conflict(
            "记忆候选已经处理，不能重复裁决。".into(),
        ));
    }
    if candidate.ambiguous
        || candidate.entity_id.is_none() && candidate.fact_type == "character_state"
    {
        return Err(PortError::Conflict(
            "人物身份存在歧义，需要人工指定后再确认。".into(),
        ));
    }
    let current = sqlx::query(
        "SELECT r.version AS version, r.content AS content FROM chapter c
         JOIN chapter_revision r ON r.id = c.official_revision_id WHERE c.id = ?",
    )
    .bind(&candidate.chapter_id)
    .fetch_optional(tx.deref_mut())
    .await
    .map_err(map_sql)?
    .ok_or_else(|| PortError::Conflict("来源章节没有正式正文。".into()))?;
    let version: i64 = current.try_get("version").map_err(map_sql)?;
    if version != candidate.chapter_version {
        return Err(PortError::Conflict(
            "来源章节已经被修改，候选事实已过期。".into(),
        ));
    }
    let content: String = current.try_get("content").map_err(map_sql)?;
    use sha2::{Digest, Sha256};
    if hex::encode(Sha256::digest(content.as_bytes())) != candidate.source_content_hash {
        return Err(PortError::Conflict(
            "来源正文哈希不一致，候选事实已过期。".into(),
        ));
    }
    if !allow_existing_state
        && candidate.fact_type == "character_state"
        && let Some(entity_id) = candidate.entity_id.as_deref()
    {
        let order = chapter_order_for(tx, &candidate.chapter_id).await?;
        let existing: Option<String> = sqlx::query_scalar(
            "SELECT body FROM character_state
             WHERE character_id = ? AND chapter_order = ? AND status = 'confirmed' AND body != ?
             LIMIT 1",
        )
        .bind(entity_id)
        .bind(order)
        .bind(&candidate.value)
        .fetch_optional(tx.deref_mut())
        .await
        .map_err(map_sql)?;
        if existing.is_some() {
            return Err(PortError::Conflict(
                "同一人物在同一剧情位置存在不兼容状态，需人工裁决后再确认。".into(),
            ));
        }
    }
    Ok(())
}

async fn write_candidate_fact(
    tx: &mut crate::db::Tx<'_>,
    candidate: &MemoryCandidateView,
    chapter_order: i32,
    now_ms: i64,
) -> Result<String, PortError> {
    let fact_status = if matches!(
        candidate.truthfulness.as_str(),
        "narrator_fact" | "author_asserted"
    ) {
        "confirmed"
    } else {
        "disputed"
    };
    let fact_id = format!("memory-{}", candidate.id);
    match candidate.fact_type.as_str() {
        "character_state" => {
            sqlx::query(
                "INSERT INTO character_state
                   (id, character_id, chapter_order, body, created_at, source_revision, status,
                    updated_at, source_chapter_id)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT(id) DO UPDATE SET body = excluded.body, chapter_order = excluded.chapter_order,
                   source_revision = excluded.source_revision, status = excluded.status,
                   updated_at = excluded.updated_at, source_chapter_id = excluded.source_chapter_id",
            ).bind(&fact_id).bind(candidate.entity_id.as_deref().unwrap_or(""))
            .bind(chapter_order).bind(&candidate.value).bind(now_ms)
            .bind(format!("memory:{}", candidate.chapter_version)).bind(fact_status)
            .bind(now_ms).bind(&candidate.chapter_id)
            .execute(tx.deref_mut()).await.map_err(map_sql)?;
        }
        "timeline_event" => {
            sqlx::query(
                "INSERT INTO timeline_event
                   (id, novel_id, chapter_order, body, created_at, source_revision, status,
                    updated_at, source_chapter_id)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT(id) DO UPDATE SET body = excluded.body, chapter_order = excluded.chapter_order,
                   source_revision = excluded.source_revision, status = excluded.status,
                   updated_at = excluded.updated_at, source_chapter_id = excluded.source_chapter_id",
            ).bind(&fact_id).bind(&candidate.novel_id).bind(chapter_order)
            .bind(&candidate.value).bind(now_ms)
            .bind(format!("memory:{}", candidate.chapter_version)).bind(fact_status)
            .bind(now_ms).bind(&candidate.chapter_id)
            .execute(tx.deref_mut()).await.map_err(map_sql)?;
        }
        "plot_thread" => {
            let title = candidate
                .entity_name
                .clone()
                .unwrap_or_else(|| "未命名伏笔".into());
            sqlx::query(
                "INSERT INTO plot_thread
                   (id, novel_id, title, body, opened_chapter_order, resolved_chapter_order,
                    status, source_revision, evidence_json, created_at, updated_at, source_chapter_id)
                 VALUES (?, ?, ?, ?, ?, NULL, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT(id) DO UPDATE SET title = excluded.title, body = excluded.body,
                   opened_chapter_order = excluded.opened_chapter_order, status = excluded.status,
                   source_revision = excluded.source_revision, updated_at = excluded.updated_at,
                   source_chapter_id = excluded.source_chapter_id",
            ).bind(&fact_id).bind(&candidate.novel_id).bind(title).bind(&candidate.value)
            .bind(chapter_order).bind(if fact_status == "confirmed" { "open" } else { "disputed" })
            .bind(format!("memory:{}", candidate.chapter_version)).bind(&candidate.evidence_quote)
            .bind(now_ms).bind(now_ms).bind(&candidate.chapter_id)
            .execute(tx.deref_mut()).await.map_err(map_sql)?;
        }
        other => return Err(PortError::Integrity(format!("未知记忆类型：{other}"))),
    }
    Ok(fact_id)
}

async fn set_candidate_status(
    tx: &mut crate::db::Tx<'_>,
    candidate: &MemoryCandidateView,
    status: &str,
    reason: Option<&str>,
    now_ms: i64,
) -> Result<(), PortError> {
    let result = sqlx::query(
        "UPDATE memory_extraction_candidate SET status = ?, decision_reason = ?,
                version = version + 1, updated_at = ?
         WHERE id = ? AND version = ? AND status IN ('proposed','deferred')",
    )
    .bind(status)
    .bind(reason)
    .bind(now_ms)
    .bind(&candidate.id)
    .bind(candidate.version)
    .execute(tx.deref_mut())
    .await
    .map_err(map_sql)?;
    if result.rows_affected() != 1 {
        return Err(PortError::Conflict(
            "记忆候选已经变化，请刷新后再确认。".into(),
        ));
    }
    Ok(())
}

async fn reject_other_candidates(
    tx: &mut crate::db::Tx<'_>,
    ids: &[String],
    chosen: &str,
    now_ms: i64,
) -> Result<(), PortError> {
    for id in ids.iter().filter(|id| id.as_str() != chosen) {
        sqlx::query(
            "UPDATE memory_extraction_candidate SET status = 'rejected', decision_reason = ?,
                    version = version + 1, updated_at = ?
             WHERE id = ? AND status IN ('proposed','deferred')",
        )
        .bind("冲突裁决：未采用该候选")
        .bind(now_ms)
        .bind(id)
        .execute(tx.deref_mut())
        .await
        .map_err(map_sql)?;
    }
    Ok(())
}

async fn reject_candidates(
    tx: &mut crate::db::Tx<'_>,
    ids: &[String],
    reason: &str,
    now_ms: i64,
) -> Result<(), PortError> {
    for id in ids {
        sqlx::query(
            "UPDATE memory_extraction_candidate SET status = 'rejected', decision_reason = ?,
                    version = version + 1, updated_at = ?
             WHERE id = ? AND status IN ('proposed','deferred')",
        )
        .bind(reason)
        .bind(now_ms)
        .bind(id)
        .execute(tx.deref_mut())
        .await
        .map_err(map_sql)?;
    }
    Ok(())
}

async fn defer_candidates(
    tx: &mut crate::db::Tx<'_>,
    ids: &[String],
    reason: &str,
    now_ms: i64,
) -> Result<(), PortError> {
    for id in ids {
        sqlx::query(
            "UPDATE memory_extraction_candidate SET status = 'deferred', decision_reason = ?,
                    version = version + 1, updated_at = ?
             WHERE id = ? AND status IN ('proposed','deferred')",
        )
        .bind(reason)
        .bind(now_ms)
        .bind(id)
        .execute(tx.deref_mut())
        .await
        .map_err(map_sql)?;
    }
    Ok(())
}

async fn resolve_structured_fact_conflict(
    tx: &mut crate::db::Tx<'_>,
    conflict: &MemoryConflictView,
    decision: &str,
    now_ms: i64,
) -> Result<(), PortError> {
    if conflict.fact_ids.len() < 2 {
        return Err(PortError::Integrity("结构化冲突缺少事实对。".into()));
    }
    let old_id = &conflict.fact_ids[0];
    let new_id = &conflict.fact_ids[1];
    let current: Option<i64> = sqlx::query_scalar(
        "SELECT r.version FROM structured_story_fact f
         JOIN chapter c ON c.id = f.source_chapter_id
         JOIN chapter_revision r ON r.id = c.official_revision_id
         WHERE f.id = ? AND f.novel_id = ? AND r.version = f.source_revision",
    )
    .bind(new_id)
    .bind(&conflict.novel_id)
    .fetch_optional(tx.deref_mut())
    .await
    .map_err(map_sql)?;
    if current.is_none() {
        return Err(PortError::Conflict(
            "新事实的来源章节已经变化，请重新确认。".into(),
        ));
    }
    match decision {
        "confirm_new" | "supersede_with_evidence" => {
            sqlx::query("UPDATE structured_story_fact SET status='superseded', superseded_by=?, updated_at=? WHERE id=? AND novel_id=?")
                .bind(new_id).bind(now_ms).bind(old_id).bind(&conflict.novel_id)
                .execute(tx.deref_mut()).await.map_err(map_sql)?;
            sqlx::query("UPDATE structured_story_fact SET truthfulness='confirmed', status='active', updated_at=? WHERE id=? AND novel_id=?")
                .bind(now_ms).bind(new_id).bind(&conflict.novel_id)
                .execute(tx.deref_mut()).await.map_err(map_sql)?;
        }
        "keep_existing" | "reject_extraction" => {
            sqlx::query("UPDATE structured_story_fact SET status='rejected', updated_at=? WHERE id=? AND novel_id=?")
                .bind(now_ms).bind(new_id).bind(&conflict.novel_id)
                .execute(tx.deref_mut()).await.map_err(map_sql)?;
        }
        "mark_disputed" => {
            sqlx::query("UPDATE structured_story_fact SET status='disputed', updated_at=? WHERE id=? AND novel_id=?")
                .bind(now_ms).bind(new_id).bind(&conflict.novel_id)
                .execute(tx.deref_mut()).await.map_err(map_sql)?;
        }
        _ => return Err(PortError::Integrity("未知结构化冲突裁决。".into())),
    }
    Ok(())
}

async fn supersede_old_facts(
    tx: &mut crate::db::Tx<'_>,
    novel_id: &str,
    fact_ids: &[String],
    now_ms: i64,
) -> Result<(), PortError> {
    for id in fact_ids {
        sqlx::query(
            "UPDATE character_state SET status = 'superseded', updated_at = ?
             WHERE id = ? AND character_id IN (SELECT id FROM character WHERE novel_id = ?)",
        )
        .bind(now_ms)
        .bind(id)
        .bind(novel_id)
        .execute(tx.deref_mut())
        .await
        .map_err(map_sql)?;
        sqlx::query(
            "UPDATE timeline_event SET status = 'superseded', updated_at = ?
             WHERE id = ? AND novel_id = ?",
        )
        .bind(now_ms)
        .bind(id)
        .bind(novel_id)
        .execute(tx.deref_mut())
        .await
        .map_err(map_sql)?;
        sqlx::query(
            "UPDATE plot_thread SET status = 'superseded', updated_at = ?
             WHERE id = ? AND novel_id = ?",
        )
        .bind(now_ms)
        .bind(id)
        .bind(novel_id)
        .execute(tx.deref_mut())
        .await
        .map_err(map_sql)?;
    }
    Ok(())
}

fn conflict_id(
    kind: &str,
    novel: &str,
    chapter: &str,
    fact_type: &str,
    entity: &str,
    ids: &str,
) -> String {
    use sha2::{Digest, Sha256};
    let mut sorted = ids.split(',').collect::<Vec<_>>();
    sorted.sort_unstable();
    let digest = Sha256::digest(
        format!(
            "{kind}\u{1f}{novel}\u{1f}{chapter}\u{1f}{fact_type}\u{1f}{entity}\u{1f}{}",
            sorted.join(",")
        )
        .as_bytes(),
    );
    format!("conflict-{}", &hex::encode(digest)[..32])
}

fn conflict_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<MemoryConflictView, PortError> {
    let candidate_ids: String = row.try_get("candidate_ids_json").map_err(map_sql)?;
    let fact_ids: String = row.try_get("fact_ids_json").map_err(map_sql)?;
    Ok(MemoryConflictView {
        id: row.try_get("id").map_err(map_sql)?,
        novel_id: row.try_get("novel_id").map_err(map_sql)?,
        conflict_type: row.try_get("conflict_type").map_err(map_sql)?,
        entity_key: row.try_get("entity_key").map_err(map_sql)?,
        chapter_id: row.try_get("chapter_id").map_err(map_sql)?,
        chapter_order: row.try_get("chapter_order").map_err(map_sql)?,
        candidate_ids: serde_json::from_str(&candidate_ids).unwrap_or_default(),
        fact_ids: serde_json::from_str(&fact_ids).unwrap_or_default(),
        status: row.try_get("status").map_err(map_sql)?,
        version: row.try_get("version").map_err(map_sql)?,
        description: row.try_get("description").map_err(map_sql)?,
        created_at: row.try_get("created_at").map_err(map_sql)?,
        updated_at: row.try_get("updated_at").map_err(map_sql)?,
    })
}
