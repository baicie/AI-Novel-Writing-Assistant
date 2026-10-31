//! 凭据变更意图的持久化：跨 SQLite 与密钥库的补偿日志。

use novel_application::PortError;
use sqlx::Row;

use crate::db::{SqliteKernel, map_sql};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredCredentialIntent {
    pub id: String,
    pub provider_id: String,
    pub secret_ref: String,
    /// `replace` 或 `clear`。replace 的密钥正文从不落库。
    pub action: String,
    pub state: String,
    pub attempts: i64,
    pub last_error: Option<String>,
}

impl SqliteKernel {
    pub async fn insert_credential_intent(
        &self,
        id: &str,
        provider_id: &str,
        secret_ref: &str,
        action: &str,
        now_ms: i64,
    ) -> Result<(), PortError> {
        sqlx::query(
            "INSERT INTO credential_intent(id, provider_id, secret_ref, action, state, attempts, created_at, updated_at)
             VALUES (?, ?, ?, ?, 'pending', 0, ?, ?)",
        )
        .bind(id)
        .bind(provider_id)
        .bind(secret_ref)
        .bind(action)
        .bind(now_ms)
        .bind(now_ms)
        .execute(&self.pool)
        .await
        .map_err(map_sql)?;
        Ok(())
    }

    pub async fn finish_credential_intent(
        &self,
        id: &str,
        state: &str,
        last_error: Option<&str>,
        now_ms: i64,
    ) -> Result<(), PortError> {
        sqlx::query(
            "UPDATE credential_intent
             SET state = ?, attempts = attempts + 1, last_error = ?, updated_at = ?
             WHERE id = ?",
        )
        .bind(state)
        .bind(last_error)
        .bind(now_ms)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_sql)?;
        Ok(())
    }

    pub async fn pending_credential_intents(
        &self,
    ) -> Result<Vec<StoredCredentialIntent>, PortError> {
        let rows = sqlx::query(
            "SELECT id, provider_id, secret_ref, action, state, attempts, last_error
             FROM credential_intent WHERE state = 'pending' ORDER BY created_at",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sql)?;
        let mut intents = Vec::with_capacity(rows.len());
        for row in rows {
            intents.push(StoredCredentialIntent {
                id: row.try_get("id").map_err(map_sql)?,
                provider_id: row.try_get("provider_id").map_err(map_sql)?,
                secret_ref: row.try_get("secret_ref").map_err(map_sql)?,
                action: row.try_get("action").map_err(map_sql)?,
                state: row.try_get("state").map_err(map_sql)?,
                attempts: row.try_get("attempts").map_err(map_sql)?,
                last_error: row.try_get("last_error").map_err(map_sql)?,
            });
        }
        Ok(intents)
    }
}
