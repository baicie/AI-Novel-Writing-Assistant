use std::ops::DerefMut;

use async_trait::async_trait;
use novel_application::{IndexJobRepository, IndexJobView, NewIndexJob, PortError};
use sqlx::Row;

use crate::db::{SqliteKernel, map_sql};

#[async_trait]
impl IndexJobRepository for SqliteKernel {
    async fn enqueue_index_job(&self, job: NewIndexJob) -> Result<IndexJobView, PortError> {
        sqlx::query(
            "INSERT INTO knowledge_index_job
               (id, command_id, novel_id, source_type, source_id, source_revision, index_version,
                status, attempt_count, max_attempts, next_run_at, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, 'pending', 0, ?, ?, ?, ?)
             ON CONFLICT(command_id) DO NOTHING",
        )
        .bind(&job.id)
        .bind(&job.command_id)
        .bind(&job.novel_id)
        .bind(&job.source_type)
        .bind(&job.source_id)
        .bind(&job.source_revision)
        .bind(job.index_version)
        .bind(job.max_attempts)
        .bind(job.now_ms)
        .bind(job.now_ms)
        .bind(job.now_ms)
        .execute(&self.pool)
        .await
        .map_err(map_sql)?;
        let row = sqlx::query(
            "SELECT id, novel_id, source_type, source_id, source_revision, index_version, status,
                    attempt_count, max_attempts, next_run_at, last_error_category, last_error,
                    created_at, updated_at
             FROM knowledge_index_job WHERE command_id = ?",
        )
        .bind(&job.command_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sql)?;
        job_from_row(&row)
    }

    async fn list_index_jobs(&self, novel_id: &str) -> Result<Vec<IndexJobView>, PortError> {
        let rows = sqlx::query(
            "SELECT id, novel_id, source_type, source_id, source_revision, index_version, status,
                    attempt_count, max_attempts, next_run_at, last_error_category, last_error,
                    created_at, updated_at
             FROM knowledge_index_job WHERE novel_id = ?
             ORDER BY CASE status WHEN 'running' THEN 0 WHEN 'retry_wait' THEN 1 WHEN 'pending' THEN 2 ELSE 3 END,
                      created_at DESC, id",
        ).bind(novel_id).fetch_all(&self.pool).await.map_err(map_sql)?;
        rows.iter().map(job_from_row).collect()
    }

    async fn claim_index_job(
        &self,
        job_id: &str,
        owner: &str,
        lease_until: i64,
        now_ms: i64,
    ) -> Result<Option<IndexJobView>, PortError> {
        let result = sqlx::query(
            "UPDATE knowledge_index_job
             SET status = 'running', lease_owner = ?, lease_until = ?, updated_at = ?
             WHERE id = ? AND status IN ('pending','retry_wait') AND next_run_at <= ?",
        )
        .bind(owner)
        .bind(lease_until)
        .bind(now_ms)
        .bind(job_id)
        .bind(now_ms)
        .execute(&self.pool)
        .await
        .map_err(map_sql)?;
        if result.rows_affected() == 0 {
            return Ok(None);
        }
        let row = sqlx::query(
            "SELECT id, novel_id, source_type, source_id, source_revision, index_version, status,
                    attempt_count, max_attempts, next_run_at, last_error_category, last_error,
                    created_at, updated_at
             FROM knowledge_index_job WHERE id = ?",
        )
        .bind(job_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sql)?;
        Ok(Some(job_from_row(&row)?))
    }

    async fn complete_index_job(
        &self,
        job_id: &str,
        owner: &str,
        now_ms: i64,
    ) -> Result<(), PortError> {
        let result = sqlx::query(
            "UPDATE knowledge_index_job SET status = 'succeeded', lease_owner = NULL,
                    lease_until = NULL, last_error = NULL, last_error_category = NULL, updated_at = ?
             WHERE id = ? AND status = 'running' AND lease_owner = ?",
        ).bind(now_ms).bind(job_id).bind(owner).execute(&self.pool).await.map_err(map_sql)?;
        if result.rows_affected() == 0 {
            return Err(PortError::Conflict("索引任务执行权已失效。".into()));
        }
        Ok(())
    }

    async fn fail_index_job(
        &self,
        job_id: &str,
        owner: &str,
        category: &str,
        error: &str,
        next_run_at: i64,
        now_ms: i64,
    ) -> Result<IndexJobView, PortError> {
        let mut tx = self.pool.begin().await.map_err(map_sql)?;
        let row = sqlx::query(
            "SELECT attempt_count, max_attempts FROM knowledge_index_job
             WHERE id = ? AND status = 'running' AND lease_owner = ?",
        )
        .bind(job_id)
        .bind(owner)
        .fetch_optional(tx.deref_mut())
        .await
        .map_err(map_sql)?
        .ok_or_else(|| PortError::Conflict("索引任务执行权已失效。".into()))?;
        let attempts: i64 = row.try_get::<i64, _>("attempt_count").map_err(map_sql)? + 1;
        let max_attempts: i64 = row.try_get("max_attempts").map_err(map_sql)?;
        let status = if attempts >= max_attempts {
            "failed"
        } else {
            "retry_wait"
        };
        sqlx::query(
            "UPDATE knowledge_index_job SET status = ?, attempt_count = ?, next_run_at = ?,
                    last_error_category = ?, last_error = ?, lease_owner = NULL, lease_until = NULL,
                    updated_at = ? WHERE id = ?",
        )
        .bind(status)
        .bind(attempts)
        .bind(next_run_at)
        .bind(category)
        .bind(error)
        .bind(now_ms)
        .bind(job_id)
        .execute(tx.deref_mut())
        .await
        .map_err(map_sql)?;
        tx.commit().await.map_err(map_sql)?;
        let row = sqlx::query(
            "SELECT id, novel_id, source_type, source_id, source_revision, index_version, status,
                    attempt_count, max_attempts, next_run_at, last_error_category, last_error,
                    created_at, updated_at
             FROM knowledge_index_job WHERE id = ?",
        )
        .bind(job_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sql)?;
        job_from_row(&row)
    }

    async fn recover_expired_index_jobs(
        &self,
        now_ms: i64,
    ) -> Result<Vec<IndexJobView>, PortError> {
        let expired: Vec<String> = sqlx::query_scalar(
            "SELECT id FROM knowledge_index_job WHERE status = 'running' AND lease_until < ?",
        )
        .bind(now_ms)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sql)?;
        if expired.is_empty() {
            return Ok(Vec::new());
        }
        sqlx::query(
            "UPDATE knowledge_index_job SET status = 'retry_wait', lease_owner = NULL,
                    lease_until = NULL, next_run_at = ?, updated_at = ?
             WHERE status = 'running' AND lease_until < ?",
        )
        .bind(now_ms)
        .bind(now_ms)
        .bind(now_ms)
        .execute(&self.pool)
        .await
        .map_err(map_sql)?;
        let mut out = Vec::new();
        for id in expired {
            let row = sqlx::query(
                "SELECT id, novel_id, source_type, source_id, source_revision, index_version, status,
                        attempt_count, max_attempts, next_run_at, last_error_category, last_error,
                        created_at, updated_at
                 FROM knowledge_index_job WHERE id = ?",
            ).bind(id).fetch_one(&self.pool).await.map_err(map_sql)?;
            out.push(job_from_row(&row)?);
        }
        Ok(out)
    }
}

fn job_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<IndexJobView, PortError> {
    Ok(IndexJobView {
        id: row.try_get("id").map_err(map_sql)?,
        novel_id: row.try_get("novel_id").map_err(map_sql)?,
        source_type: row.try_get("source_type").map_err(map_sql)?,
        source_id: row.try_get("source_id").map_err(map_sql)?,
        source_revision: row.try_get("source_revision").map_err(map_sql)?,
        index_version: row.try_get("index_version").map_err(map_sql)?,
        status: row.try_get("status").map_err(map_sql)?,
        attempt_count: row.try_get("attempt_count").map_err(map_sql)?,
        max_attempts: row.try_get("max_attempts").map_err(map_sql)?,
        next_run_at: row.try_get("next_run_at").map_err(map_sql)?,
        last_error_category: row.try_get("last_error_category").map_err(map_sql)?,
        last_error: row.try_get("last_error").map_err(map_sql)?,
        created_at: row.try_get("created_at").map_err(map_sql)?,
        updated_at: row.try_get("updated_at").map_err(map_sql)?,
    })
}
