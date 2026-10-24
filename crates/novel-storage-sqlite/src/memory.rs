use std::ops::DerefMut;

use async_trait::async_trait;
use novel_application::{
    MemoryCandidateView, MemoryExtractionJobView, MemoryRepository, NewMemoryCandidate,
    NewMemoryJob, PortError,
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
