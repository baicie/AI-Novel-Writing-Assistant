use std::sync::Arc;

use async_trait::async_trait;
use novel_application::{AppError, ModelSecrets, PortError};
use novel_platform::OsSecretStore;
use novel_storage_sqlite::SqliteKernel;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::state::AppState;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveModelSetupRequest {
    pub endpoint: String,
    pub model: String,
    pub allow_localhost_http: bool,
    pub api_key: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelSetupView {
    pub endpoint: String,
    pub model: String,
    pub allow_localhost_http: bool,
    pub has_api_key: bool,
}

pub struct OsModelSecrets {
    pub store: Arc<OsSecretStore>,
}

#[async_trait]
impl ModelSecrets for OsModelSecrets {
    async fn api_key(&self, provider: &str) -> Result<Option<String>, PortError> {
        use novel_platform::SecretStore;
        let key = self
            .store
            .get(provider)
            .await
            .map_err(|_| PortError::Storage("系统凭据暂时不可用".into()))?;
        if key.is_some() {
            return Ok(key);
        }
        // The desktop currently stores one OpenAI-compatible credential. Runs may carry a
        // different provider label, but they still refer to that same local credential slot.
        self.store
            .get(novel_storage_sqlite::MODEL_SECRET_REF)
            .await
            .map_err(|_| PortError::Storage("系统凭据暂时不可用".into()))
    }
}

#[tauri::command]
pub async fn get_model_setup(
    state: State<'_, AppState>,
) -> Result<Option<ModelSetupView>, AppError> {
    let kernel: Arc<SqliteKernel> = state.kernel.clone();
    let secrets: Arc<OsSecretStore> = state.secrets.clone();
    read_model_setup(kernel.as_ref(), secrets.as_ref()).await
}

#[tauri::command]
pub async fn save_model_setup(
    state: State<'_, AppState>,
    request: SaveModelSetupRequest,
) -> Result<ModelSetupView, AppError> {
    let kernel: Arc<SqliteKernel> = state.kernel.clone();
    let secrets: Arc<OsSecretStore> = state.secrets.clone();
    write_model_setup(
        kernel.as_ref(),
        secrets.as_ref(),
        &request.endpoint,
        &request.model,
        request.allow_localhost_http,
        &request.api_key,
    )
    .await
}

#[tauri::command]
pub async fn has_api_key(state: State<'_, AppState>) -> Result<bool, AppError> {
    let secrets: Arc<OsSecretStore> = state.secrets.clone();
    check_has_api_key(secrets.as_ref()).await
}

async fn read_model_setup(
    kernel: &SqliteKernel,
    secrets: &OsSecretStore,
) -> Result<Option<ModelSetupView>, AppError> {
    use novel_platform::SecretStore;
    let stored = kernel.load_model_setup().await.map_err(AppError::from)?;
    Ok(match stored {
        Some(stored) => {
            let has_api_key = secrets
                .get(&stored.secret_ref)
                .await
                .map_err(|_| AppError::from(PortError::Storage("系统凭据暂时不可用".into())))?
                .is_some();
            #[cfg(feature = "e2e")]
            {
                has_api_key = has_api_key || std::env::var("BIZ_NOVEL_E2E_ENDPOINT").is_ok();
            }
            Some(ModelSetupView {
                endpoint: stored.endpoint,
                model: stored.model,
                allow_localhost_http: stored.allow_localhost_http,
                has_api_key,
            })
        }
        None => None,
    })
}

async fn write_model_setup(
    kernel: &SqliteKernel,
    secrets: &OsSecretStore,
    endpoint: &str,
    model: &str,
    allow_localhost_http: bool,
    api_key: &str,
) -> Result<ModelSetupView, AppError> {
    use novel_platform::SecretStore;
    let now = novel_platform::now_ms();
    let secret_ref = novel_storage_sqlite::MODEL_SECRET_REF.to_string();
    secrets
        .put(&secret_ref, api_key.trim())
        .await
        .map_err(|_| AppError::from(PortError::Storage("无法写入系统凭据。".into())))?;
    kernel
        .save_model_setup(endpoint, model, allow_localhost_http, now)
        .await
        .map_err(AppError::from)?;
    Ok(ModelSetupView {
        endpoint: endpoint.to_string(),
        model: model.to_string(),
        allow_localhost_http,
        has_api_key: !api_key.trim().is_empty(),
    })
}

async fn check_has_api_key(secrets: &OsSecretStore) -> Result<bool, AppError> {
    use novel_platform::SecretStore;
    let secret_ref = novel_storage_sqlite::MODEL_SECRET_REF;
    let value = secrets
        .get(secret_ref)
        .await
        .map_err(|_| AppError::from(PortError::Storage("系统凭据暂时不可用".into())))?;
    Ok(value.is_some())
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestModelSetupRequest {
    pub endpoint: String,
    pub model: String,
    pub api_key: String,
    pub allow_localhost_http: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestModelSetupResult {
    pub ok: bool,
    pub message: String,
    pub latency_ms: u64,
}

/// 用一次最小生成验证「地址 + 模型 + 密钥」是否可用，供桌面端模型配置页直接反馈结果。
#[tauri::command]
pub async fn test_model_setup(
    request: TestModelSetupRequest,
) -> Result<TestModelSetupResult, AppError> {
    use novel_llm::LlmRequest;
    use tokio_util::sync::CancellationToken;

    let provider = crate::llm_provider();
    let started = std::time::Instant::now();
    let (tokens, mut receiver) = tokio::sync::mpsc::channel::<String>(8);
    let drain = tokio::spawn(async move { while receiver.recv().await.is_some() {} });
    let result = provider
        .generate(
            LlmRequest {
                request_id: format!("model-setup-test-{}", started.elapsed().as_nanos()),
                model: request.model.clone(),
                prompt: "回复 ok 两个字符即可。".into(),
                endpoint: request.endpoint.clone(),
                api_key: request.api_key.clone(),
                allow_localhost_http: request.allow_localhost_http,
                max_output_tokens: Some(16),
            },
            tokens,
            CancellationToken::new(),
        )
        .await;
    drop(drain);
    let latency_ms = started.elapsed().as_millis() as u64;
    Ok(match result {
        Ok(response) => TestModelSetupResult {
            ok: true,
            message: format!(
                "连接成功，模型返回：{}",
                response.text.trim().chars().take(40).collect::<String>()
            ),
            latency_ms,
        },
        Err(error) => TestModelSetupResult {
            ok: false,
            message: format!("{error}"),
            latency_ms,
        },
    })
}
