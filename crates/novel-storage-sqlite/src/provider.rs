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

    /// 内部级联删除：业务删除请走 `delete_provider_guarded`。
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
/// 发现结果的确认结果：只统计与候选相关的三种情况，从不删除未出现的模型。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyDiscoveredOutcome {
    pub added: usize,
    pub updated: usize,
    /// 因为用户覆盖而被保留原值的字段数（模型数）。
    pub preserved: usize,
    pub revision: i64,
}

impl SqliteKernel {
    pub async fn provider_revision(&self, provider_id: &str) -> Result<Option<i64>, PortError> {
        let row = sqlx::query("SELECT revision FROM provider_credential WHERE id = ?")
            .bind(provider_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_sql)?;
        row.map(|row| row.try_get::<i64, _>("revision").map_err(map_sql))
            .transpose()
    }

    /// 确认发现结果：**只做追加与合并**，永不删除、永不清空、永不覆盖用户改过的字段。
    ///
    /// - `selected` 为空 → 直接返回（空列表不能当作「清空模型」）；
    /// - `expected_revision` 与库里不一致 → `PortError::Conflict`（stale discovery 不落库）；
    /// - 已在库里但这次没被选/没被返回的模型保持原样；
    /// - 全程一个事务，失败即回滚。
    pub async fn apply_discovered_models(
        &self,
        provider_id: &str,
        candidates: &[ModelProfile],
        selected: &[String],
        expected_revision: Option<i64>,
        now_ms: i64,
    ) -> Result<ApplyDiscoveredOutcome, PortError> {
        let current = self
            .provider_revision(provider_id)
            .await?
            .ok_or_else(|| PortError::NotFound(format!("模型来源不存在：{provider_id}")))?;
        if expected_revision.is_some_and(|expected| expected != current) {
            return Err(PortError::Conflict(format!(
                "配置已被修改（期望版本 {}，当前 {current}），请重新获取模型列表",
                expected_revision.unwrap_or_default()
            )));
        }
        if selected.is_empty() {
            return Ok(ApplyDiscoveredOutcome {
                added: 0,
                updated: 0,
                preserved: 0,
                revision: current,
            });
        }

        let existing: std::collections::HashMap<String, ModelProfile> = self
            .list_models(provider_id)
            .await?
            .into_iter()
            .map(|model| (model.model_id.clone(), model))
            .collect();

        let mut added = 0usize;
        let mut updated = 0usize;
        let mut preserved = 0usize;
        let mut tx = self.pool.begin().await.map_err(map_sql)?;
        for model_id in selected {
            let Some(candidate) = candidates.iter().find(|item| &item.model_id == model_id) else {
                // 候选已经过期（stale discovery），跳过而不是凭空造一条模型
                continue;
            };
            let merged = match existing.get(model_id) {
                Some(current_model) => {
                    let merged = current_model.apply_discovered(candidate);
                    if !current_model.overridden.is_empty() {
                        preserved += 1;
                    }
                    merged
                }
                None => candidate.clone(),
            };
            write_model_row(&mut tx, &merged, existing.contains_key(model_id), now_ms).await?;
            if existing.contains_key(model_id) {
                updated += 1;
            } else {
                added += 1;
            }
        }
        let next_revision = current + 1;
        sqlx::query("UPDATE provider_credential SET revision = ?, updated_at = ? WHERE id = ?")
            .bind(next_revision)
            .bind(now_ms)
            .bind(provider_id)
            .execute(&mut *tx)
            .await
            .map_err(map_sql)?;
        tx.commit().await.map_err(map_sql)?;
        Ok(ApplyDiscoveredOutcome {
            added,
            updated,
            preserved,
            revision: next_revision,
        })
    }

    /// 删除 provider：默认拒绝「仍被绑定引用」或「仍是默认模型配置」的删除。
    pub async fn delete_provider_guarded(
        &self,
        provider_id: &str,
        force: bool,
    ) -> Result<(), PortError> {
        let bindings: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM model_binding WHERE provider_id = ?")
                .bind(provider_id)
                .fetch_one(&self.pool)
                .await
                .map_err(map_sql)?;
        if bindings > 0 && !force {
            return Err(PortError::Conflict(format!(
                "还有 {bindings} 条任务绑定在使用这个模型来源，先改绑定再删除"
            )));
        }
        if provider_id == DEFAULT_PROVIDER_ID && !force {
            return Err(PortError::Conflict(
                "这是桌面版当前使用的默认模型配置，删除后创作会失去模型来源".into(),
            ));
        }
        self.delete_provider(provider_id).await
    }
}

async fn write_model_row<'a>(
    tx: &mut sqlx::Transaction<'a, sqlx::Sqlite>,
    profile: &ModelProfile,
    exists: bool,
    now_ms: i64,
) -> Result<(), PortError> {
    let overrides = if profile.overridden.is_empty() {
        None
    } else {
        let fields: Vec<CapabilityField> = profile.overridden.iter().copied().collect();
        Some(
            serde_json::to_string(&fields)
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
             reasoning_efforts, source, user_overrides, created_at, updated_at, revision)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1)
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
           updated_at = excluded.updated_at,
           revision = provider_model.revision + 1",
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
    .bind(overrides)
    .bind(now_ms)
    .bind(now_ms)
    .execute(&mut **tx)
    .await
    .map_err(map_sql)?;
    let _ = exists;
    Ok(())
}
/// CAS 版 Provider 写入：新建必须不存在，更新必须版本一致。
impl SqliteKernel {
    pub async fn upsert_provider_cas(
        &self,
        profile: &ProviderProfile,
        expected_revision: Option<i64>,
        now_ms: i64,
    ) -> Result<i64, PortError> {
        // 单条条件更新完成 CAS：读后再写会让两个窗口同时成功。
        match expected_revision {
            Some(expected) => {
                let affected = sqlx::query(
                    "UPDATE provider_credential SET
                       provider = ?, secret_ref = ?, endpoint = ?, allow_localhost_http = ?,
                       protocol = ?, display_name = ?, is_builtin = ?, sort_order = ?,
                       updated_at = ?, revision = revision + 1
                     WHERE id = ? AND revision = ?",
                )
                .bind(profile.protocol.as_str())
                .bind(&profile.credential.reference)
                .bind(&profile.base_url)
                .bind(i64::from(u8::from(profile.allow_localhost_http)))
                .bind(profile.protocol.as_str())
                .bind(&profile.display_name)
                .bind(i64::from(u8::from(profile.is_builtin)))
                .bind(i64::from(profile.sort_order))
                .bind(now_ms)
                .bind(&profile.id)
                .bind(expected)
                .execute(&self.pool)
                .await
                .map_err(map_sql)?
                .rows_affected();
                if affected == 0 {
                    let current = self.provider_revision(&profile.id).await?;
                    return Err(match current {
                        Some(actual) => PortError::Conflict(format!(
                            "配置已被修改（期望版本 {expected}，当前 {actual}），请刷新后再试"
                        )),
                        None => PortError::NotFound(format!("模型来源不存在：{}", profile.id)),
                    });
                }
                Ok(expected + 1)
            }
            None => {
                let inserted = sqlx::query(
                    "INSERT INTO provider_credential(
                         id, provider, secret_ref, endpoint, model, allow_localhost_http, created_at,
                         protocol, display_name, is_builtin, sort_order, updated_at, revision)
                     VALUES (?, ?, ?, ?, '', ?, ?, ?, ?, ?, ?, ?, 1)
                     ON CONFLICT(id) DO NOTHING",
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
                .map_err(map_sql)?
                .rows_affected();
                if inserted == 0 {
                    return Err(PortError::Conflict(format!(
                        "模型来源已存在，改用更新（需要提供 expectedRevision）：{}",
                        profile.id
                    )));
                }
                Ok(1)
            }
        }
    }

    pub async fn binding_count(&self, provider_id: &str) -> Result<i64, PortError> {
        sqlx::query_scalar("SELECT COUNT(*) FROM model_binding WHERE provider_id = ?")
            .bind(provider_id)
            .fetch_one(&self.pool)
            .await
            .map_err(map_sql)
    }

    pub async fn delete_model_cas(
        &self,
        provider_id: &str,
        model_id: &str,
        expected_revision: Option<i64>,
    ) -> Result<(), PortError> {
        if let Some(expected) = expected_revision {
            let current = self
                .provider_revision(provider_id)
                .await?
                .ok_or_else(|| PortError::NotFound(format!("模型来源不存在：{provider_id}")))?;
            if current != expected {
                return Err(PortError::Conflict(format!(
                    "配置已被修改（期望版本 {expected}，当前 {current}），请刷新后再试"
                )));
            }
        }
        let affected =
            sqlx::query("DELETE FROM provider_model WHERE provider_id = ? AND model_id = ?")
                .bind(provider_id)
                .bind(model_id)
                .execute(&self.pool)
                .await
                .map_err(map_sql)?
                .rows_affected();
        if affected == 0 {
            return Err(PortError::NotFound(format!("模型不存在：{model_id}")));
        }
        Ok(())
    }

    pub async fn set_binding_cas(
        &self,
        binding: &ModelBinding,
        expected_revision: Option<i64>,
        now_ms: i64,
    ) -> Result<(), PortError> {
        if let Some(expected) = expected_revision {
            let current = self
                .provider_revision(&binding.provider_id)
                .await?
                .ok_or_else(|| {
                    PortError::NotFound(format!("模型来源不存在：{}", binding.provider_id))
                })?;
            if current != expected {
                return Err(PortError::Conflict(format!(
                    "配置已被修改（期望版本 {expected}，当前 {current}），请刷新后再试"
                )));
            }
        }
        self.set_model_binding(binding, now_ms).await
    }
}
