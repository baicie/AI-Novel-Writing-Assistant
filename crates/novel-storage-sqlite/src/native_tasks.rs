use async_trait::async_trait;
use novel_application::{NativeTaskQuery, NativeTaskRepository, NativeTaskSummary, PortError};
use sqlx::Row;

use crate::db::{SqliteKernel, map_sql};

fn actions(task_type: &str, status: &str) -> (Vec<String>, bool) {
    match task_type {
        "director_run" => {
            let actions = match status {
                "waiting_approval" => vec!["open_approval", "open_detail"],
                "paused" => vec!["open_detail", "manual_resolution"],
                "failed" => vec!["open_detail"],
                _ => vec!["open_detail"],
            };
            (
                actions.into_iter().map(str::to_string).collect(),
                status == "waiting_approval" || status == "paused",
            )
        }
        "batch" => (
            vec!["open_batch".to_string(), "open_detail".to_string()],
            matches!(status, "waiting_approval" | "paused" | "failed"),
        ),
        "index_job" => (
            vec!["open_detail".to_string()],
            matches!(status, "failed" | "retry_wait"),
        ),
        "memory_extraction" => (
            vec!["open_detail".to_string()],
            status == "failed" || status == "succeeded",
        ),
        "quality_debt" => (vec!["open_detail".to_string()], true),
        _ => (vec!["open_detail".to_string()], false),
    }
}

async fn runs(
    kernel: &SqliteKernel,
    novel_id: Option<&str>,
) -> Result<Vec<NativeTaskSummary>, PortError> {
    let rows = sqlx::query(
        "SELECT r.id, r.novel_id,
                json_extract(r.contract_json, '$.targetChapterId') AS chapter_id,
                c.status, c.cursor_step_id, c.failure_reason,
                n.title AS novel_title, ch.title AS chapter_title, r.created_at
         FROM workflow_run r
         JOIN workflow_control c ON c.run_id = r.id
         JOIN novel n ON n.id = r.novel_id
         LEFT JOIN chapter ch ON ch.id = json_extract(r.contract_json, '$.targetChapterId')
         WHERE (? IS NULL OR r.novel_id = ?)
         ORDER BY r.created_at DESC LIMIT 200",
    )
    .bind(novel_id)
    .bind(novel_id)
    .fetch_all(&kernel.pool)
    .await
    .map_err(map_sql)?;
    rows.into_iter()
        .map(|row| {
            let id: String = row.try_get("id").map_err(map_sql)?;
            let novel_id: String = row.try_get("novel_id").map_err(map_sql)?;
            let status: String = row.try_get("status").map_err(map_sql)?;
            let (available, requires) = actions("director_run", &status);
            let chapter_title: Option<String> = row.try_get("chapter_title").map_err(map_sql)?;
            Ok(NativeTaskSummary {
                task_id: id.clone(),
                task_type: "director_run".into(),
                novel_id: novel_id.clone(),
                chapter_id: row.try_get("chapter_id").map_err(map_sql)?,
                batch_id: None,
                title: format!(
                    "单章创作 · {} · {}",
                    row.try_get::<String, _>("novel_title").map_err(map_sql)?,
                    chapter_title.unwrap_or_else(|| "未命名章节".into())
                ),
                status: status.clone(),
                current_stage: row.try_get("cursor_step_id").map_err(map_sql)?,
                progress: None,
                created_at: row.try_get("created_at").map_err(map_sql)?,
                updated_at: row.try_get("created_at").map_err(map_sql)?,
                last_error: row.try_get("failure_reason").map_err(map_sql)?,
                requires_user_action: requires,
                available_actions: available,
                detail_route: format!("/lab/director/{novel_id}?runId={id}"),
            })
        })
        .collect()
}

async fn batches(
    kernel: &SqliteKernel,
    novel_id: Option<&str>,
) -> Result<Vec<NativeTaskSummary>, PortError> {
    let rows = sqlx::query(
        "SELECT b.id, b.novel_id, b.status, b.cursor, b.failure_reason, b.created_at, b.updated_at,
                n.title AS novel_title, (SELECT COUNT(*) FROM batch_run_item i WHERE i.batch_id = b.id) AS total,
                (SELECT COUNT(*) FROM batch_run_item i WHERE i.batch_id = b.id AND i.status = 'succeeded') AS done
         FROM batch_run b JOIN novel n ON n.id = b.novel_id
         WHERE (? IS NULL OR b.novel_id = ?)
         ORDER BY b.updated_at DESC LIMIT 200",
    ).bind(novel_id).bind(novel_id).fetch_all(&kernel.pool).await.map_err(map_sql)?;
    rows.into_iter()
        .map(|row| {
            let id: String = row.try_get("id").map_err(map_sql)?;
            let novel_id: String = row.try_get("novel_id").map_err(map_sql)?;
            let status: String = row.try_get("status").map_err(map_sql)?;
            let total: i64 = row.try_get("total").map_err(map_sql)?;
            let done: i64 = row.try_get("done").map_err(map_sql)?;
            let (available, requires) = actions("batch", &status);
            Ok(NativeTaskSummary {
                task_id: id.clone(),
                task_type: "batch".into(),
                novel_id: novel_id.clone(),
                chapter_id: None,
                batch_id: Some(id.clone()),
                title: format!(
                    "连续创作 · {}",
                    row.try_get::<String, _>("novel_title").map_err(map_sql)?
                ),
                status,
                current_stage: Some(format!("{done}/{total}")),
                progress: Some(if total > 0 { done * 100 / total } else { 0 }),
                created_at: row.try_get("created_at").map_err(map_sql)?,
                updated_at: row.try_get("updated_at").map_err(map_sql)?,
                last_error: row.try_get("failure_reason").map_err(map_sql)?,
                requires_user_action: requires,
                available_actions: available,
                detail_route: format!("/lab/director/{novel_id}"),
            })
        })
        .collect()
}

async fn index_jobs(
    kernel: &SqliteKernel,
    novel_id: Option<&str>,
) -> Result<Vec<NativeTaskSummary>, PortError> {
    let rows = sqlx::query(
        "SELECT j.id, j.novel_id, j.status, j.last_error, j.created_at, j.updated_at, n.title AS novel_title
         FROM knowledge_index_job j JOIN novel n ON n.id = j.novel_id
         WHERE (? IS NULL OR j.novel_id = ?)
         ORDER BY j.updated_at DESC LIMIT 200",
    ).bind(novel_id).bind(novel_id).fetch_all(&kernel.pool).await.map_err(map_sql)?;
    rows.into_iter()
        .map(|row| {
            let id: String = row.try_get("id").map_err(map_sql)?;
            let novel_id: String = row.try_get("novel_id").map_err(map_sql)?;
            let status: String = row.try_get("status").map_err(map_sql)?;
            let (available, requires) = actions("index_job", &status);
            Ok(NativeTaskSummary {
                task_id: id,
                task_type: "index_job".into(),
                novel_id: novel_id.clone(),
                chapter_id: None,
                batch_id: None,
                title: format!(
                    "知识索引 · {}",
                    row.try_get::<String, _>("novel_title").map_err(map_sql)?
                ),
                status,
                current_stage: None,
                progress: None,
                created_at: row.try_get("created_at").map_err(map_sql)?,
                updated_at: row.try_get("updated_at").map_err(map_sql)?,
                last_error: row.try_get("last_error").map_err(map_sql)?,
                requires_user_action: requires,
                available_actions: available,
                detail_route: format!("/lab/director/{novel_id}"),
            })
        })
        .collect()
}

async fn memory_jobs(
    kernel: &SqliteKernel,
    novel_id: Option<&str>,
) -> Result<Vec<NativeTaskSummary>, PortError> {
    let rows = sqlx::query(
        "SELECT j.id, j.novel_id, j.chapter_id, j.status, j.error, j.created_at, j.updated_at,
                n.title AS novel_title, ch.title AS chapter_title
         FROM memory_extraction_job j JOIN novel n ON n.id = j.novel_id
         LEFT JOIN chapter ch ON ch.id = j.chapter_id
         WHERE (? IS NULL OR j.novel_id = ?)
         ORDER BY j.updated_at DESC LIMIT 200",
    )
    .bind(novel_id)
    .bind(novel_id)
    .fetch_all(&kernel.pool)
    .await
    .map_err(map_sql)?;
    rows.into_iter()
        .map(|row| {
            let id: String = row.try_get("id").map_err(map_sql)?;
            let novel_id: String = row.try_get("novel_id").map_err(map_sql)?;
            let status: String = row.try_get("status").map_err(map_sql)?;
            let (available, requires) = actions("memory_extraction", &status);
            Ok(NativeTaskSummary {
                task_id: id,
                task_type: "memory_extraction".into(),
                novel_id: novel_id.clone(),
                chapter_id: row.try_get("chapter_id").map_err(map_sql)?,
                batch_id: None,
                title: format!(
                    "记忆抽取 · {} · {}",
                    row.try_get::<String, _>("novel_title").map_err(map_sql)?,
                    row.try_get::<Option<String>, _>("chapter_title")
                        .map_err(map_sql)?
                        .unwrap_or_else(|| "章节".into())
                ),
                status,
                current_stage: None,
                progress: None,
                created_at: row.try_get("created_at").map_err(map_sql)?,
                updated_at: row.try_get("updated_at").map_err(map_sql)?,
                last_error: row.try_get("error").map_err(map_sql)?,
                requires_user_action: requires,
                available_actions: available,
                detail_route: format!("/lab/director/{novel_id}"),
            })
        })
        .collect()
}

async fn quality_debt(
    kernel: &SqliteKernel,
    novel_id: Option<&str>,
) -> Result<Vec<NativeTaskSummary>, PortError> {
    let rows = sqlx::query(
        "SELECT q.id, q.novel_id, q.run_id, q.chapter_order, q.detail, q.created_at, n.title AS novel_title
         FROM quality_debt q JOIN novel n ON n.id = q.novel_id
         WHERE (? IS NULL OR q.novel_id = ?)
         ORDER BY q.created_at DESC LIMIT 200",
    ).bind(novel_id).bind(novel_id).fetch_all(&kernel.pool).await.map_err(map_sql)?;
    rows.into_iter()
        .map(|row| {
            let id: String = row.try_get("id").map_err(map_sql)?;
            let novel_id: String = row.try_get("novel_id").map_err(map_sql)?;
            Ok(NativeTaskSummary {
                task_id: id,
                task_type: "quality_debt".into(),
                novel_id: novel_id.clone(),
                chapter_id: None,
                batch_id: None,
                title: format!(
                    "质量问题 · {} · 第{}章",
                    row.try_get::<String, _>("novel_title").map_err(map_sql)?,
                    row.try_get::<i64, _>("chapter_order").map_err(map_sql)?
                ),
                status: "open".into(),
                current_stage: None,
                progress: None,
                created_at: row.try_get("created_at").map_err(map_sql)?,
                updated_at: row.try_get("created_at").map_err(map_sql)?,
                last_error: Some(row.try_get::<String, _>("detail").map_err(map_sql)?),
                requires_user_action: true,
                available_actions: vec!["open_detail".into()],
                detail_route: format!("/lab/director/{novel_id}"),
            })
        })
        .collect()
}

#[async_trait]
impl NativeTaskRepository for SqliteKernel {
    async fn list_native_tasks(
        &self,
        query: &NativeTaskQuery,
    ) -> Result<Vec<NativeTaskSummary>, PortError> {
        let novel = query.novel_id.as_deref();
        let mut items = Vec::new();
        items.extend(runs(self, novel).await?);
        items.extend(batches(self, novel).await?);
        items.extend(index_jobs(self, novel).await?);
        items.extend(memory_jobs(self, novel).await?);
        items.extend(quality_debt(self, novel).await?);
        if let Some(kind) = &query.task_type {
            items.retain(|item| &item.task_type == kind);
        }
        if let Some(status) = &query.status {
            items.retain(|item| &item.status == status);
        }
        items.sort_by(|left, right| {
            right
                .updated_at
                .cmp(&left.updated_at)
                .then_with(|| left.task_id.cmp(&right.task_id))
        });
        let offset = query.offset.max(0) as usize;
        let limit = if query.limit <= 0 {
            100
        } else {
            query.limit.min(200)
        } as usize;
        Ok(items.into_iter().skip(offset).take(limit).collect())
    }
}
