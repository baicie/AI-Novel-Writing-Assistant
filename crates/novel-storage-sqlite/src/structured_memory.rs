use async_trait::async_trait;
use novel_application::{
    NewStructuredFact, PortError, StructuredFactView, StructuredMemoryRepository,
};
use sqlx::{Row, Sqlite, Transaction};

use crate::db::{SqliteKernel, map_sql};

fn view(row: &sqlx::sqlite::SqliteRow) -> Result<StructuredFactView, PortError> {
    Ok(StructuredFactView {
        id: row.try_get("id").map_err(map_sql)?,
        novel_id: row.try_get("novel_id").map_err(map_sql)?,
        entity_id: row.try_get("entity_id").map_err(map_sql)?,
        fact_type: row.try_get("fact_type").map_err(map_sql)?,
        value_json: row.try_get("value_json").map_err(map_sql)?,
        source_chapter_id: row.try_get("source_chapter_id").map_err(map_sql)?,
        source_revision: row.try_get("source_revision").map_err(map_sql)?,
        evidence_quote: row.try_get("evidence_quote").map_err(map_sql)?,
        truthfulness: row.try_get("truthfulness").map_err(map_sql)?,
        effective_chapter_order: row.try_get("effective_chapter_order").map_err(map_sql)?,
        status: row.try_get("status").map_err(map_sql)?,
        superseded_by: row.try_get("superseded_by").map_err(map_sql)?,
        created_at: row.try_get("created_at").map_err(map_sql)?,
        updated_at: row.try_get("updated_at").map_err(map_sql)?,
    })
}

fn parse_value(raw: &str) -> serde_json::Value {
    serde_json::from_str(raw).unwrap_or_else(|_| serde_json::Value::String(raw.to_string()))
}

fn conflicts(existing: &StructuredFactView, incoming: &NewStructuredFact) -> bool {
    if existing.fact_type != incoming.fact_type
        || existing.effective_chapter_order != incoming.effective_chapter_order
    {
        return false;
    }
    match incoming.fact_type.as_str() {
        "location" => {
            let old = parse_value(&existing.value_json);
            let new = parse_value(&incoming.value_json);
            old.get("location") != new.get("location")
        }
        "life_state" => {
            let old = parse_value(&existing.value_json)
                .get("state")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let new = parse_value(&incoming.value_json)
                .get("state")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            matches!(
                (old.as_str(), new.as_str()),
                ("alive", "dead") | ("dead", "alive")
            )
        }
        "relationship" => {
            let old = parse_value(&existing.value_json);
            let new = parse_value(&incoming.value_json);
            let old_target = old.get("targetEntityId").and_then(|v| v.as_str());
            let new_target = new.get("targetEntityId").and_then(|v| v.as_str());
            let old_relation = old.get("relation").and_then(|v| v.as_str()).unwrap_or("");
            let new_relation = new.get("relation").and_then(|v| v.as_str()).unwrap_or("");
            old_target == new_target
                && matches!(
                    (old_relation, new_relation),
                    ("ally_of", "enemy_of") | ("enemy_of", "ally_of")
                )
        }
        _ => false,
    }
}

async fn verify_source(
    tx: &mut Transaction<'_, Sqlite>,
    fact: &NewStructuredFact,
) -> Result<(), PortError> {
    let character: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM character WHERE id = ? AND novel_id = ?")
            .bind(&fact.entity_id)
            .bind(&fact.novel_id)
            .fetch_one(&mut **tx)
            .await
            .map_err(map_sql)?;
    if character == 0 {
        return Err(PortError::Conflict("人物不属于这本书。".into()));
    }
    let version: Option<i64> = sqlx::query_scalar(
        "SELECT r.version FROM chapter c JOIN chapter_revision r ON r.id = c.official_revision_id
         WHERE c.id = ? AND c.novel_id = ?",
    )
    .bind(&fact.source_chapter_id)
    .bind(&fact.novel_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(map_sql)?;
    match version {
        Some(current) if current == fact.source_revision => Ok(()),
        Some(_) => Err(PortError::Conflict(
            "来源章节已经更新，请重新确认事实。".into(),
        )),
        None => Err(PortError::Conflict("来源章节没有正式正文。".into())),
    }
}

#[async_trait]
impl StructuredMemoryRepository for SqliteKernel {
    async fn list_structured_facts(
        &self,
        novel_id: &str,
        entity_id: Option<&str>,
        as_of_chapter_order: Option<i64>,
    ) -> Result<Vec<StructuredFactView>, PortError> {
        let rows = if let Some(as_of) = as_of_chapter_order {
            sqlx::query(
                "SELECT f.* FROM structured_story_fact f
                 WHERE f.novel_id = ?
                   AND (? IS NULL OR f.entity_id = ?)
                   AND f.truthfulness = 'confirmed'
                   AND f.effective_chapter_order <= ?
                   AND EXISTS (SELECT 1 FROM chapter c JOIN chapter_revision r ON r.id = c.official_revision_id
                               WHERE c.id = f.source_chapter_id AND c.novel_id = f.novel_id AND r.version = f.source_revision)
                   AND NOT EXISTS (
                     SELECT 1 FROM structured_story_fact newer
                     WHERE newer.novel_id = f.novel_id AND newer.entity_id = f.entity_id
                       AND newer.fact_type = f.fact_type AND newer.truthfulness = 'confirmed'
                       AND newer.effective_chapter_order <= ?
                       AND (newer.effective_chapter_order > f.effective_chapter_order
                            OR (newer.effective_chapter_order = f.effective_chapter_order AND
                                (newer.updated_at > f.updated_at
                                 OR (newer.updated_at = f.updated_at AND newer.created_at > f.created_at)
                                 OR (newer.updated_at = f.updated_at AND newer.created_at = f.created_at AND newer.id > f.id)))))
                 ORDER BY f.effective_chapter_order DESC, f.updated_at DESC, f.id DESC",
            ).bind(novel_id).bind(entity_id).bind(entity_id).bind(as_of).bind(as_of)
            .fetch_all(&self.pool).await.map_err(map_sql)?
        } else {
            sqlx::query(
                "SELECT f.* FROM structured_story_fact f
                 WHERE f.novel_id = ?
                   AND (? IS NULL OR f.entity_id = ?)
                   AND EXISTS (SELECT 1 FROM chapter c JOIN chapter_revision r ON r.id = c.official_revision_id
                               WHERE c.id = f.source_chapter_id AND c.novel_id = f.novel_id AND r.version = f.source_revision)
                 ORDER BY f.effective_chapter_order DESC, f.updated_at DESC, f.id DESC",
            ).bind(novel_id).bind(entity_id).bind(entity_id)
            .fetch_all(&self.pool).await.map_err(map_sql)?
        };
        rows.iter().map(view).collect()
    }

    async fn confirm_structured_fact(
        &self,
        fact: NewStructuredFact,
    ) -> Result<StructuredFactView, PortError> {
        let mut tx = self.pool.begin().await.map_err(map_sql)?;
        verify_source(&mut tx, &fact).await?;
        let existing_rows = sqlx::query(
            "SELECT * FROM structured_story_fact
             WHERE novel_id = ? AND entity_id = ? AND fact_type = ?
               AND truthfulness = 'confirmed' AND status = 'active'
               AND effective_chapter_order = ?",
        )
        .bind(&fact.novel_id)
        .bind(&fact.entity_id)
        .bind(&fact.fact_type)
        .bind(fact.effective_chapter_order)
        .fetch_all(&mut *tx)
        .await
        .map_err(map_sql)?;
        let existing: Option<StructuredFactView> = existing_rows.iter().find_map(|row| {
            let view = view(row).ok()?;
            if conflicts(&view, &fact) {
                Some(view)
            } else {
                None
            }
        });
        if let Some(old) = existing {
            let new_view = sqlx::query(
                "INSERT INTO structured_story_fact
                 (id, novel_id, entity_id, fact_type, value_json, source_chapter_id, source_revision,
                  evidence_quote, truthfulness, effective_chapter_order, status, superseded_by, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'proposed', ?, 'disputed', NULL, ?, ?)",
            ).bind(&fact.id).bind(&fact.novel_id).bind(&fact.entity_id).bind(&fact.fact_type)
            .bind(&fact.value_json).bind(&fact.source_chapter_id).bind(fact.source_revision)
            .bind(&fact.evidence_quote).bind(fact.effective_chapter_order).bind(fact.now_ms).bind(fact.now_ms)
            .execute(&mut *tx).await.map_err(map_sql)?;
            let _ = new_view;
            let conflict_id = format!("structured-{}", fact.id);
            let description = format!(
                "同一人物在第{}章存在冲突的{}事实。",
                fact.effective_chapter_order,
                match fact.fact_type.as_str() {
                    "location" => "地点",
                    "life_state" => "生死",
                    "relationship" => "关系",
                    _ => "人物",
                }
            );
            sqlx::query(
                "INSERT INTO memory_conflict
                 (id, novel_id, conflict_type, entity_key, chapter_id, chapter_order,
                  candidate_ids_json, fact_ids_json, status, version, description, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, '[]', ?, 'open', 0, ?, ?, ?)",
            ).bind(&conflict_id).bind(&fact.novel_id).bind(format!("structured_{}", fact.fact_type))
            .bind(&fact.entity_id).bind(&fact.source_chapter_id).bind(fact.effective_chapter_order)
            .bind(serde_json::json!([old.id, fact.id]).to_string()).bind(description)
            .bind(fact.now_ms).bind(fact.now_ms).execute(&mut *tx).await.map_err(map_sql)?;
            let row = sqlx::query("SELECT * FROM structured_story_fact WHERE id = ?")
                .bind(&fact.id)
                .fetch_one(&mut *tx)
                .await
                .map_err(map_sql)?;
            let result = view(&row)?;
            tx.commit().await.map_err(map_sql)?;
            return Ok(result);
        }
        sqlx::query(
            "UPDATE structured_story_fact SET status = 'superseded', superseded_by = ?, updated_at = ?
             WHERE novel_id = ? AND entity_id = ? AND fact_type = ? AND status = 'active'
               AND effective_chapter_order <= ?",
        ).bind(&fact.id).bind(fact.now_ms).bind(&fact.novel_id).bind(&fact.entity_id)
        .bind(&fact.fact_type).bind(fact.effective_chapter_order)
        .execute(&mut *tx).await.map_err(map_sql)?;
        sqlx::query(
            "INSERT INTO structured_story_fact
             (id, novel_id, entity_id, fact_type, value_json, source_chapter_id, source_revision,
              evidence_quote, truthfulness, effective_chapter_order, status, superseded_by, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'confirmed', ?, 'active', NULL, ?, ?)",
        ).bind(&fact.id).bind(&fact.novel_id).bind(&fact.entity_id).bind(&fact.fact_type)
        .bind(&fact.value_json).bind(&fact.source_chapter_id).bind(fact.source_revision)
        .bind(&fact.evidence_quote).bind(fact.effective_chapter_order).bind(fact.now_ms).bind(fact.now_ms)
        .execute(&mut *tx).await.map_err(map_sql)?;
        let row = sqlx::query("SELECT * FROM structured_story_fact WHERE id = ?")
            .bind(&fact.id)
            .fetch_one(&mut *tx)
            .await
            .map_err(map_sql)?;
        let result = view(&row)?;
        tx.commit().await.map_err(map_sql)?;
        Ok(result)
    }

    async fn supersede_structured_fact(
        &self,
        novel_id: &str,
        fact_id: &str,
        replacement_id: &str,
        now_ms: i64,
    ) -> Result<(), PortError> {
        sqlx::query("UPDATE structured_story_fact SET status='superseded', superseded_by=?, updated_at=? WHERE id=? AND novel_id=?")
            .bind(replacement_id).bind(now_ms).bind(fact_id).bind(novel_id)
            .execute(&self.pool).await.map_err(map_sql)?;
        Ok(())
    }
}
