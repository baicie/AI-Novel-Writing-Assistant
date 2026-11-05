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
    let evidence_present = evidence
        .as_deref()
        .is_some_and(|quote| content.as_deref().is_some_and(|text| text.contains(quote)));
    let source_valid =
        source_revision.is_some() && source_current_version == source_revision && evidence_present;
    let stored_status: String = row.try_get("status").map_err(map_sql)?;
    // 最近一次生命周期事件的来源失效时，界面必须看到 stale，而不是悄悄退回更早的状态。
    let status = if !source_valid
        && matches!(
            stored_status.as_str(),
            "open" | "developing" | "deferred" | "resolved"
        ) {
        "stale".to_string()
    } else {
        stored_status
    };
    Ok(PlotThreadView {
        id: row.try_get("id").map_err(map_sql)?,
        novel_id: row.try_get("novel_id").map_err(map_sql)?,
        title: row.try_get("title").map_err(map_sql)?,
        body: row.try_get("body").map_err(map_sql)?,
        status,
        opened_chapter_order: row.try_get("opened_chapter_order").map_err(map_sql)?,
        last_development_chapter_order: row
            .try_get("last_development_chapter_order")
            .map_err(map_sql)?,
        resolved_chapter_order: row.try_get("resolved_chapter_order").map_err(map_sql)?,
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
        "SELECT p.id, p.novel_id, p.title, p.body, p.status, p.opened_chapter_order,
                p.last_development_chapter_order, p.resolved_chapter_order,
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
            "SELECT status, version, opened_chapter_order, last_development_chapter_order
             FROM plot_thread WHERE id = ? AND novel_id = ?",
        )
        .bind(&input.plot_id)
        .bind(&input.novel_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sql)?
        .ok_or_else(|| PortError::NotFound("没有找到这条伏笔。".into()))?;
        let from_status: String = row.try_get("status").map_err(map_sql)?;
        let version: i64 = row.try_get("version").map_err(map_sql)?;
        let opened_order: i32 = row.try_get("opened_chapter_order").map_err(map_sql)?;
        let last_development: Option<i32> = row
            .try_get("last_development_chapter_order")
            .map_err(map_sql)?;
        if version != input.expected_version {
            return Err(PortError::Conflict("伏笔已经变化，请刷新后再操作。".into()));
        }
        let allowed: &[&str] = match input.action.as_str() {
            "develop" | "payoff" => &["open", "developing", "deferred", "stale"],
            "defer" => &["open", "developing"],
            "abandon" => &["open", "developing", "deferred", "stale"],
            "recover" => &["stale"],
            _ => return Err(PortError::Integrity("未知的伏笔动作。".into())),
        };
        if !allowed.contains(&from_status.as_str()) {
            return Err(PortError::Conflict(format!(
                "当前伏笔状态 {from_status} 不能执行 {}。",
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
        let next_resolved = if input.action == "payoff" {
            Some(chapter_order)
        } else {
            None
        };
        sqlx::query(
            "UPDATE plot_thread
             SET status = ?, source_revision = ?, evidence_json = ?, source_chapter_id = ?,
                 last_development_chapter_order = ?, updated_at = ?, version = version + 1,
                 resolved_chapter_order = CASE WHEN ? IS NULL THEN resolved_chapter_order ELSE ? END
             WHERE id = ? AND novel_id = ? AND version = ? AND status = ?",
        )
        .bind(next_status)
        .bind(input.source_revision.to_string())
        .bind(&input.evidence)
        .bind(&input.chapter_id)
        .bind(next_development)
        .bind(input.now_ms)
        .bind(next_resolved)
        .bind(next_resolved)
        .bind(&input.plot_id)
        .bind(&input.novel_id)
        .bind(version)
        .bind(&from_status)
        .execute(&mut *tx)
        .await
        .map_err(map_sql)?;
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
        .bind(&from_status)
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
            "SELECT p.id, p.novel_id, p.title, p.body,
                    COALESCE(e.to_status, p.status) AS status,
                    p.opened_chapter_order,
                    (SELECT MAX(e3.chapter_order) FROM plot_thread_event e3
                     WHERE e3.plot_id = p.id AND e3.kind IN ('develop','recover')
                       AND e3.chapter_order <= ?) AS last_development_chapter_order,
                    CASE WHEN COALESCE(e.to_status, p.status) = 'resolved'
                         THEN COALESCE(e.chapter_order, p.resolved_chapter_order)
                         ELSE NULL END AS resolved_chapter_order,
                    COALESCE(e.chapter_id, p.source_chapter_id) AS source_chapter_id,
                    COALESCE(CAST(e.source_revision AS TEXT), p.source_revision) AS source_revision,
                    COALESCE(e.evidence, p.evidence_json) AS evidence_json,
                    c.sort_order AS source_chapter_order, c.title AS source_chapter_title,
                    r.version AS source_current_version, r.content AS source_content,
                    p.version, p.created_at, p.updated_at
             FROM plot_thread p
             LEFT JOIN plot_thread_event e ON e.id = (
               SELECT e2.id FROM plot_thread_event e2
               WHERE e2.plot_id = p.id AND e2.chapter_order <= ?
               ORDER BY e2.chapter_order DESC, e2.created_at DESC, e2.id DESC LIMIT 1
             )
             LEFT JOIN chapter c ON c.id = COALESCE(e.chapter_id, p.source_chapter_id)
                                AND c.novel_id = p.novel_id
             LEFT JOIN chapter_revision r ON r.id = c.official_revision_id
             WHERE p.novel_id = ? AND p.opened_chapter_order <= ?
             ORDER BY p.opened_chapter_order DESC, p.created_at DESC, p.id
             LIMIT 500",
        )
        .bind(as_of_chapter_order)
        .bind(as_of_chapter_order)
        .bind(novel_id)
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
            "SELECT e.id, e.plot_id, p.title, e.kind, e.chapter_id, e.chapter_order,
                    c.title AS chapter_title, e.evidence, e.from_status, e.to_status, e.created_at
             FROM plot_thread_event e
             JOIN plot_thread p ON p.id = e.plot_id
             LEFT JOIN chapter c ON c.id = e.chapter_id
             WHERE e.novel_id = ? AND e.chapter_order <= ?
             ORDER BY e.chapter_order DESC, e.created_at DESC, e.id
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
                    "SELECT COUNT(*) FROM timeline_event WHERE id = ? AND novel_id = ?",
                )
                .bind(id)
                .bind(&input.novel_id)
                .fetch_one(&mut *tx)
                .await
                .map_err(map_sql)?,
                "plot_thread" => sqlx::query_scalar(
                    "SELECT COUNT(*) FROM plot_thread WHERE id = ? AND novel_id = ?",
                )
                .bind(id)
                .bind(&input.novel_id)
                .fetch_one(&mut *tx)
                .await
                .map_err(map_sql)?,
                "structured_fact" => sqlx::query_scalar(
                    "SELECT COUNT(*) FROM structured_story_fact
                     WHERE id = ? AND novel_id = ? AND truthfulness = 'confirmed'",
                )
                .bind(id)
                .bind(&input.novel_id)
                .fetch_one(&mut *tx)
                .await
                .map_err(map_sql)?,
                _ => 0,
            };
            if exists == 0 {
                return Err(PortError::Conflict("因果关系引用了不存在的实体。".into()));
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
            to_kind: input.to_kind,
            to_id: input.to_id,
            to_label: None,
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
                     END AS to_label
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
                let evidence_present = content
                    .as_deref()
                    .is_some_and(|text| text.contains(&evidence));
                Ok(CausalityEdgeView {
                    id: row.try_get("id").map_err(map_sql)?,
                    novel_id: row.try_get("novel_id").map_err(map_sql)?,
                    from_kind: row.try_get("from_kind").map_err(map_sql)?,
                    from_id: row.try_get("from_id").map_err(map_sql)?,
                    from_label: row.try_get("from_label").map_err(map_sql)?,
                    to_kind: row.try_get("to_kind").map_err(map_sql)?,
                    to_id: row.try_get("to_id").map_err(map_sql)?,
                    to_label: row.try_get("to_label").map_err(map_sql)?,
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
