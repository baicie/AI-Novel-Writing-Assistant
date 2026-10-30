//! Provider / 模型注册表的存储层（迁移 0021）。
//!
//! 权威存储仍然是 `provider_credential`：老的 `id='default'` 行继续可读可写，
//! 新字段与新表都是增量，老版本应用忽略它们。

use novel_application::PortError;
use novel_domain::{
    CapabilityField, CredentialKind, CredentialReference, Modality, ModelBinding,
    ModelCapabilities, ModelProfile, ModelSource, ProviderProfile, ProviderProtocol,
};
use sqlx::Row;

use crate::db::{SqliteKernel, map_sql};

const DEFAULT_PROVIDER_ID: &str = "default";

impl SqliteKernel {
    pub async fn upsert_provider(
        &self,
        profile: &ProviderProfile,
        now_ms: i64,
    ) -> Result<(), PortError> {
        sqlx::query(
            "INSERT INTO provider_credential(
                 id, provider, secret_ref, endpoint, model, allow_localhost_http, created_at,
                 protocol, display_name, is_builtin, sort_order, updated_at)
             VALUES (?, ?, ?, ?, '', ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
               provider = excluded.provider,
               secret_ref = excluded.secret_ref,
               endpoint = excluded.endpoint,
               allow_localhost_http = excluded.allow_localhost_http,
               protocol = excluded.protocol,
               display_name = excluded.display_name,
               is_builtin = excluded.is_builtin,
               sort_order = excluded.sort_order,
               updated_at = excluded.updated_at",
        )
        .bind(&profile.id)
        .bind(profile.protocol.as_str())
        .bind(&profile.credential.reference)
        .bind(&profile.base_url)
        .bind(i64::from(u8::from(profile.allow_localhost_http)))
        .bind(now_ms)
        .bind(profile.protocol.as_str())
        .bind(&profile.display_name)
        .bind(i64::from(u8::from(profile.is_builtin)))
        .bind(i64::from(profile.sort_order))
        .bind(now_ms)
        .execute(&self.pool)
        .await
        .map_err(map_sql)?;
        Ok(())
    }

    pub async fn list_providers(&self) -> Result<Vec<ProviderProfile>, PortError> {
        let rows = sqlx::query(
            "SELECT id, provider, secret_ref, endpoint, allow_localhost_http, protocol, display_name,
                    is_builtin, sort_order
             FROM provider_credential
             ORDER BY sort_order, id",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sql)?;
        let mut providers = Vec::with_capacity(rows.len());
        for row in rows {
            let protocol: String = row.try_get("protocol").map_err(map_sql)?;
            let allow: i64 = row.try_get("allow_localhost_http").map_err(map_sql)?;
            let builtin: i64 = row.try_get("is_builtin").map_err(map_sql)?;
            providers.push(ProviderProfile {
                id: row.try_get("id").map_err(map_sql)?,
                display_name: row.try_get("display_name").map_err(map_sql)?,
                protocol: ProviderProtocol::parse(&protocol)
                    .unwrap_or(ProviderProtocol::OpenAiCompatible),
                base_url: row.try_get("endpoint").map_err(map_sql)?,
                credential: CredentialReference {
                    kind: CredentialKind::OsSecret,
                    reference: row.try_get("secret_ref").map_err(map_sql)?,
                },
                is_builtin: builtin != 0,
                allow_localhost_http: allow != 0,
                sort_order: row.try_get::<i64, _>("sort_order").map_err(map_sql)? as i32,
            });
        }
        Ok(providers)
    }

    pub async fn delete_provider(&self, provider_id: &str) -> Result<(), PortError> {
        let mut tx = self.pool.begin().await.map_err(map_sql)?;
        for statement in [
            "DELETE FROM provider_model WHERE provider_id = ?",
            "DELETE FROM model_binding WHERE provider_id = ?",
            "DELETE FROM provider_credential WHERE id = ?",
        ] {
            sqlx::query(statement)
                .bind(provider_id)
                .execute(&mut *tx)
                .await
                .map_err(map_sql)?;
        }
        tx.commit().await.map_err(map_sql)?;
        Ok(())
    }

    /// 写入一个模型。`user_edit` 为真时把「本次显式给出的字段」记为用户覆盖，
    /// 之后远端发现结果不会覆盖这些字段。
    pub async fn upsert_model(
        &self,
        profile: &ModelProfile,
        user_edit: bool,
        now_ms: i64,
    ) -> Result<(), PortError> {
        let overrides: Vec<CapabilityField> = if user_edit {
            profile.overridden.iter().copied().collect()
        } else {
            Vec::new()
        };
        let overrides_json = if overrides.is_empty() {
            None
        } else {
            Some(
                serde_json::to_string(&overrides)
                    .map_err(|error| PortError::Storage(format!("覆盖字段序列化失败：{error}")))?,
            )
        };
        let modalities = profile
            .capabilities
            .input_modalities
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| PortError::Storage(format!("模态序列化失败：{error}")))?;
        let efforts = profile
            .capabilities
            .reasoning_efforts
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| PortError::Storage(format!("推理档位序列化失败：{error}")))?;
        sqlx::query(
            "INSERT INTO provider_model(
                 provider_id, model_id, display_name, context_window, max_output_tokens,
                 supports_streaming, supports_tools, supports_reasoning, input_modalities,
                 reasoning_efforts, source, user_overrides, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(provider_id, model_id) DO UPDATE SET
               display_name = excluded.display_name,
               context_window = excluded.context_window,
               max_output_tokens = excluded.max_output_tokens,
               supports_streaming = excluded.supports_streaming,
               supports_tools = excluded.supports_tools,
               supports_reasoning = excluded.supports_reasoning,
               input_modalities = excluded.input_modalities,
               reasoning_efforts = excluded.reasoning_efforts,
               source = excluded.source,
               user_overrides = excluded.user_overrides,
               updated_at = excluded.updated_at",
        )
        .bind(&profile.provider_id)
        .bind(&profile.model_id)
        .bind(&profile.display_name)
        .bind(profile.capabilities.context_window.map(i64::from))
        .bind(profile.capabilities.max_output_tokens.map(i64::from))
        .bind(
            profile
                .capabilities
                .supports_streaming
                .map(|value| i64::from(u8::from(value))),
        )
        .bind(
            profile
                .capabilities
                .supports_tools
                .map(|value| i64::from(u8::from(value))),
        )
        .bind(
            profile
                .capabilities
                .supports_reasoning
                .map(|value| i64::from(u8::from(value))),
        )
        .bind(modalities)
        .bind(efforts)
        .bind(source_str(profile.source))
        .bind(overrides_json)
        .bind(now_ms)
        .bind(now_ms)
        .execute(&self.pool)
        .await
        .map_err(map_sql)?;
        Ok(())
    }

    pub async fn list_models(&self, provider_id: &str) -> Result<Vec<ModelProfile>, PortError> {
        let rows = sqlx::query(
            "SELECT model_id, display_name, context_window, max_output_tokens, supports_streaming,
                    supports_tools, supports_reasoning, input_modalities, reasoning_efforts,
                    source, user_overrides
             FROM provider_model WHERE provider_id = ? ORDER BY model_id",
        )
        .bind(provider_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sql)?;
        let mut models = Vec::with_capacity(rows.len());
        for row in rows {
            let source: String = row.try_get("source").map_err(map_sql)?;
            let overrides_raw: Option<String> = row.try_get("user_overrides").map_err(map_sql)?;
            let overridden = overrides_raw
                .as_deref()
                .map(|raw| serde_json::from_str::<Vec<CapabilityField>>(raw).unwrap_or_default())
                .unwrap_or_default()
                .into_iter()
                .collect();
            models.push(ModelProfile {
                provider_id: provider_id.to_string(),
                model_id: row.try_get("model_id").map_err(map_sql)?,
                display_name: row.try_get("display_name").map_err(map_sql)?,
                capabilities: ModelCapabilities {
                    context_window: row
                        .try_get::<Option<i64>, _>("context_window")
                        .map_err(map_sql)?
                        .map(|value| value as u32),
                    max_output_tokens: row
                        .try_get::<Option<i64>, _>("max_output_tokens")
                        .map_err(map_sql)?
                        .map(|value| value as u32),
                    supports_streaming: flag(&row, "supports_streaming")?,
                    supports_tools: flag(&row, "supports_tools")?,
                    supports_reasoning: flag(&row, "supports_reasoning")?,
                    input_modalities: json_column::<Vec<Modality>>(&row, "input_modalities")?,
                    reasoning_efforts: json_column(&row, "reasoning_efforts")?,
                },
                source: parse_source(&source),
                overridden,
            });
        }
        Ok(models)
    }

    pub async fn set_model_binding(
        &self,
        binding: &ModelBinding,
        now_ms: i64,
    ) -> Result<(), PortError> {
        sqlx::query(
            "INSERT INTO model_binding(task_type, provider_id, model_id, reasoning_effort, updated_at)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(task_type) DO UPDATE SET
               provider_id = excluded.provider_id,
               model_id = excluded.model_id,
               reasoning_effort = excluded.reasoning_effort,
               updated_at = excluded.updated_at",
        )
        .bind(&binding.task_type)
        .bind(&binding.provider_id)
        .bind(&binding.model_id)
        .bind(&binding.reasoning_effort)
        .bind(now_ms)
        .execute(&self.pool)
        .await
        .map_err(map_sql)?;
        Ok(())
    }

    pub async fn list_model_bindings(&self) -> Result<Vec<ModelBinding>, PortError> {
        let rows = sqlx::query(
            "SELECT task_type, provider_id, model_id, reasoning_effort FROM model_binding ORDER BY task_type",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sql)?;
        let mut bindings = Vec::with_capacity(rows.len());
        for row in rows {
            bindings.push(ModelBinding {
                task_type: row.try_get("task_type").map_err(map_sql)?,
                provider_id: row.try_get("provider_id").map_err(map_sql)?,
                model_id: row.try_get("model_id").map_err(map_sql)?,
                reasoning_effort: row.try_get("reasoning_effort").map_err(map_sql)?,
            });
        }
        Ok(bindings)
    }

    /// 老接口：仍然读写 `id='default'` 那一行，供现有 UI 与首次上手流程使用。
    pub async fn default_provider_id(&self) -> &'static str {
        DEFAULT_PROVIDER_ID
    }
}

fn source_str(source: ModelSource) -> &'static str {
    match source {
        ModelSource::Builtin => "builtin",
        ModelSource::Discovered => "discovered",
        ModelSource::Supplemental => "supplemental",
        ModelSource::User => "user",
    }
}

fn parse_source(value: &str) -> ModelSource {
    match value {
        "builtin" => ModelSource::Builtin,
        "discovered" => ModelSource::Discovered,
        "supplemental" => ModelSource::Supplemental,
        _ => ModelSource::User,
    }
}

fn flag(row: &sqlx::sqlite::SqliteRow, column: &str) -> Result<Option<bool>, PortError> {
    let raw: Option<i64> = row.try_get(column).map_err(map_sql)?;
    Ok(raw.map(|value| value != 0))
}

fn json_column<T: serde::de::DeserializeOwned>(
    row: &sqlx::sqlite::SqliteRow,
    column: &str,
) -> Result<Option<T>, PortError> {
    let raw: Option<String> = row.try_get(column).map_err(map_sql)?;
    match raw {
        Some(text) => serde_json::from_str(&text)
            .map(Some)
            .map_err(|error| PortError::Storage(format!("{column} 反序列化失败：{error}"))),
        None => Ok(None),
    }
}
