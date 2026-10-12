use novel_application::PortError;
use serde::Serialize;
use sqlx::Row;

use crate::db::{map_sql, SqliteKernel};

pub const MODEL_SECRET_REF: &str = "openai-compatible";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredModelSetup {
    pub endpoint: String,
    pub model: String,
    pub allow_localhost_http: bool,
    pub secret_ref: String,
}

impl SqliteKernel {
    pub async fn save_model_setup(
        &self,
        endpoint: &str,
        model: &str,
        allow_localhost_http: bool,
        now_ms: i64,
    ) -> Result<(), PortError> {
        sqlx::query(
            "INSERT INTO provider_credential(id, provider, secret_ref, endpoint, model, allow_localhost_http, created_at)
             VALUES ('default', 'openai-compatible', ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
               endpoint = excluded.endpoint,
               model = excluded.model,
               allow_localhost_http = excluded.allow_localhost_http",
        )
        .bind(MODEL_SECRET_REF)
        .bind(endpoint)
        .bind(model)
        .bind(i64::from(u8::from(allow_localhost_http)))
        .bind(now_ms)
        .execute(&self.pool)
        .await
        .map_err(map_sql)?;
        Ok(())
    }

    pub async fn load_model_setup(&self) -> Result<Option<StoredModelSetup>, PortError> {
        let row = sqlx::query(
            "SELECT endpoint, model, allow_localhost_http, secret_ref FROM provider_credential WHERE id = 'default'",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sql)?;
        let Some(row) = row else {
            return Ok(None);
        };
        let allow: i64 = row.try_get("allow_localhost_http").map_err(map_sql)?;
        Ok(Some(StoredModelSetup {
            endpoint: row.try_get("endpoint").map_err(map_sql)?,
            model: row.try_get("model").map_err(map_sql)?,
            allow_localhost_http: allow != 0,
            secret_ref: row.try_get("secret_ref").map_err(map_sql)?,
        }))
    }
}
