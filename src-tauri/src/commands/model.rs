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
                .map(|value| value.is_some())
                .unwrap_or(false);
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
    let value = secrets.get(secret_ref).await.unwrap_or(None);
    Ok(value.is_some())
}
