use std::ops::DerefMut;
use std::path::Path;

use novel_application::PortError;
use novel_domain::{PauseKind, RunPause, StopSignalKind};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::Row;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool};

use crate::db::{SqliteKernel, map_sql};

pub const LEGACY_SCHEMA_COMMIT: &str = "8e848e6e";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UnsupportedItem {
    pub table: String,
    pub column: Option<String>,
    pub legacy_id: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MigrationReport {
    pub source_commit: String,
    pub status: String,
    pub novels: usize,
    pub chapters: usize,
    pub runs_paused_for_review: usize,
    pub unsupported: Vec<UnsupportedItem>,
}

pub async fn import_legacy(
    source: &Path,
    target: &Path,
    now_ms: i64,
) -> Result<MigrationReport, PortError> {
    if source == target {
        return Err(PortError::Conflict("不能把旧库直接当成新库来改。".into()));
    }
    let source_pool =
        SqlitePool::connect_with(SqliteConnectOptions::new().filename(source).read_only(true))
            .await
            .map_err(map_sql)?;
    sqlx::query("PRAGMA query_only = ON")
        .execute(&source_pool)
        .await
        .map_err(map_sql)?;
    let target_kernel = SqliteKernel::open(target).await?;
    let mut report = MigrationReport {
        source_commit: LEGACY_SCHEMA_COMMIT.into(),
        status: "verified".into(),
        novels: 0,
        chapters: 0,
        runs_paused_for_review: 0,
        unsupported: Vec::new(),
    };
    collect_gaps(&source_pool, &mut report).await?;
    let mut tx = target_kernel.pool().begin().await.map_err(map_sql)?;
    import_novels(&source_pool, &mut tx, now_ms, &mut report).await?;
    import_chapters(&source_pool, &mut tx, now_ms, &mut report).await?;
    import_runs(&source_pool, &mut tx, now_ms, &mut report).await?;
    if !report.unsupported.is_empty() {
        report.status = "completed_with_gaps".into();
    }
    let report_json = serde_json::to_string(&report)
        .map_err(|_| PortError::Integrity("迁移报告无法保存。".into()))?;
    sqlx::query("INSERT INTO migration_run(id, source_path, source_commit, started_at, finished_at, status, report_json) VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(format!("migration-{now_ms}"))
        .bind(source.display().to_string())
        .bind(LEGACY_SCHEMA_COMMIT)
        .bind(now_ms)
        .bind(now_ms)
        .bind(&report.status)
        .bind(report_json)
        .execute(tx.deref_mut())
        .await
        .map_err(map_sql)?;
    tx.commit().await.map_err(map_sql)?;
    Ok(report)
}

async fn collect_gaps(source: &SqlitePool, report: &mut MigrationReport) -> Result<(), PortError> {
    let tables = table_names(source).await?;
    for table in &tables {
        if matches!(
            table.as_str(),
            "Novel" | "Chapter" | "DirectorNextRun" | "DirectorNextRunControl"
        ) {
            let allowed: &[&str] = match table.as_str() {
                "Novel" => &["id", "title", "description", "status"],
                "Chapter" => &["id", "novelId", "title", "content", "order"],
                "DirectorNextRun" => &["id", "novelId", "planVersion", "contractJson"],
                "DirectorNextRunControl" => &[
                    "runId",
                    "novelId",
                    "version",
                    "status",
                    "pauseJson",
                    "failureReason",
                ],
                _ => &[],
            };
            for column in columns(source, table).await? {
                if !allowed.contains(&column.as_str()) {
                    report.unsupported.push(UnsupportedItem {
                        table: table.clone(),
                        column: Some(column),
                        legacy_id: None,
                        reason: "这一列还没有进入新的本地资料，原库备份里仍然保留。".into(),
                    });
                }
            }
        } else if !table.starts_with("sqlite_") {
            report.unsupported.push(UnsupportedItem {
                table: table.clone(),
                column: None,
                legacy_id: None,
                reason: "这张旧表不在第一阶段可导入的创作资料里。".into(),
            });
        }
    }
    Ok(())
}

async fn import_novels(
    source: &SqlitePool,
    tx: &mut crate::db::Tx<'_>,
    now_ms: i64,
    report: &mut MigrationReport,
) -> Result<(), PortError> {
    if !table_names(source)
        .await?
        .iter()
        .any(|table| table == "Novel")
    {
        return Ok(());
    }
    let rows = sqlx::query("SELECT id, title, description, status FROM Novel")
        .fetch_all(source)
        .await
        .map_err(map_sql)?;
    for row in rows {
        let id: String = row.try_get("id").map_err(map_sql)?;
        if mapped(tx, "Novel", &id).await? {
            report.novels += 1;
            continue;
        }
        let title: String = row.try_get("title").map_err(map_sql)?;
        let description: Option<String> = row.try_get("description").map_err(map_sql)?;
        let status: Option<String> = row.try_get("status").map_err(map_sql)?;
        sqlx::query("INSERT INTO novel(id, title, description, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(&id)
            .bind(title)
            .bind(description.unwrap_or_default())
            .bind(status.unwrap_or_else(|| "draft".into()))
            .bind(now_ms)
            .bind(now_ms)
            .execute(tx.deref_mut())
            .await
            .map_err(map_sql)?;
        remember(tx, "Novel", &id, "novel", &id, None).await?;
        report.novels += 1;
    }
    Ok(())
}

async fn import_chapters(
    source: &SqlitePool,
    tx: &mut crate::db::Tx<'_>,
    now_ms: i64,
    report: &mut MigrationReport,
) -> Result<(), PortError> {
    if !table_names(source)
        .await?
        .iter()
        .any(|table| table == "Chapter")
    {
        return Ok(());
    }
    let rows =
        sqlx::query("SELECT id, novelId, title, content, \"order\" AS sort_order FROM Chapter")
            .fetch_all(source)
            .await
            .map_err(map_sql)?;
    for row in rows {
        let id: String = row.try_get("id").map_err(map_sql)?;
        if mapped(tx, "Chapter", &id).await? {
            report.chapters += 1;
            continue;
        }
        let novel_id: String = row.try_get("novelId").map_err(map_sql)?;
        let title: String = row.try_get("title").map_err(map_sql)?;
        let content: Option<String> = row.try_get("content").map_err(map_sql)?;
        let sort_order: i64 = row.try_get("sort_order").map_err(map_sql)?;
        let content = content.unwrap_or_default();
        let hash = hex_sha256(content.as_bytes());
        let protection = if content.is_empty() {
            "none"
        } else {
            "user_edited"
        };
        sqlx::query("INSERT INTO chapter(id, novel_id, title, sort_order, protection, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(&id)
            .bind(&novel_id)
            .bind(&title)
            .bind(sort_order)
            .bind(protection)
            .bind(now_ms)
            .bind(now_ms)
            .execute(tx.deref_mut())
            .await
            .map_err(map_sql)?;
        if !content.is_empty() {
            let revision_id = format!("legacy-revision-{id}");
            sqlx::query("INSERT INTO chapter_revision(id, chapter_id, version, origin, content, content_sha256, created_at) VALUES (?, ?, 1, 'user', ?, ?, ?)")
                .bind(&revision_id)
                .bind(&id)
                .bind(&content)
                .bind(&hash)
                .bind(now_ms)
                .execute(tx.deref_mut())
                .await
                .map_err(map_sql)?;
            sqlx::query("UPDATE chapter SET official_revision_id = ? WHERE id = ?")
                .bind(&revision_id)
                .bind(&id)
                .execute(tx.deref_mut())
                .await
                .map_err(map_sql)?;
        }
        remember(tx, "Chapter", &id, "chapter", &id, Some(&hash)).await?;
        report.chapters += 1;
    }
    Ok(())
}

async fn import_runs(
    source: &SqlitePool,
    tx: &mut crate::db::Tx<'_>,
    now_ms: i64,
    report: &mut MigrationReport,
) -> Result<(), PortError> {
    let tables = table_names(source).await?;
    if !tables.iter().any(|table| table == "DirectorNextRun")
        || !tables.iter().any(|table| table == "DirectorNextRunControl")
    {
        return Ok(());
    }
    let rows = sqlx::query(
        "SELECT r.id, r.novelId, r.planVersion, r.contractJson, c.status
         FROM DirectorNextRun r JOIN DirectorNextRunControl c ON c.runId = r.id",
    )
    .fetch_all(source)
    .await
    .map_err(map_sql)?;
    for row in rows {
        let id: String = row.try_get("id").map_err(map_sql)?;
        if mapped(tx, "DirectorNextRun", &id).await? {
            continue;
        }
        let novel_id: String = row.try_get("novelId").map_err(map_sql)?;
        let plan_version: String = row.try_get("planVersion").map_err(map_sql)?;
        let contract: String = row.try_get("contractJson").map_err(map_sql)?;
        let status: String = row.try_get("status").map_err(map_sql)?;
        let (mapped_status, pause) = map_legacy_status(&status);
        if pause
            .as_ref()
            .is_some_and(|item| item.signal == StopSignalKind::MigrationReviewRequired)
        {
            report.runs_paused_for_review += 1;
        }
        if status != "running"
            && status != "waiting_gate"
            && status != "paused"
            && status != "completed"
            && status != "failed"
            && status != "cancelled"
            && status != "queued"
        {
            report.unsupported.push(UnsupportedItem {
                table: "DirectorNextRunControl".into(),
                column: Some("status".into()),
                legacy_id: Some(id.clone()),
                reason: "无法识别的旧运行状态已改为等待人工查看，没有当成完成。".into(),
            });
        }
        let pause_json = pause
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|_| PortError::Integrity("迁移暂停原因无法保存。".into()))?;
        sqlx::query("INSERT INTO workflow_run(id, novel_id, plan_version, contract_json, created_at) VALUES (?, ?, ?, ?, ?)")
            .bind(&id)
            .bind(&novel_id)
            .bind(plan_version)
            .bind(contract)
            .bind(now_ms)
            .execute(tx.deref_mut())
            .await
            .map_err(map_sql)?;
        sqlx::query("INSERT INTO workflow_control(run_id, novel_id, status, version, pause_json) VALUES (?, ?, ?, 0, ?)")
            .bind(&id)
            .bind(&novel_id)
            .bind(mapped_status)
            .bind(pause_json)
            .execute(tx.deref_mut())
            .await
            .map_err(map_sql)?;
        remember(tx, "DirectorNextRun", &id, "workflow_run", &id, None).await?;
    }
    Ok(())
}

fn map_legacy_status(status: &str) -> (&'static str, Option<RunPause>) {
    let review = RunPause {
        kind: PauseKind::MigrationReview,
        signal: StopSignalKind::MigrationReviewRequired,
        reason: "旧的创作进度无法安全续跑，请确认后再继续。".into(),
    };
    match status {
        "queued" => ("queued", None),
        "running" => ("paused", Some(review)),
        "waiting_gate" => ("waiting_approval", None),
        "paused" => ("paused", None),
        "completed" => ("succeeded", None),
        "failed" => ("failed", None),
        "cancelled" => ("cancelled", None),
        _ => ("paused", Some(review)),
    }
}

async fn table_names(pool: &SqlitePool) -> Result<Vec<String>, PortError> {
    let rows = sqlx::query("SELECT name FROM sqlite_master WHERE type = 'table'")
        .fetch_all(pool)
        .await
        .map_err(map_sql)?;
    rows.into_iter()
        .map(|row| row.try_get("name").map_err(map_sql))
        .collect()
}

async fn columns(pool: &SqlitePool, table: &str) -> Result<Vec<String>, PortError> {
    if !table
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return Err(PortError::Integrity("旧表名无法识别。".into()));
    }
    let rows = sqlx::query(&format!("PRAGMA table_info(\"{table}\")"))
        .fetch_all(pool)
        .await
        .map_err(map_sql)?;
    rows.into_iter()
        .map(|row| row.try_get("name").map_err(map_sql))
        .collect()
}

async fn mapped(tx: &mut crate::db::Tx<'_>, table: &str, id: &str) -> Result<bool, PortError> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM legacy_id_map WHERE legacy_table = ? AND legacy_id = ?",
    )
    .bind(table)
    .bind(id)
    .fetch_one(tx.deref_mut())
    .await
    .map_err(map_sql)?;
    Ok(count > 0)
}

async fn remember(
    tx: &mut crate::db::Tx<'_>,
    legacy_table: &str,
    legacy_id: &str,
    new_table: &str,
    new_id: &str,
    hash: Option<&str>,
) -> Result<(), PortError> {
    sqlx::query("INSERT INTO legacy_id_map(legacy_table, legacy_id, new_table, new_id, content_sha256) VALUES (?, ?, ?, ?, ?)")
        .bind(legacy_table)
        .bind(legacy_id)
        .bind(new_table)
        .bind(new_id)
        .bind(hash)
        .execute(tx.deref_mut())
        .await
        .map_err(map_sql)?;
    Ok(())
}

fn hex_sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
