use std::ops::DerefMut;

use async_trait::async_trait;
use novel_application::{
    BatchFence, BatchItemView, BatchRepository, BatchRunView, NewBatchRun, PortError,
};
use sqlx::Row;

use crate::db::{SqliteKernel, map_sql};

fn batch_from_rows(
    row: &sqlx::sqlite::SqliteRow,
    items: Vec<BatchItemView>,
) -> Result<BatchRunView, PortError> {
    Ok(BatchRunView {
        id: row.try_get("id").map_err(map_sql)?,
        novel_id: row.try_get("novel_id").map_err(map_sql)?,
        command_id: row.try_get("command_id").map_err(map_sql)?,
        request_hash: row.try_get("request_hash").map_err(map_sql)?,
        status: row.try_get("status").map_err(map_sql)?,
        cursor: row.try_get("cursor").map_err(map_sql)?,
        version: row.try_get("version").map_err(map_sql)?,
        claim_epoch: row.try_get("claim_epoch").map_err(map_sql)?,
        failure_reason: row.try_get("failure_reason").map_err(map_sql)?,
        created_at: row.try_get("created_at").map_err(map_sql)?,
        updated_at: row.try_get("updated_at").map_err(map_sql)?,
        items,
    })
}

async fn load_batch(kernel: &SqliteKernel, id: &str) -> Result<BatchRunView, PortError> {
    let row = sqlx::query("SELECT * FROM batch_run WHERE id = ?")
        .bind(id)
        .fetch_optional(&kernel.pool)
        .await
        .map_err(map_sql)?
        .ok_or_else(|| PortError::NotFound("没有找到批量创作任务。".into()))?;
    let item_rows =
        sqlx::query("SELECT * FROM batch_run_item WHERE batch_id = ? ORDER BY ordinal ASC")
            .bind(id)
            .fetch_all(&kernel.pool)
            .await
            .map_err(map_sql)?;
    let items = item_rows
        .iter()
        .map(|item| {
            Ok(BatchItemView {
                id: item.try_get("id").map_err(map_sql)?,
                chapter_id: item.try_get("chapter_id").map_err(map_sql)?,
                ordinal: item.try_get("ordinal").map_err(map_sql)?,
                run_id: item.try_get("run_id").map_err(map_sql)?,
                status: item.try_get("status").map_err(map_sql)?,
                version: item.try_get("version").map_err(map_sql)?,
                last_error: item.try_get("last_error").map_err(map_sql)?,
                started_at: item.try_get("started_at").map_err(map_sql)?,
                completed_at: item.try_get("completed_at").map_err(map_sql)?,
            })
        })
        .collect::<Result<Vec<_>, PortError>>()?;
    batch_from_rows(&row, items)
}

async fn insert_event(
    tx: &mut crate::db::Tx<'_>,
    batch_id: &str,
    event_type: &str,
    payload: &str,
    now_ms: i64,
) -> Result<(), PortError> {
    let sequence: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(sequence), 0) + 1 FROM batch_run_event WHERE batch_id = ?",
    )
    .bind(batch_id)
    .fetch_one(tx.deref_mut())
    .await
    .map_err(map_sql)?;
    sqlx::query("INSERT INTO batch_run_event(id, batch_id, sequence, event_type, payload_json, created_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(format!("{batch_id}:event:{sequence}"))
        .bind(batch_id)
        .bind(sequence)
        .bind(event_type)
        .bind(payload)
        .bind(now_ms)
        .execute(tx.deref_mut())
        .await
        .map_err(map_sql)?;
    Ok(())
}

async fn touch_batch(
    tx: &mut crate::db::Tx<'_>,
    id: &str,
    fence: &BatchFence,
    status: &str,
    cursor: i32,
    failure: Option<&str>,
    now_ms: i64,
) -> Result<(), PortError> {
    let result = sqlx::query(
        "UPDATE batch_run SET status = ?, cursor = ?, failure_reason = ?, version = version + 1,
                lease_owner = NULL, lease_until = NULL, updated_at = ?
         WHERE id = ? AND version = ? AND lease_owner = ? AND claim_epoch = ? AND lease_until >= ?",
    )
    .bind(status)
    .bind(cursor)
    .bind(failure)
    .bind(now_ms)
    .bind(id)
    .bind(fence.version)
    .bind(&fence.owner)
    .bind(fence.claim_epoch)
    .bind(now_ms)
    .execute(tx.deref_mut())
    .await
    .map_err(map_sql)?;
    if result.rows_affected() != 1 {
        return Err(PortError::Conflict("批量任务的执行权已经变化。".into()));
    }
    Ok(())
}

#[async_trait]
impl BatchRepository for SqliteKernel {
    async fn create_batch(&self, record: NewBatchRun) -> Result<BatchRunView, PortError> {
        let mut tx = self.pool.begin().await.map_err(map_sql)?;
        let existing: Option<(String, String)> =
            sqlx::query_as("SELECT id, request_hash FROM batch_run WHERE command_id = ?")
                .bind(&record.command_id)
                .fetch_optional(tx.deref_mut())
                .await
                .map_err(map_sql)?;
        if let Some((existing_id, existing_hash)) = existing {
            if existing_hash != record.request_hash {
                return Err(PortError::Conflict(
                    "同一个操作编号不能创建不同的批量任务。".into(),
                ));
            }
            tx.commit().await.map_err(map_sql)?;
            return load_batch(self, &existing_id).await;
        }
        sqlx::query(
            "INSERT INTO batch_run(id, novel_id, command_id, request_hash, request_json, status, cursor, version, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, 'created', 0, 0, ?, ?)",
        )
        .bind(&record.id).bind(&record.novel_id).bind(&record.command_id)
        .bind(&record.request_hash).bind(&record.request_json)
        .bind(record.now_ms).bind(record.now_ms)
        .execute(tx.deref_mut()).await.map_err(map_sql)?;
        for (id, chapter_id, ordinal) in &record.items {
            sqlx::query(
                "INSERT INTO batch_run_item(id, batch_id, chapter_id, ordinal, status, version)
                 VALUES (?, ?, ?, ?, 'pending', 0)",
            )
            .bind(id)
            .bind(&record.id)
            .bind(chapter_id)
            .bind(ordinal)
            .execute(tx.deref_mut())
            .await
            .map_err(map_sql)?;
        }
        insert_event(&mut tx, &record.id, "batch_created", "{}", record.now_ms).await?;
        tx.commit().await.map_err(map_sql)?;
        load_batch(self, &record.id).await
    }

    async fn get_batch(&self, id: &str) -> Result<BatchRunView, PortError> {
        load_batch(self, id).await
    }

    async fn get_batch_request_json(&self, id: &str) -> Result<String, PortError> {
        sqlx::query_scalar("SELECT request_json FROM batch_run WHERE id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map_err(map_sql)
    }

    async fn list_batches(&self, novel_id: &str) -> Result<Vec<BatchRunView>, PortError> {
        let ids: Vec<String> = sqlx::query_scalar(
            "SELECT id FROM batch_run WHERE novel_id = ? ORDER BY created_at DESC",
        )
        .bind(novel_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sql)?;
        let mut batches = Vec::with_capacity(ids.len());
        for id in ids {
            batches.push(load_batch(self, &id).await?);
        }
        Ok(batches)
    }

    async fn list_runnable_batch_ids(&self) -> Result<Vec<String>, PortError> {
        sqlx::query_scalar(
            "SELECT id FROM batch_run
             WHERE status IN ('created','running','waiting_approval')
               AND (lease_until IS NULL OR lease_until < CAST(strftime('%s','now') AS INTEGER) * 1000)
             ORDER BY created_at ASC LIMIT 20",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sql)
    }

    async fn claim_batch(
        &self,
        id: &str,
        owner: &str,
        lease_until: i64,
        now_ms: i64,
    ) -> Result<Option<BatchRunView>, PortError> {
        let updated = sqlx::query(
            "UPDATE batch_run SET lease_owner = ?, lease_until = ?, claim_epoch = claim_epoch + 1, updated_at = ?
             WHERE id = ? AND status IN ('created','running','waiting_approval')
               AND (lease_until IS NULL OR lease_until < ?)",
        )
        .bind(owner).bind(lease_until).bind(now_ms).bind(id).bind(now_ms)
        .execute(&self.pool).await.map_err(map_sql)?;
        if updated.rows_affected() == 0 {
            return Ok(None);
        }
        Ok(Some(load_batch(self, id).await?))
    }

    async fn update_batch(
        &self,
        fence: &BatchFence,
        status: &str,
        cursor: i32,
        failure: Option<&str>,
        now_ms: i64,
    ) -> Result<BatchRunView, PortError> {
        let id: String = sqlx::query_scalar(
            "SELECT id FROM batch_run WHERE lease_owner = ? AND claim_epoch = ? AND version = ?",
        )
        .bind(&fence.owner)
        .bind(fence.claim_epoch)
        .bind(fence.version)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sql)?;
        let mut tx = self.pool.begin().await.map_err(map_sql)?;
        touch_batch(&mut tx, &id, fence, status, cursor, failure, now_ms).await?;
        insert_event(
            &mut tx,
            &id,
            "batch_state",
            &serde_json::json!({"status": status, "cursor": cursor}).to_string(),
            now_ms,
        )
        .await?;
        tx.commit().await.map_err(map_sql)?;
        load_batch(self, &id).await
    }

    async fn bind_item_run(
        &self,
        fence: &BatchFence,
        item_id: &str,
        run_id: &str,
        now_ms: i64,
    ) -> Result<BatchRunView, PortError> {
        let id: String = sqlx::query_scalar(
            "SELECT id FROM batch_run WHERE lease_owner = ? AND claim_epoch = ? AND version = ?",
        )
        .bind(&fence.owner)
        .bind(fence.claim_epoch)
        .bind(fence.version)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sql)?;
        let mut tx = self.pool.begin().await.map_err(map_sql)?;
        let updated = sqlx::query(
            "UPDATE batch_run_item SET run_id = ?, status = 'running', started_at = COALESCE(started_at, ?), version = version + 1
             WHERE id = ? AND batch_id = ? AND run_id IS NULL",
        ).bind(run_id).bind(now_ms).bind(item_id).bind(&id)
        .execute(tx.deref_mut()).await.map_err(map_sql)?;
        if updated.rows_affected() != 1 {
            return Err(PortError::Conflict("批量章节已经绑定过创作任务。".into()));
        }
        touch_batch(&mut tx, &id, fence, "running", 0, None, now_ms).await?;
        insert_event(
            &mut tx,
            &id,
            "item_run_bound",
            &serde_json::json!({"itemId": item_id, "runId": run_id}).to_string(),
            now_ms,
        )
        .await?;
        tx.commit().await.map_err(map_sql)?;
        load_batch(self, &id).await
    }

    async fn update_item(
        &self,
        fence: &BatchFence,
        item_id: &str,
        status: &str,
        error: Option<&str>,
        now_ms: i64,
    ) -> Result<BatchRunView, PortError> {
        let id: String = sqlx::query_scalar(
            "SELECT id FROM batch_run WHERE lease_owner = ? AND claim_epoch = ? AND version = ?",
        )
        .bind(&fence.owner)
        .bind(fence.claim_epoch)
        .bind(fence.version)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sql)?;
        let mut tx = self.pool.begin().await.map_err(map_sql)?;
        sqlx::query(
            "UPDATE batch_run_item SET status = ?, last_error = ?, completed_at = CASE WHEN ? IN ('succeeded','failed','cancelled') THEN ? ELSE completed_at END, version = version + 1
             WHERE id = ? AND batch_id = ?",
        ).bind(status).bind(error).bind(status).bind(now_ms).bind(item_id).bind(&id)
        .execute(tx.deref_mut()).await.map_err(map_sql)?;
        insert_event(
            &mut tx,
            &id,
            "item_state",
            &serde_json::json!({"itemId": item_id, "status": status}).to_string(),
            now_ms,
        )
        .await?;
        tx.commit().await.map_err(map_sql)?;
        load_batch(self, &id).await
    }

    async fn set_batch_status(
        &self,
        id: &str,
        expected_version: i64,
        status: &str,
        now_ms: i64,
    ) -> Result<BatchRunView, PortError> {
        let mut tx = self.pool.begin().await.map_err(map_sql)?;
        let result = sqlx::query(
            "UPDATE batch_run SET status = ?, version = version + 1, lease_owner = NULL, lease_until = NULL, updated_at = ?
             WHERE id = ? AND version = ?",
        ).bind(status).bind(now_ms).bind(id).bind(expected_version)
        .execute(tx.deref_mut()).await.map_err(map_sql)?;
        if result.rows_affected() != 1 {
            return Err(PortError::Conflict(
                "批量任务已经变化，请刷新后重试。".into(),
            ));
        }
        insert_event(
            &mut tx,
            id,
            "batch_command",
            &serde_json::json!({"status": status}).to_string(),
            now_ms,
        )
        .await?;
        tx.commit().await.map_err(map_sql)?;
        load_batch(self, id).await
    }
}
