use async_trait::async_trait;
use novel_application::{
    CausalityEdgeView, CreatePlotThreadInput, NewCausalityEdge, PlotRepository,
    PlotThreadEventView, PlotThreadView, PlotTransition, PortError,
};
use sqlx::{Row, Sqlite, Transaction};

use crate::db::{SqliteKernel, map_sql};

fn stored_revision(raw: Option<&str>) -> Option<i64> {
    let raw = raw?.trim();
    if raw.is_empty() {
        return None;
    }
    raw.strip_prefix("memory:")
        .unwrap_or(raw)
        .parse::<i64>()
        .ok()
}

fn plot_view(row: &sqlx::sqlite::SqliteRow) -> Result<PlotThreadView, PortError> {
    let source_revision = stored_revision(
        row.try_get::<Option<String>, _>("source_revision")
            .ok()
            .flatten()
            .as_deref(),
    );
    let source_current_version: Option<i64> =
        row.try_get("source_current_version").map_err(map_sql)?;
    let evidence: Option<String> = row.try_get("evidence_json").map_err(map_sql)?;
    let content: Option<String> = row.try_get("source_content").map_err(map_sql)?;
    let evidence_present = evidence.as_deref().is_some_and(|quote| {
        !quote.trim().is_empty() && content.as_deref().is_some_and(|text| text.contains(quote))
    });
    let source_valid =
        source_revision.is_some() && source_current_version == source_revision && evidence_present;
    let lifecycle_status: String = row.try_get("lifecycle_status").map_err(map_sql)?;
    let stored_status: String = row.try_get("stored_status").map_err(map_sql)?;
    // 最近一次生命周期事件的来源失效时，界面必须看到 stale，而不是悄悄退回更早的状态。
    let status = if !source_valid
        && matches!(
            lifecycle_status.as_str(),
            "open" | "developing" | "deferred" | "resolved"
        ) {
        "stale".to_string()
    } else {
        lifecycle_status.clone()
    };
    Ok(PlotThreadView {
        id: row.try_get("id").map_err(map_sql)?,
        novel_id: row.try_get("novel_id").map_err(map_sql)?,
        title: row.try_get("title").map_err(map_sql)?,
        body: row.try_get("body").map_err(map_sql)?,
        status,
        lifecycle_status,
        stored_status,
        opened_chapter_order: row.try_get("opened_chapter_order").map_err(map_sql)?,
        last_development_chapter_order: row
            .try_get("last_development_chapter_order")
            .map_err(map_sql)?,
        resolved_chapter_order: row.try_get("resolved_chapter_order").map_err(map_sql)?,
        latest_event_chapter_order: row.try_get("latest_event_chapter_order").map_err(map_sql)?,
        source_chapter_id: row.try_get("source_chapter_id").map_err(map_sql)?,
        source_chapter_order: row.try_get("source_chapter_order").map_err(map_sql)?,
        source_chapter_title: row.try_get("source_chapter_title").map_err(map_sql)?,
        source_revision,
        source_current_version,
        evidence,
        evidence_present_in_current: evidence_present,
        source_valid,
        version: row.try_get("version").map_err(map_sql)?,
        created_at: row.try_get("created_at").map_err(map_sql)?,
        updated_at: row.try_get("updated_at").map_err(map_sql)?,
    })
}

async fn verify_plot_source(
    tx: &mut Transaction<'_, Sqlite>,
    novel_id: &str,
    chapter_id: &str,
    source_revision: i64,
    evidence: &str,
    min_chapter_order: i32,
) -> Result<i32, PortError> {
    if evidence.trim().is_empty() {
        return Err(PortError::Conflict("伏笔证据不能为空。".into()));
    }
    let row = sqlx::query(
        "SELECT c.sort_order, r.version, r.content FROM chapter c
         JOIN chapter_revision r ON r.id = c.official_revision_id
         WHERE c.id = ? AND c.novel_id = ?",
    )
    .bind(chapter_id)
    .bind(novel_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(map_sql)?;
    let Some(row) = row else {
        return Err(PortError::Conflict("来源章节没有正式正文。".into()));
    };
    let order: i32 = row.try_get("sort_order").map_err(map_sql)?;
    let current: i64 = row.try_get("version").map_err(map_sql)?;
    if current != source_revision {
        return Err(PortError::Conflict(
            "来源章节已经更新，请重新选取证据。".into(),
        ));
    }
    let content: String = row.try_get("content").map_err(map_sql)?;
    if !content.contains(evidence) {
        return Err(PortError::Conflict(
            "证据不在当前正式正文中，请重新选取证据。".into(),
        ));
    }
    if order < min_chapter_order {
        return Err(PortError::Conflict(
            "伏笔的推进章节不能早于埋设章节。".into(),
        ));
    }
    Ok(order)
}

async fn plot_by_id(
    kernel: &SqliteKernel,
    novel_id: &str,
    plot_id: &str,
) -> Result<PlotThreadView, PortError> {
    let row = sqlx::query(
        "SELECT p.id, p.novel_id, p.title, p.body,
                COALESCE(
                  (SELECT e5.to_status FROM plot_thread_event e5
                   WHERE e5.plot_id = p.id ORDER BY e5.rowid DESC LIMIT 1),
                  p.status
                ) AS lifecycle_status,
                p.status AS stored_status,
                COALESCE(
                  (SELECT c0.sort_order FROM plot_thread_event e0
                   JOIN chapter c0 ON c0.id = e0.chapter_id
                   WHERE e0.plot_id = p.id AND e0.kind = 'setup'
                   ORDER BY e0.rowid LIMIT 1),
                  p.opened_chapter_order
                ) AS opened_chapter_order,
                p.last_development_chapter_order, p.resolved_chapter_order,
                (SELECT MAX(COALESCE(c4.sort_order, e4.chapter_order))
                 FROM plot_thread_event e4
                 LEFT JOIN chapter c4 ON c4.id = e4.chapter_id
                 WHERE e4.plot_id = p.id) AS latest_event_chapter_order,
                p.source_chapter_id, p.source_revision, p.evidence_json,
                c.sort_order AS source_chapter_order, c.title AS source_chapter_title,
                r.version AS source_current_version, r.content AS source_content,
                p.version, p.created_at, p.updated_at
         FROM plot_thread p
         LEFT JOIN chapter c ON c.id = p.source_chapter_id AND c.novel_id = p.novel_id
         LEFT JOIN chapter_revision r ON r.id = c.official_revision_id
         WHERE p.novel_id = ? AND p.id = ?",
    )
    .bind(novel_id)
    .bind(plot_id)
    .fetch_optional(&kernel.pool)
    .await
    .map_err(map_sql)?
    .ok_or_else(|| PortError::NotFound("没有找到这条伏笔。".into()))?;
    plot_view(&row)
}

#[async_trait]
impl PlotRepository for SqliteKernel {
    async fn create_plot_thread(
        &self,
        input: CreatePlotThreadInput,
    ) -> Result<PlotThreadView, PortError> {
        let mut tx = self.pool.begin().await.map_err(map_sql)?;
        if let Some((existing_plot, existing_hash)) = sqlx::query_as::<_, (String, String)>(
            "SELECT plot_id, request_hash FROM plot_thread_event WHERE command_id = ?",
        )
        .bind(&input.command_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sql)?
        {
            if existing_hash != input.request_hash || existing_plot != input.id {
                return Err(PortError::Conflict(
                    "commandId 已用于不同的伏笔请求。".into(),
                ));
            }
            tx.commit().await.map_err(map_sql)?;
            return plot_by_id(self, &input.novel_id, &input.id).await;
        }
        let order = verify_plot_source(
            &mut tx,
            &input.novel_id,
            &input.chapter_id,
            input.source_revision,
            &input.evidence,
            0,
        )
        .await?;
        sqlx::query(
            "INSERT INTO plot_thread
               (id, novel_id, title, body, opened_chapter_order, resolved_chapter_order, status,
                source_revision, evidence_json, created_at, updated_at, source_chapter_id,
                version, last_development_chapter_order)
             VALUES (?, ?, ?, ?, ?, NULL, 'open', ?, ?, ?, ?, ?, 1, NULL)",
        )
        .bind(&input.id)
        .bind(&input.novel_id)
        .bind(input.title.trim())
        .bind(input.body.trim())
        .bind(order)
        .bind(input.source_revision.to_string())
        .bind(&input.evidence)
        .bind(input.now_ms)
        .bind(input.now_ms)
        .bind(&input.chapter_id)
        .execute(&mut *tx)
        .await
        .map_err(map_sql)?;
        sqlx::query(
            "INSERT INTO plot_thread_event
               (id, plot_id, novel_id, kind, chapter_id, chapter_order, source_revision, evidence,
                from_status, to_status, command_id, request_hash, created_at)
             VALUES (?, ?, ?, 'setup', ?, ?, ?, ?, NULL, 'open', ?, ?, ?)",
        )
        .bind(format!("setup-{}", input.id))
        .bind(&input.id)
        .bind(&input.novel_id)
        .bind(&input.chapter_id)
        .bind(order)
        .bind(input.source_revision)
        .bind(&input.evidence)
        .bind(&input.command_id)
        .bind(&input.request_hash)
        .bind(input.now_ms)
        .execute(&mut *tx)
        .await
        .map_err(map_sql)?;
        tx.commit().await.map_err(map_sql)?;
        plot_by_id(self, &input.novel_id, &input.id).await
    }

    async fn transition_plot_thread(
        &self,
        input: PlotTransition,
    ) -> Result<PlotThreadView, PortError> {
        let mut tx = self.pool.begin().await.map_err(map_sql)?;
        if let Some((existing_plot, existing_hash)) = sqlx::query_as::<_, (String, String)>(
            "SELECT plot_id, request_hash FROM plot_thread_event WHERE command_id = ?",
        )
        .bind(&input.command_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sql)?
        {
            if existing_plot != input.plot_id || existing_hash != input.request_hash {
                return Err(PortError::Conflict(
                    "commandId 已用于不同的伏笔动作。".into(),
                ));
            }
            tx.commit().await.map_err(map_sql)?;
            return plot_by_id(self, &input.novel_id, &input.plot_id).await;
        }
        let row = sqlx::query(
            "SELECT p.status, p.version, p.opened_chapter_order,
                    p.last_development_chapter_order, p.source_revision, p.evidence_json,
                    (SELECT c0.sort_order FROM plot_thread_event e0
                     JOIN chapter c0 ON c0.id = e0.chapter_id
                     WHERE e0.plot_id = p.id AND e0.kind = 'setup'
                     ORDER BY e0.rowid LIMIT 1) AS setup_order,
                    r.version AS source_current_version, r.content AS source_content
             FROM plot_thread p
             LEFT JOIN chapter c ON c.id = p.source_chapter_id AND c.novel_id = p.novel_id
             LEFT JOIN chapter_revision r ON r.id = c.official_revision_id
             WHERE p.id = ? AND p.novel_id = ?",
        )
        .bind(&input.plot_id)
        .bind(&input.novel_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sql)?
        .ok_or_else(|| PortError::NotFound("没有找到这条伏笔。".into()))?;
        let stored_status: String = row.try_get("status").map_err(map_sql)?;
        let version: i64 = row.try_get("version").map_err(map_sql)?;
        let stored_opened: i32 = row.try_get("opened_chapter_order").map_err(map_sql)?;
        let setup_order: Option<i32> = row.try_get("setup_order").map_err(map_sql)?;
        let opened_order = setup_order.unwrap_or(stored_opened);
        let last_development: Option<i32> = row
            .try_get("last_development_chapter_order")
            .map_err(map_sql)?;
        if version != input.expected_version {
            return Err(PortError::Conflict("伏笔已经变化，请刷新后再操作。".into()));
        }
        // 存储状态与展示状态必须分开判断：旧数据或章节删除后，界面看到的是 stale，
        // 但数据库里可能仍写着 resolved/deferred；作者必须能用新证据恢复，而不是被卡死。
        let stored_source_revision: Option<String> =
            row.try_get("source_revision").map_err(map_sql)?;
        let stored_evidence: Option<String> = row.try_get("evidence_json").map_err(map_sql)?;
        let source_current_version: Option<i64> =
            row.try_get("source_current_version").map_err(map_sql)?;
        let source_content: Option<String> = row.try_get("source_content").map_err(map_sql)?;
        let parsed_revision = stored_revision(stored_source_revision.as_deref());
        let source_valid = parsed_revision.is_some()
            && parsed_revision == source_current_version
            && stored_evidence.as_deref().is_some_and(|quote| {
                !quote.trim().is_empty()
                    && source_content
                        .as_deref()
                        .is_some_and(|text| text.contains(quote))
            });
        let effective_from = if !source_valid
            && matches!(
                stored_status.as_str(),
                "open" | "developing" | "deferred" | "resolved"
            ) {
            "stale"
        } else {
            stored_status.as_str()
        };
        let allowed: &[&str] = match input.action.as_str() {
            "develop" | "payoff" => &["open", "developing", "deferred", "stale"],
            "defer" => &["open", "developing"],
            "abandon" => &["open", "developing", "deferred", "stale"],
            "recover" => &["stale"],
            _ => return Err(PortError::Integrity("未知的伏笔动作。".into())),
        };
        if !allowed.contains(&effective_from) {
            return Err(PortError::Conflict(format!(
                "当前伏笔状态 {effective_from} 不能执行 {}。",
                input.action
            )));
        }
        let chapter_order = verify_plot_source(
            &mut tx,
            &input.novel_id,
            &input.chapter_id,
            input.source_revision,
            &input.evidence,
            opened_order,
        )
        .await?;
        // 新动作必须发生在已有动作之后，否则新增事件会被更晚的事件遮住，
        // 导致数据库里的当前状态与未来章节的 As-Of 折叠结果不一致。
        let latest_event_order: Option<i32> = sqlx::query_scalar(
            "SELECT MAX(COALESCE(c.sort_order, e.chapter_order))
             FROM plot_thread_event e
             LEFT JOIN chapter c ON c.id = e.chapter_id
             WHERE e.plot_id = ?",
        )
        .bind(&input.plot_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_sql)?;
        if latest_event_order.is_some_and(|latest| chapter_order < latest) {
            return Err(PortError::Conflict(
                "请在最近一次伏笔动作之后的章节继续操作。".into(),
            ));
        }
        let next_status = match input.action.as_str() {
            "develop" => "developing",
            "payoff" => "resolved",
            "defer" => "deferred",
            "abandon" => "abandoned",
            "recover" => "developing",
            _ => unreachable!(),
        };
        let next_development = match input.action.as_str() {
            "develop" | "recover" => Some(last_development.unwrap_or(0).max(chapter_order)),
            _ => last_development,
        };
        // 除回收外的动作清除旧的回收章节：恢复/废弃后的伏笔不能继续显示“回收第 N 章”。
        let next_resolved = if input.action == "payoff" {
            Some(chapter_order)
        } else {
            None
        };
        let updated = sqlx::query(
            "UPDATE plot_thread
             SET status = ?, source_revision = ?, evidence_json = ?, source_chapter_id = ?,
                 last_development_chapter_order = ?, updated_at = ?, version = version + 1,
                 resolved_chapter_order = ?
             WHERE id = ? AND novel_id = ? AND version = ? AND status = ?",
        )
        .bind(next_status)
        .bind(input.source_revision.to_string())
        .bind(&input.evidence)
        .bind(&input.chapter_id)
        .bind(next_development)
        .bind(input.now_ms)
        .bind(next_resolved)
        .bind(&input.plot_id)
        .bind(&input.novel_id)
        .bind(version)
        .bind(&stored_status)
        .execute(&mut *tx)
        .await
        .map_err(map_sql)?;
        if updated.rows_affected() != 1 {
            return Err(PortError::Conflict("伏笔已经变化，请刷新后再操作。".into()));
        }
        sqlx::query(
            "INSERT INTO plot_thread_event
               (id, plot_id, novel_id, kind, chapter_id, chapter_order, source_revision, evidence,
                from_status, to_status, command_id, request_hash, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(format!("transition-{}", input.command_id))
        .bind(&input.plot_id)
        .bind(&input.novel_id)
        .bind(&input.action)
        .bind(&input.chapter_id)
        .bind(chapter_order)
        .bind(input.source_revision)
        .bind(&input.evidence)
        // 记录数据库里真实的前一状态；来源失效只影响“展示状态”和动作白名单。
        .bind(&stored_status)
        .bind(next_status)
        .bind(&input.command_id)
        .bind(&input.request_hash)
        .bind(input.now_ms)
        .execute(&mut *tx)
        .await
        .map_err(map_sql)?;
        tx.commit().await.map_err(map_sql)?;
        plot_by_id(self, &input.novel_id, &input.plot_id).await
    }

    async fn list_plot_threads(
        &self,
        novel_id: &str,
        as_of_chapter_order: i32,
    ) -> Result<Vec<PlotThreadView>, PortError> {
        let rows = sqlx::query(
            // 章节顺序必须按当前目录重新计算：章节重排后，事件的 As-Of 位置跟着变。
            // 同一章同一毫秒的多次动作按 rowid（真实写入顺序）折叠，避免用 commandId 猜先后。
            "WITH plot_base AS (
               SELECT p.*,
                      COALESCE(
                        (SELECT c0.sort_order FROM plot_thread_event e0
                         JOIN chapter c0 ON c0.id = e0.chapter_id
                         WHERE e0.plot_id = p.id AND e0.kind = 'setup'
                         ORDER BY e0.rowid LIMIT 1),
                        p.opened_chapter_order
                      ) AS effective_opened
               FROM plot_thread p
               WHERE p.novel_id = ?
             )
             SELECT p.id, p.novel_id, p.title, p.body,
                    CASE
                      WHEN e.to_status IS NOT NULL THEN e.to_status
                      WHEN p.status = 'resolved'
                           AND (p.resolved_chapter_order IS NULL
                                OR p.resolved_chapter_order > ?)
                        THEN 'open'
                      ELSE p.status
                    END AS lifecycle_status,
                    p.status AS stored_status,
                    p.effective_opened AS opened_chapter_order,
                    (SELECT MAX(COALESCE(c3.sort_order, e3.chapter_order))
                     FROM plot_thread_event e3
                     LEFT JOIN chapter c3 ON c3.id = e3.chapter_id
                     WHERE e3.plot_id = p.id AND e3.kind IN ('develop','recover')
                       AND COALESCE(c3.sort_order, e3.chapter_order) <= ?)
                      AS last_development_chapter_order,
                    CASE
                      WHEN e.to_status = 'resolved'
                        THEN COALESCE(ec.sort_order, e.chapter_order, p.resolved_chapter_order)
                      WHEN e.id IS NULL AND p.status = 'resolved'
                           AND p.resolved_chapter_order IS NOT NULL
                           AND p.resolved_chapter_order <= ?
                        THEN p.resolved_chapter_order
                      ELSE NULL
                    END AS resolved_chapter_order,
                    (SELECT MAX(COALESCE(c4.sort_order, e4.chapter_order))
                     FROM plot_thread_event e4
                     LEFT JOIN chapter c4 ON c4.id = e4.chapter_id
                     WHERE e4.plot_id = p.id) AS latest_event_chapter_order,
                    COALESCE(e.chapter_id, p.source_chapter_id) AS source_chapter_id,
                    COALESCE(CAST(e.source_revision AS TEXT), p.source_revision) AS source_revision,
                    COALESCE(e.evidence, p.evidence_json) AS evidence_json,
                    c.sort_order AS source_chapter_order, c.title AS source_chapter_title,
                    r.version AS source_current_version, r.content AS source_content,
                    p.version, p.created_at, p.updated_at
             FROM plot_base p
             LEFT JOIN plot_thread_event e ON e.id = (
               SELECT e2.id FROM plot_thread_event e2
               LEFT JOIN chapter c2 ON c2.id = e2.chapter_id
               WHERE e2.plot_id = p.id
                 AND COALESCE(c2.sort_order, e2.chapter_order) <= ?
               ORDER BY COALESCE(c2.sort_order, e2.chapter_order) DESC,
                        e2.created_at DESC, e2.rowid DESC
               LIMIT 1
             )
             LEFT JOIN chapter ec ON ec.id = e.chapter_id
             LEFT JOIN chapter c ON c.id = COALESCE(e.chapter_id, p.source_chapter_id)
                                AND c.novel_id = p.novel_id
             LEFT JOIN chapter_revision r ON r.id = c.official_revision_id
             WHERE p.effective_opened <= ?
             ORDER BY p.effective_opened DESC, p.created_at DESC, p.id
             LIMIT 500",
        )
        .bind(novel_id)
        .bind(as_of_chapter_order)
        .bind(as_of_chapter_order)
        .bind(as_of_chapter_order)
        .bind(as_of_chapter_order)
        .bind(as_of_chapter_order)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sql)?;
        rows.iter().map(plot_view).collect()
    }

    async fn list_plot_events(
        &self,
        novel_id: &str,
        as_of_chapter_order: i32,
        limit: i64,
    ) -> Result<Vec<PlotThreadEventView>, PortError> {
        let rows = sqlx::query(
            "SELECT e.id, e.plot_id, p.title, e.kind, e.chapter_id,
                    COALESCE(c.sort_order, e.chapter_order) AS chapter_order,
                    c.title AS chapter_title, e.evidence, e.from_status, e.to_status,
                    CASE WHEN r.version = e.source_revision
                              AND instr(r.content, e.evidence) > 0
                         THEN 1 ELSE 0 END AS source_valid,
                    e.created_at
             FROM plot_thread_event e
             JOIN plot_thread p ON p.id = e.plot_id
             LEFT JOIN chapter c ON c.id = e.chapter_id AND c.novel_id = e.novel_id
             LEFT JOIN chapter_revision r ON r.id = c.official_revision_id
             WHERE e.novel_id = ?
               AND COALESCE(c.sort_order, e.chapter_order) <= ?
             ORDER BY COALESCE(c.sort_order, e.chapter_order) DESC,
                      e.created_at DESC, e.rowid DESC
             LIMIT ?",
        )
        .bind(novel_id)
        .bind(as_of_chapter_order)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sql)?;
        rows.iter()
            .map(|row| {
                Ok(PlotThreadEventView {
                    id: row.try_get("id").map_err(map_sql)?,
                    plot_id: row.try_get("plot_id").map_err(map_sql)?,
                    title: row.try_get("title").map_err(map_sql)?,
                    kind: row.try_get("kind").map_err(map_sql)?,
                    chapter_id: row.try_get("chapter_id").map_err(map_sql)?,
                    chapter_order: row.try_get("chapter_order").map_err(map_sql)?,
                    chapter_title: row.try_get("chapter_title").map_err(map_sql)?,
                    evidence: row.try_get("evidence").map_err(map_sql)?,
                    from_status: row.try_get("from_status").map_err(map_sql)?,
                    to_status: row.try_get("to_status").map_err(map_sql)?,
                    source_valid: row.try_get::<i64, _>("source_valid").map_err(map_sql)? != 0,
                    created_at: row.try_get("created_at").map_err(map_sql)?,
                })
            })
            .collect()
    }

    async fn create_causality_edge(
        &self,
        input: NewCausalityEdge,
    ) -> Result<CausalityEdgeView, PortError> {
        let mut tx = self.pool.begin().await.map_err(map_sql)?;
        for (kind, id) in [
            (input.from_kind.as_str(), input.from_id.as_str()),
            (input.to_kind.as_str(), input.to_id.as_str()),
        ] {
            let exists: i64 = match kind {
                "timeline_event" => sqlx::query_scalar(
                    "SELECT COUNT(*) FROM timeline_event te
                     WHERE te.id = ? AND te.novel_id = ? AND te.status = 'confirmed'
                       AND (te.source_chapter_id IS NULL OR EXISTS (
                         SELECT 1 FROM chapter tc
                         JOIN chapter_revision tr ON tr.id = tc.official_revision_id
                         WHERE tc.id = te.source_chapter_id AND tc.novel_id = te.novel_id
                           AND tr.version = CAST(REPLACE(te.source_revision, 'memory:', '') AS INTEGER)))",
                )
                .bind(id)
                .bind(&input.novel_id)
                .fetch_one(&mut *tx)
                .await
                .map_err(map_sql)?,
                "plot_thread" => sqlx::query_scalar(
                    "SELECT COUNT(*) FROM plot_thread pt
                     WHERE pt.id = ? AND pt.novel_id = ?
                       AND pt.status NOT IN ('superseded','disputed','stale')
                       AND EXISTS (
                         SELECT 1 FROM chapter pc
                         JOIN chapter_revision pr ON pr.id = pc.official_revision_id
                         WHERE pc.id = pt.source_chapter_id AND pc.novel_id = pt.novel_id
                           AND trim(COALESCE(pt.evidence_json, '')) != ''
                           AND pr.version = CAST(REPLACE(pt.source_revision, 'memory:', '') AS INTEGER)
                           AND instr(pr.content, pt.evidence_json) > 0)",
                )
                .bind(id)
                .bind(&input.novel_id)
                .fetch_one(&mut *tx)
                .await
                .map_err(map_sql)?,
                "structured_fact" => sqlx::query_scalar(
                    "SELECT COUNT(*) FROM structured_story_fact sf
                     WHERE sf.id = ? AND sf.novel_id = ?
                       AND sf.truthfulness = 'confirmed' AND sf.status = 'active'
                       AND EXISTS (
                         SELECT 1 FROM chapter sc
                         JOIN chapter_revision sr ON sr.id = sc.official_revision_id
                         WHERE sc.id = sf.source_chapter_id AND sc.novel_id = sf.novel_id
                           AND trim(sf.evidence_quote) != ''
                           AND sr.version = sf.source_revision
                           AND instr(sr.content, sf.evidence_quote) > 0)",
                )
                .bind(id)
                .bind(&input.novel_id)
                .fetch_one(&mut *tx)
                .await
                .map_err(map_sql)?,
                _ => 0,
            };
            if exists == 0 {
                return Err(PortError::Conflict(
                    "因果关系引用了不存在或已失效的实体。".into(),
                ));
            }
        }
        // 循环检测：在已确认边上加入新边，若从 to 能走回 from 则拒绝。
        let edges = sqlx::query(
            "SELECT from_kind, from_id, to_kind, to_id FROM plot_causality_edge
             WHERE novel_id = ? AND status = 'confirmed'",
        )
        .bind(&input.novel_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(map_sql)?;
        let mut adjacency: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        for row in &edges {
            let from = format!(
                "{}:{}",
                row.try_get::<String, _>("from_kind").map_err(map_sql)?,
                row.try_get::<String, _>("from_id").map_err(map_sql)?
            );
            let to = format!(
                "{}:{}",
                row.try_get::<String, _>("to_kind").map_err(map_sql)?,
                row.try_get::<String, _>("to_id").map_err(map_sql)?
            );
            adjacency.entry(from).or_default().push(to);
        }
        let from_key = format!("{}:{}", input.from_kind, input.from_id);
        let to_key = format!("{}:{}", input.to_kind, input.to_id);
        adjacency
            .entry(from_key.clone())
            .or_default()
            .push(to_key.clone());
        let mut stack = vec![to_key];
        let mut seen = std::collections::HashSet::new();
        while let Some(current) = stack.pop() {
            if current == from_key {
                return Err(PortError::Conflict(
                    "因果关系会形成循环，请检查上下游事件。".into(),
                ));
            }
            if !seen.insert(current.clone()) {
                continue;
            }
            if let Some(next) = adjacency.get(&current) {
                stack.extend(next.iter().cloned());
            }
        }
        let chapter_order = verify_plot_source(
            &mut tx,
            &input.novel_id,
            &input.chapter_id,
            input.source_revision,
            &input.evidence,
            0,
        )
        .await?;
        let inserted = sqlx::query(
            "INSERT INTO plot_causality_edge
               (id, novel_id, from_kind, from_id, to_kind, to_id, relation, evidence,
                source_chapter_id, source_revision, status, version, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'confirmed', 1, ?, ?)
             ON CONFLICT(novel_id, from_kind, from_id, to_kind, to_id, relation) DO NOTHING",
        )
        .bind(&input.id)
        .bind(&input.novel_id)
        .bind(&input.from_kind)
        .bind(&input.from_id)
        .bind(&input.to_kind)
        .bind(&input.to_id)
        .bind(input.relation.trim())
        .bind(&input.evidence)
        .bind(&input.chapter_id)
        .bind(input.source_revision)
        .bind(input.now_ms)
        .bind(input.now_ms)
        .execute(&mut *tx)
        .await
        .map_err(map_sql)?;
        if inserted.rows_affected() == 0 {
            return Err(PortError::Conflict("这条因果关系已经存在。".into()));
        }
        tx.commit().await.map_err(map_sql)?;
        let _ = chapter_order;
        Ok(CausalityEdgeView {
            id: input.id,
            novel_id: input.novel_id,
            from_kind: input.from_kind,
            from_id: input.from_id,
            from_label: None,
            from_valid: true,
            to_kind: input.to_kind,
            to_id: input.to_id,
            to_label: None,
            to_valid: true,
            relation: input.relation,
            evidence: input.evidence,
            source_chapter_id: input.chapter_id,
            source_chapter_order: None,
            source_revision: input.source_revision,
            source_current_version: None,
            source_valid: true,
            status: "confirmed".into(),
            version: 1,
            created_at: input.now_ms,
            updated_at: input.now_ms,
        })
    }

    async fn list_causality_edges(
        &self,
        novel_id: &str,
    ) -> Result<Vec<CausalityEdgeView>, PortError> {
        let rows = sqlx::query(
            "SELECT e.id, e.novel_id, e.from_kind, e.from_id, e.to_kind, e.to_id, e.relation,
                    e.evidence, e.source_chapter_id, e.source_revision, e.status, e.version,
                    e.created_at, e.updated_at,
                    c.sort_order AS source_chapter_order, r.version AS source_current_version,
                    r.content AS source_content,
                    CASE WHEN e.from_kind = 'timeline_event' THEN
                        (SELECT body FROM timeline_event WHERE id = e.from_id)
                     WHEN e.from_kind = 'plot_thread' THEN
                        (SELECT title FROM plot_thread WHERE id = e.from_id)
                     WHEN e.from_kind = 'structured_fact' THEN
                        (SELECT fact_type || '：' || value_json FROM structured_story_fact WHERE id = e.from_id)
                     END AS from_label,
                    CASE WHEN e.to_kind = 'timeline_event' THEN
                        (SELECT body FROM timeline_event WHERE id = e.to_id)
                     WHEN e.to_kind = 'plot_thread' THEN
                        (SELECT title FROM plot_thread WHERE id = e.to_id)
                     WHEN e.to_kind = 'structured_fact' THEN
                        (SELECT fact_type || '：' || value_json FROM structured_story_fact WHERE id = e.to_id)
                     END AS to_label,
                    CASE
                      WHEN e.from_kind = 'timeline_event' THEN EXISTS(
                        SELECT 1 FROM timeline_event te
                        WHERE te.id = e.from_id AND te.novel_id = e.novel_id
                          AND te.status = 'confirmed'
                          AND (te.source_chapter_id IS NULL OR EXISTS(
                            SELECT 1 FROM chapter tc
                            JOIN chapter_revision tr ON tr.id = tc.official_revision_id
                            WHERE tc.id = te.source_chapter_id AND tc.novel_id = te.novel_id
                              AND tr.version = CAST(REPLACE(te.source_revision, 'memory:', '') AS INTEGER))))
                      WHEN e.from_kind = 'plot_thread' THEN EXISTS(
                        SELECT 1 FROM plot_thread pt
                        JOIN chapter pc ON pc.id = pt.source_chapter_id AND pc.novel_id = pt.novel_id
                        JOIN chapter_revision pr ON pr.id = pc.official_revision_id
                        WHERE pt.id = e.from_id AND pt.novel_id = e.novel_id
                          AND pt.status NOT IN ('superseded','stale','disputed')
                          AND trim(COALESCE(pt.evidence_json, '')) != ''
                          AND pr.version = CAST(REPLACE(pt.source_revision, 'memory:', '') AS INTEGER)
                          AND instr(pr.content, pt.evidence_json) > 0)
                      WHEN e.from_kind = 'structured_fact' THEN EXISTS(
                        SELECT 1 FROM structured_story_fact sf
                        JOIN chapter sc ON sc.id = sf.source_chapter_id
                        JOIN chapter_revision sr ON sr.id = sc.official_revision_id
                        WHERE sf.id = e.from_id AND sf.novel_id = e.novel_id
                          AND sf.truthfulness = 'confirmed' AND sf.status = 'active'
                          AND trim(sf.evidence_quote) != ''
                          AND sr.version = sf.source_revision
                          AND instr(sr.content, sf.evidence_quote) > 0)
                      ELSE 0
                    END AS from_valid,
                    CASE
                      WHEN e.to_kind = 'timeline_event' THEN EXISTS(
                        SELECT 1 FROM timeline_event te
                        WHERE te.id = e.to_id AND te.novel_id = e.novel_id
                          AND te.status = 'confirmed'
                          AND (te.source_chapter_id IS NULL OR EXISTS(
                            SELECT 1 FROM chapter tc
                            JOIN chapter_revision tr ON tr.id = tc.official_revision_id
                            WHERE tc.id = te.source_chapter_id AND tc.novel_id = te.novel_id
                              AND tr.version = CAST(REPLACE(te.source_revision, 'memory:', '') AS INTEGER))))
                      WHEN e.to_kind = 'plot_thread' THEN EXISTS(
                        SELECT 1 FROM plot_thread pt
                        JOIN chapter pc ON pc.id = pt.source_chapter_id AND pc.novel_id = pt.novel_id
                        JOIN chapter_revision pr ON pr.id = pc.official_revision_id
                        WHERE pt.id = e.to_id AND pt.novel_id = e.novel_id
                          AND pt.status NOT IN ('superseded','stale','disputed')
                          AND trim(COALESCE(pt.evidence_json, '')) != ''
                          AND pr.version = CAST(REPLACE(pt.source_revision, 'memory:', '') AS INTEGER)
                          AND instr(pr.content, pt.evidence_json) > 0)
                      WHEN e.to_kind = 'structured_fact' THEN EXISTS(
                        SELECT 1 FROM structured_story_fact sf
                        JOIN chapter sc ON sc.id = sf.source_chapter_id
                        JOIN chapter_revision sr ON sr.id = sc.official_revision_id
                        WHERE sf.id = e.to_id AND sf.novel_id = e.novel_id
                          AND sf.truthfulness = 'confirmed' AND sf.status = 'active'
                          AND trim(sf.evidence_quote) != ''
                          AND sr.version = sf.source_revision
                          AND instr(sr.content, sf.evidence_quote) > 0)
                      ELSE 0
                    END AS to_valid
             FROM plot_causality_edge e
             LEFT JOIN chapter c ON c.id = e.source_chapter_id AND c.novel_id = e.novel_id
             LEFT JOIN chapter_revision r ON r.id = c.official_revision_id
             WHERE e.novel_id = ?
             ORDER BY e.created_at, e.id",
        )
        .bind(novel_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sql)?;
        rows.iter()
            .map(|row| {
                let source_version: i64 = row.try_get("source_revision").map_err(map_sql)?;
                let current: Option<i64> =
                    row.try_get("source_current_version").map_err(map_sql)?;
                let content: Option<String> = row.try_get("source_content").map_err(map_sql)?;
                let evidence: String = row.try_get("evidence").map_err(map_sql)?;
                let evidence_present = !evidence.trim().is_empty()
                    && content
                        .as_deref()
                        .is_some_and(|text| text.contains(&evidence));
                Ok(CausalityEdgeView {
                    id: row.try_get("id").map_err(map_sql)?,
                    novel_id: row.try_get("novel_id").map_err(map_sql)?,
                    from_kind: row.try_get("from_kind").map_err(map_sql)?,
                    from_id: row.try_get("from_id").map_err(map_sql)?,
                    from_label: row.try_get("from_label").map_err(map_sql)?,
                    from_valid: row.try_get::<i64, _>("from_valid").map_err(map_sql)? != 0,
                    to_kind: row.try_get("to_kind").map_err(map_sql)?,
                    to_id: row.try_get("to_id").map_err(map_sql)?,
                    to_label: row.try_get("to_label").map_err(map_sql)?,
                    to_valid: row.try_get::<i64, _>("to_valid").map_err(map_sql)? != 0,
                    relation: row.try_get("relation").map_err(map_sql)?,
                    evidence,
                    source_chapter_id: row.try_get("source_chapter_id").map_err(map_sql)?,
                    source_chapter_order: row.try_get("source_chapter_order").map_err(map_sql)?,
                    source_revision: source_version,
                    source_current_version: current,
                    source_valid: current == Some(source_version) && evidence_present,
                    status: row.try_get("status").map_err(map_sql)?,
                    version: row.try_get("version").map_err(map_sql)?,
                    created_at: row.try_get("created_at").map_err(map_sql)?,
                    updated_at: row.try_get("updated_at").map_err(map_sql)?,
                })
            })
            .collect()
    }
}
