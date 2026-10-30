use std::ops::DerefMut;
use std::path::Path;
use std::time::Duration;

use novel_application::PortError;
use novel_prompt::builtin_prompts;
use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteSynchronous,
};
use sqlx::{Row, Sqlite};

pub type Tx<'a> = sqlx::Transaction<'a, Sqlite>;

pub struct SqliteKernel {
    pub(crate) pool: SqlitePool,
    pub(crate) index_write_lock: tokio::sync::Mutex<()>,
}

impl SqliteKernel {
    pub async fn open(path: &Path) -> Result<Self, PortError> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            // WAL + NORMAL is the durable-across-process-crash mode used by the
            // local draft journal. It is not a promise against sudden power loss.
            .synchronous(SqliteSynchronous::Normal)
            .foreign_keys(true)
            .busy_timeout(Duration::from_millis(5000));
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await
            .map_err(map_sql)?;
        migrate(&pool).await?;
        let kernel = Self {
            pool,
            index_write_lock: tokio::sync::Mutex::new(()),
        };
        kernel.seed_prompts().await?;
        Ok(kernel)
    }

    /// Test-only: open a database with only the first `through` migrations applied.
    /// Used to build historical-schema fixtures for upgrade tests.
    #[cfg(feature = "test-harness")]
    pub async fn open_at_migration(path: &Path, through: usize) -> Result<Self, PortError> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .foreign_keys(true)
            .busy_timeout(Duration::from_millis(5000));
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await
            .map_err(map_sql)?;
        ensure_schema_migration_table(&pool).await?;
        for (version, script) in MIGRATION_SCRIPTS.iter().take(through) {
            if migration_applied(&pool, version).await? {
                continue;
            }
            let now_ms: i64 =
                sqlx::query_scalar("SELECT CAST(strftime('%s','now')*1000 AS INTEGER)")
                    .fetch_one(&pool)
                    .await
                    .map_err(map_sql)?;
            let mut tx = pool.begin().await.map_err(map_sql)?;
            for statement in split_sql(script) {
                sqlx::query(&statement)
                    .execute(tx.deref_mut())
                    .await
                    .map_err(map_sql)?;
            }
            sqlx::query("INSERT INTO schema_migration(version, applied_at) VALUES (?, ?)")
                .bind(version)
                .bind(now_ms)
                .execute(tx.deref_mut())
                .await
                .map_err(map_sql)?;
            tx.commit().await.map_err(map_sql)?;
        }
        Ok(Self {
            pool,
            index_write_lock: tokio::sync::Mutex::new(()),
        })
    }

    /// Create a consistent backup using SQLite's VACUUM INTO.
    pub async fn backup_to(&self, path: &Path) -> Result<(), PortError> {
        sqlx::query("VACUUM INTO ?")
            .bind(path.to_string_lossy().to_string())
            .execute(&self.pool)
            .await
            .map_err(map_sql)?;
        Ok(())
    }

    pub fn latest_schema_version() -> &'static str {
        MIGRATION_SCRIPTS
            .last()
            .map(|(version, _)| *version)
            .unwrap_or("0000")
    }

    pub async fn schema_version(path: &Path) -> Result<String, PortError> {
        let options = SqliteConnectOptions::new().filename(path).read_only(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .map_err(map_sql)?;
        let version: Option<String> =
            sqlx::query_scalar("SELECT MAX(version) FROM schema_migration")
                .fetch_one(&pool)
                .await
                .map_err(map_sql)?;
        pool.close().await;
        Ok(version.unwrap_or_else(|| "0000".into()))
    }

    pub async fn verify_database_file(path: &Path) -> Result<String, PortError> {
        let options = SqliteConnectOptions::new().filename(path).read_only(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .map_err(map_sql)?;
        let result: String = sqlx::query_scalar("PRAGMA integrity_check")
            .fetch_one(&pool)
            .await
            .map_err(map_sql)?;
        pool.close().await;
        Ok(result)
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    async fn seed_prompts(&self) -> Result<(), PortError> {
        for asset in builtin_prompts() {
            sqlx::query("INSERT OR IGNORE INTO prompt_asset(id, version, family, template) VALUES (?, ?, ?, ?)")
                .bind(asset.id)
                .bind(asset.version)
                .bind(asset.family)
                .bind(asset.template)
                .execute(&self.pool)
                .await
                .map_err(map_sql)?;
        }
        Ok(())
    }

    pub async fn candidate_text(&self, id: &str) -> Result<Option<String>, PortError> {
        let row = sqlx::query("SELECT content FROM chapter_candidate WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_sql)?;
        row.map(|row| row.try_get("content").map_err(map_sql))
            .transpose()
    }

    pub async fn invocation_tokens(
        &self,
        request_id: &str,
    ) -> Result<Option<(Option<i64>, Option<i64>)>, PortError> {
        let row = sqlx::query(
            "SELECT prompt_tokens, completion_tokens FROM llm_invocation WHERE request_id = ?",
        )
        .bind(request_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sql)?;
        match row {
            None => Ok(None),
            Some(row) => Ok(Some((
                row.try_get("prompt_tokens").map_err(map_sql)?,
                row.try_get("completion_tokens").map_err(map_sql)?,
            ))),
        }
    }

    pub async fn latest_invocation_tokens(
        &self,
    ) -> Result<Option<(Option<i64>, Option<i64>)>, PortError> {
        let row = sqlx::query("SELECT prompt_tokens, completion_tokens FROM llm_invocation ORDER BY started_at DESC LIMIT 1")
            .fetch_optional(&self.pool)
            .await
            .map_err(map_sql)?;
        match row {
            None => Ok(None),
            Some(row) => Ok(Some((
                row.try_get("prompt_tokens").map_err(map_sql)?,
                row.try_get("completion_tokens").map_err(map_sql)?,
            ))),
        }
    }
}

pub fn map_sql(error: sqlx::Error) -> PortError {
    let text = error.to_string();
    if let Some(database) = error.as_database_error() {
        match database.code().as_deref() {
            // SQLITE_BUSY / SQLITE_LOCKED are normal contention, not corruption.
            Some("5") | Some("6") => {
                return PortError::Conflict("本地数据库正在写入，请稍后重试。".into());
            }
            // Keep corrupt/integrity failures distinguishable and non-retryable
            // by callers that inspect the storage error.
            Some("11") => {
                return PortError::Storage(format!("SQLite 完整性错误：{text}"));
            }
            _ => {}
        }
    }
    if text.contains("UNIQUE constraint failed") {
        if text.contains("workflow_control") || text.contains("one_active_run") {
            PortError::Conflict("这本书已经有一个进行中的创作任务。".into())
        } else {
            PortError::Conflict("这份内容刚被更新过，请重新打开后再试。".into())
        }
    } else {
        PortError::Storage(text)
    }
}

/// 迁移总数：测试用它校验「重复打开不会重复执行」，避免每加一条迁移就要改断言里的硬编码数字。
pub const MIGRATION_COUNT: usize = MIGRATION_SCRIPTS.len();

pub(crate) const MIGRATION_SCRIPTS: &[(&str, &str)] = &[
    ("0001", include_str!("../../../migrations/0001_init.sql")),
    (
        "0002",
        include_str!("../../../migrations/0002_model_setup.sql"),
    ),
    (
        "0003",
        include_str!("../../../migrations/0003_setup_fields.sql"),
    ),
    (
        "0004",
        include_str!("../../../migrations/0004_post_generation_style_review.sql"),
    ),
    (
        "0005",
        include_str!("../../../migrations/0005_chapter_local_draft.sql"),
    ),
    (
        "0006",
        include_str!("../../../migrations/0006_quality_debt.sql"),
    ),
    (
        "0007",
        include_str!("../../../migrations/0007_candidate_edit_draft.sql"),
    ),
    (
        "0008",
        include_str!("../../../migrations/0008_quality_debt_occurrence.sql"),
    ),
    (
        "0009",
        include_str!("../../../migrations/0009_knowledge_source_version.sql"),
    ),
    (
        "0010",
        include_str!("../../../migrations/0010_story_memory.sql"),
    ),
    (
        "0011",
        include_str!("../../../migrations/0011_story_fact_source.sql"),
    ),
    (
        "0012",
        include_str!("../../../migrations/0012_memory_extraction.sql"),
    ),
    (
        "0013",
        include_str!("../../../migrations/0013_memory_conflict.sql"),
    ),
    (
        "0014",
        include_str!("../../../migrations/0014_knowledge_index_job.sql"),
    ),
    (
        "0015",
        include_str!("../../../migrations/0015_conflict_request_hash.sql"),
    ),
    (
        "0016",
        include_str!("../../../migrations/0016_index_job_epoch.sql"),
    ),
    (
        "0017",
        include_str!("../../../migrations/0017_knowledge_revision_rank.sql"),
    ),
    (
        "0018",
        include_str!("../../../migrations/0018_chapter_revision_provenance.sql"),
    ),
    (
        "0019",
        include_str!("../../../migrations/0019_batch_run.sql"),
    ),
    (
        "0020",
        include_str!("../../../migrations/0020_structured_story_fact.sql"),
    ),
    (
        "0021",
        include_str!("../../../migrations/0021_structured_fact_version.sql"),
    ),
    (
        "0022",
        include_str!("../../../migrations/0022_provider_registry.sql"),
    ),
];

async fn migrate(pool: &SqlitePool) -> Result<(), PortError> {
    ensure_schema_migration_table(pool).await?;
    for (version, script) in MIGRATION_SCRIPTS {
        if migration_applied(pool, version).await? {
            continue;
        }
        let now_ms =
            sqlx::query_scalar::<_, i64>("SELECT CAST(strftime('%s','now')*1000 AS INTEGER)")
                .fetch_one(pool)
                .await
                .map_err(map_sql)?;
        let mut tx = pool.begin().await.map_err(map_sql)?;
        for statement in split_sql(script) {
            sqlx::query(&statement)
                .execute(tx.deref_mut())
                .await
                .map_err(map_sql)?;
        }
        sqlx::query("INSERT INTO schema_migration(version, applied_at) VALUES (?, ?)")
            .bind(version)
            .bind(now_ms)
            .execute(tx.deref_mut())
            .await
            .map_err(map_sql)?;
        tx.commit().await.map_err(map_sql)?;
    }
    Ok(())
}

async fn ensure_schema_migration_table(pool: &SqlitePool) -> Result<(), PortError> {
    let ready: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'schema_migration'",
    )
    .fetch_one(pool)
    .await
    .map_err(map_sql)?;
    if ready > 0 {
        return Ok(());
    }
    sqlx::query(
        "CREATE TABLE schema_migration (
            version TEXT PRIMARY KEY,
            applied_at INTEGER NOT NULL
        )",
    )
    .execute(pool)
    .await
    .map_err(map_sql)?;
    Ok(())
}

async fn migration_applied(pool: &SqlitePool, version: &str) -> Result<bool, PortError> {
    let ready: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'schema_migration'",
    )
    .fetch_one(pool)
    .await
    .map_err(map_sql)?;
    if ready == 0 {
        return Ok(false);
    }
    let applied: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM schema_migration WHERE version = ?")
            .bind(version)
            .fetch_one(pool)
            .await
            .map_err(map_sql)?;
    Ok(applied > 0)
}

fn split_sql(script: &str) -> Vec<String> {
    script
        .split(';')
        .map(str::trim)
        .filter(|statement| !statement.is_empty())
        .map(ToString::to_string)
        .collect()
}

pub async fn next_seq(tx: &mut Tx<'_>, run_id: &str) -> Result<i64, PortError> {
    sqlx::query_scalar("SELECT COALESCE(MAX(seq), 0) + 1 FROM workflow_event WHERE run_id = ?")
        .bind(run_id)
        .fetch_one(tx.deref_mut())
        .await
        .map_err(map_sql)
}

pub async fn insert_event(
    tx: &mut Tx<'_>,
    run_id: &str,
    seq: i64,
    kind: &str,
    step_id: Option<&str>,
    payload: &str,
    occurred_at: i64,
) -> Result<(), PortError> {
    sqlx::query("INSERT INTO workflow_event(run_id, seq, kind, step_id, payload_json, occurred_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(run_id)
        .bind(seq)
        .bind(kind)
        .bind(step_id)
        .bind(payload)
        .bind(occurred_at)
        .execute(tx.deref_mut())
        .await
        .map_err(map_sql)?;
    Ok(())
}
