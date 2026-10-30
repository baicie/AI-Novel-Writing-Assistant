//! P16.2B-2：Provider 相关 Tauri Command。
//!
//! 本层只做三件事：解码/校验 DTO、调用 `ProviderService`、序列化结果。
//! 不执行 SQL、不直接碰密钥库、不把谓词逻辑写在命令里。

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use novel_application::{
    CredentialAction, ProviderDraft, ProviderError, ProviderService, ProviderView,
};
use novel_domain::{ModelBinding, ModelProfile, ProviderProtocol};
use novel_llm::{BUILTIN_PROVIDERS, CatalogProvider, DiscoveryRequest};
use serde::{Deserialize, Serialize};
use tauri::State;

use novel_platform::SecretStore;

use crate::credential_vault::OsCredentialVault;
use crate::state::AppState;

/// 预览有效期：过期必须重新发现，避免用陈旧候选落库。
const PREVIEW_TTL_MS: i64 = 10 * 60 * 1000;

#[derive(Default)]
pub struct ProviderPreviews {
    entries: Mutex<HashMap<String, PreviewEntry>>,
    counter: AtomicU64,
}

struct PreviewEntry {
    provider_id: String,
    candidates: Vec<ModelProfile>,
    created_at_ms: i64,
}

impl ProviderPreviews {
    pub fn store(&self, provider_id: &str, candidates: Vec<ModelProfile>, now_ms: i64) -> String {
        let token = format!(
            "pv-{now_ms}-{}",
            self.counter.fetch_add(1, Ordering::Relaxed)
        );
        let mut entries = self.entries.lock().expect("preview lock");
        entries.retain(|_, entry| now_ms - entry.created_at_ms <= PREVIEW_TTL_MS);
        entries.insert(
            token.clone(),
            PreviewEntry {
                provider_id: provider_id.to_string(),
                candidates,
                created_at_ms: now_ms,
            },
        );
        token
    }

    pub fn take(
        &self,
        token: &str,
        provider_id: &str,
        now_ms: i64,
    ) -> Result<Vec<ModelProfile>, ProviderError> {
        let mut entries = self.entries.lock().expect("preview lock");
        let Some(entry) = entries.get(token) else {
            return Err(preview_expired());
        };
        if now_ms - entry.created_at_ms > PREVIEW_TTL_MS || entry.provider_id != provider_id {
            entries.remove(token);
            return Err(preview_expired());
        }
        let candidates = entry.candidates.clone();
        entries.remove(token);
        Ok(candidates)
    }
}

fn preview_expired() -> ProviderError {
    let mut error = ProviderError::validation("模型列表预览已过期，请重新获取");
    error.code = novel_application::ProviderErrorCode::NotFound;
    error
}

fn service(state: &AppState) -> ProviderService {
    ProviderService::new(
        state.kernel.clone(),
        std::sync::Arc::new(OsCredentialVault::new(
            state.secrets.clone() as std::sync::Arc<dyn novel_platform::SecretStore>
        )),
    )
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CredentialKindDto {
    Keep,
    Replace,
    Clear,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialActionDto {
    pub kind: CredentialKindDto,
    /// 只在 `replace` 时允许出现；其他情况携带视为参数错误。
    pub api_key: Option<String>,
}

impl CredentialActionDto {
    fn validate_and_convert(self) -> Result<CredentialAction, ProviderError> {
        match (self.kind, self.api_key) {
            (CredentialKindDto::Replace, Some(key)) if !key.trim().is_empty() => {
                Ok(CredentialAction::Replace(key.trim().to_string()))
            }
            (CredentialKindDto::Replace, _) => {
                Err(ProviderError::validation("替换密钥时必须提供 apiKey"))
            }
            (CredentialKindDto::Keep, None) => Ok(CredentialAction::Keep),
            (CredentialKindDto::Clear, None) => Ok(CredentialAction::Clear),
            (CredentialKindDto::Keep, Some(_)) | (CredentialKindDto::Clear, Some(_)) => Err(
                ProviderError::validation("keep / clear 不能同时携带 apiKey"),
            ),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertProviderRequest {
    pub provider_id: String,
    pub display_name: Option<String>,
    /// openai-compatible / anthropic-messages / ollama
    pub protocol: String,
    pub base_url: String,
    pub allow_localhost_http: Option<bool>,
    pub sort_order: Option<i32>,
    pub is_builtin: Option<bool>,
    pub credential: CredentialActionDto,
    /// 新建时省略；更新时必填（CAS）。
    pub expected_revision: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertModelRequest {
    pub provider_id: String,
    pub model_id: String,
    pub display_name: Option<String>,
    pub context_window: Option<u32>,
    pub max_output_tokens: Option<u32>,
    pub supports_streaming: Option<bool>,
    pub supports_tools: Option<bool>,
    pub supports_reasoning: Option<bool>,
    pub expected_revision: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryDraftRequest {
    /// 允许尚未保存的草稿：没有 provider_id 也能取模型。
    pub provider_id: Option<String>,
    pub protocol: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub allow_localhost_http: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryPreview {
    pub preview_token: String,
    pub provider_id: String,
    pub models: Vec<ModelProfile>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyDiscoveryRequest {
    pub provider_id: String,
    pub preview_token: String,
    pub selected_model_ids: Vec<String>,
    pub expected_revision: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertBindingRequest {
    pub task_type: String,
    pub provider_id: String,
    pub model_id: String,
    pub reasoning_effort: Option<String>,
    pub expected_revision: Option<i64>,
}

fn protocol_from(value: &str) -> Result<ProviderProtocol, ProviderError> {
    ProviderProtocol::parse(value)
        .ok_or_else(|| ProviderError::validation(format!("不支持的协议：{value}")))
}

#[tauri::command]
pub fn list_provider_templates() -> Vec<&'static CatalogProvider> {
    BUILTIN_PROVIDERS.iter().collect()
}

#[tauri::command]
pub async fn list_provider_profiles(
    state: State<'_, AppState>,
) -> Result<Vec<ProviderView>, ProviderError> {
    service(&state).list_providers().await
}

#[tauri::command]
pub async fn get_provider_profile(
    state: State<'_, AppState>,
    provider_id: String,
) -> Result<ProviderView, ProviderError> {
    service(&state).get_provider(&provider_id).await
}

#[tauri::command]
pub async fn upsert_provider_profile(
    state: State<'_, AppState>,
    request: UpsertProviderRequest,
) -> Result<ProviderView, ProviderError> {
    let credential = request.credential.validate_and_convert()?;
    let draft = ProviderDraft {
        id: request.provider_id,
        display_name: request.display_name,
        protocol: protocol_from(&request.protocol)?,
        base_url: request.base_url,
        allow_localhost_http: request.allow_localhost_http.unwrap_or(false),
        sort_order: request.sort_order.unwrap_or(0),
        is_builtin: request.is_builtin.unwrap_or(false),
    };
    service(&state)
        .upsert_provider(
            draft,
            credential,
            request.expected_revision,
            novel_platform::now_ms(),
        )
        .await
}

#[tauri::command]
pub async fn delete_provider_profile(
    state: State<'_, AppState>,
    provider_id: String,
    force: Option<bool>,
) -> Result<(), ProviderError> {
    service(&state)
        .delete_provider(&provider_id, force.unwrap_or(false))
        .await
}

#[tauri::command]
pub async fn list_provider_models(
    state: State<'_, AppState>,
    provider_id: String,
) -> Result<Vec<ModelProfile>, ProviderError> {
    service(&state).list_models(&provider_id).await
}

#[tauri::command]
pub async fn upsert_provider_model(
    state: State<'_, AppState>,
    request: UpsertModelRequest,
) -> Result<(), ProviderError> {
    let model = ModelProfile {
        provider_id: request.provider_id,
        model_id: request.model_id,
        display_name: request.display_name,
        capabilities: novel_domain::ModelCapabilities {
            context_window: request.context_window,
            max_output_tokens: request.max_output_tokens,
            supports_streaming: request.supports_streaming,
            supports_tools: request.supports_tools,
            supports_reasoning: request.supports_reasoning,
            input_modalities: None,
            reasoning_efforts: None,
        },
        source: novel_domain::ModelSource::User,
        overridden: [
            novel_domain::CapabilityField::ContextWindow,
            novel_domain::CapabilityField::MaxOutputTokens,
            novel_domain::CapabilityField::SupportsStreaming,
            novel_domain::CapabilityField::SupportsTools,
            novel_domain::CapabilityField::SupportsReasoning,
        ]
        .into_iter()
        .collect(),
    };
    service(&state)
        .upsert_model(
            model,
            true,
            request.expected_revision,
            novel_platform::now_ms(),
        )
        .await
}

#[tauri::command]
pub async fn delete_provider_model(
    state: State<'_, AppState>,
    provider_id: String,
    model_id: String,
    expected_revision: Option<i64>,
) -> Result<(), ProviderError> {
    service(&state)
        .delete_model(&provider_id, &model_id, expected_revision)
        .await
}

/// 发现只返回候选，不落库；草稿（未保存的 provider）同样可用。
#[tauri::command]
pub async fn discover_provider_models(
    state: State<'_, AppState>,
    request: DiscoveryDraftRequest,
) -> Result<DiscoveryPreview, ProviderError> {
    let adapter = novel_llm::discovery_for(&request.protocol).ok_or_else(|| {
        ProviderError::validation(format!("该协议暂不支持获取模型列表：{}", request.protocol))
    })?;
    let api_key = match request.api_key {
        Some(key) if !key.trim().is_empty() => key,
        _ => match request.provider_id.as_deref() {
            // 草稿没有密钥就发不带鉴权的请求；已保存的 provider 用系统凭据。
            Some(provider_id) => state
                .secrets
                .get(provider_id)
                .await
                .ok()
                .flatten()
                .unwrap_or_default(),
            None => String::new(),
        },
    };
    let discovery_request = DiscoveryRequest::new(
        request.base_url.clone(),
        api_key,
        request.allow_localhost_http.unwrap_or(false),
    );
    let outcome = adapter
        .discover(
            &discovery_request,
            tokio_util::sync::CancellationToken::new(),
        )
        .await
        .map_err(discovery_error)?;
    let provider_id = request.provider_id.unwrap_or_else(|| "draft".to_string());
    let candidates: Vec<ModelProfile> = outcome
        .models
        .into_iter()
        .map(|model| ModelProfile {
            provider_id: provider_id.clone(),
            model_id: model.id,
            display_name: model.display_name,
            capabilities: novel_domain::ModelCapabilities {
                context_window: model.context_window,
                max_output_tokens: model.max_output_tokens,
                supports_streaming: None,
                supports_tools: None,
                supports_reasoning: None,
                input_modalities: model.input_modalities.map(|items| {
                    items
                        .into_iter()
                        .filter_map(|item| match item.as_str() {
                            "text" => Some(novel_domain::Modality::Text),
                            "image" => Some(novel_domain::Modality::Image),
                            "audio" => Some(novel_domain::Modality::Audio),
                            "video" => Some(novel_domain::Modality::Video),
                            _ => None,
                        })
                        .collect()
                }),
                reasoning_efforts: None,
            },
            source: novel_domain::ModelSource::Discovered,
            overridden: Default::default(),
        })
        .collect();
    let token =
        state
            .provider_previews
            .store(&provider_id, candidates.clone(), novel_platform::now_ms());
    Ok(DiscoveryPreview {
        preview_token: token,
        provider_id,
        models: candidates,
        warnings: outcome.warnings,
    })
}

fn discovery_error(error: novel_llm::DiscoveryError) -> ProviderError {
    let message = error.to_string();
    ProviderError::validation(format!("{}（{}）", message, error.code()))
}

#[tauri::command]
pub async fn apply_provider_discovery(
    state: State<'_, AppState>,
    request: ApplyDiscoveryRequest,
) -> Result<novel_storage_sqlite::ApplyDiscoveredOutcome, ProviderError> {
    let candidates = state.provider_previews.take(
        &request.preview_token,
        &request.provider_id,
        novel_platform::now_ms(),
    )?;
    Ok(state
        .kernel
        .apply_discovered_models(
            &request.provider_id,
            &candidates,
            &request.selected_model_ids,
            request.expected_revision,
            novel_platform::now_ms(),
        )
        .await?)
}

#[tauri::command]
pub async fn list_model_bindings(
    state: State<'_, AppState>,
) -> Result<Vec<ModelBinding>, ProviderError> {
    service(&state).list_bindings().await
}

#[tauri::command]
pub async fn upsert_model_binding(
    state: State<'_, AppState>,
    request: UpsertBindingRequest,
) -> Result<(), ProviderError> {
    service(&state)
        .upsert_binding(
            ModelBinding {
                task_type: request.task_type,
                provider_id: request.provider_id,
                model_id: request.model_id,
                reasoning_effort: request.reasoning_effort,
            },
            request.expected_revision,
            novel_platform::now_ms(),
        )
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_replace_requires_key() {
        let missing = CredentialActionDto {
            kind: CredentialKindDto::Replace,
            api_key: None,
        };
        assert!(missing.validate_and_convert().is_err());
        let blank = CredentialActionDto {
            kind: CredentialKindDto::Replace,
            api_key: Some("   ".into()),
        };
        assert!(blank.validate_and_convert().is_err());
    }

    #[test]
    fn credential_keep_and_clear_reject_key() {
        for kind in [CredentialKindDto::Keep, CredentialKindDto::Clear] {
            let with_key = CredentialActionDto {
                kind,
                api_key: Some("sk-x".into()),
            };
            assert!(
                with_key.validate_and_convert().is_err(),
                "{kind:?} 不能携带 apiKey"
            );
            let clean = CredentialActionDto {
                kind,
                api_key: None,
            };
            assert!(clean.validate_and_convert().is_ok());
        }
    }

    #[test]
    fn credential_replace_trims_key() {
        let dto = CredentialActionDto {
            kind: CredentialKindDto::Replace,
            api_key: Some("  sk-abc  ".into()),
        };
        assert_eq!(
            dto.validate_and_convert().unwrap(),
            CredentialAction::Replace("sk-abc".into())
        );
    }

    #[test]
    fn dto_deserializes_camel_case() {
        let dto: CredentialActionDto =
            serde_json::from_str(r#"{"kind":"replace","apiKey":"sk-1"}"#).unwrap();
        assert_eq!(dto.kind, CredentialKindDto::Replace);
        assert_eq!(dto.api_key.as_deref(), Some("sk-1"));
    }

    #[test]
    fn preview_expires_after_ttl_and_is_single_use() {
        let previews = ProviderPreviews::default();
        let candidates = vec![ModelProfile {
            provider_id: "gw".into(),
            model_id: "a".into(),
            display_name: None,
            capabilities: Default::default(),
            source: novel_domain::ModelSource::Discovered,
            overridden: Default::default(),
        }];
        let token = previews.store("gw", candidates, 1_000);
        assert!(
            previews
                .take(&token, "gw", 1_000 + PREVIEW_TTL_MS + 1)
                .is_err(),
            "过期必须报错"
        );
        let token = previews.store("gw", Vec::new(), 2_000);
        assert!(previews.take(&token, "gw", 2_000).is_ok());
        assert!(previews.take(&token, "gw", 2_000).is_err(), "一次性 token");
    }
}
