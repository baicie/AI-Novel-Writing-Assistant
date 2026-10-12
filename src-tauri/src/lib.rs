use std::path::PathBuf;
use std::sync::Arc;

use novel_application::{
    AppError, ChapterSnapshot, CommandOutcome, GenerationEvent, ModelSecrets, NoFault, NovelSummary,
    OpenRunInput, PortError, Runtime, RuntimeDeps, SystemClock, TickOutcome, UuidIds,
};
use novel_llm::UnconfiguredProvider;
use novel_platform::OsSecretStore;
use novel_storage_sqlite::SqliteKernel;
use serde::Deserialize;
use tauri::ipc::Channel;
use tauri::{Manager, State};

struct AppState {
    runtime: Arc<Runtime>,
    kernel: Arc<SqliteKernel>,
    secrets: Arc<OsSecretStore>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenRunRequest {
    command_id: String,
    novel_id: String,
    target_chapter_id: String,
    provider: String,
    model: String,
    prompt_versions: std::collections::BTreeMap<String, String>,
    token_budget: Option<u64>,
    max_automatic_retries: u8,
    endpoint: String,
    allow_localhost_http: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveModelSetupRequest {
    endpoint: String,
    model: String,
    allow_localhost_http: bool,
    api_key: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelSetupView {
    endpoint: String,
    model: String,
    allow_localhost_http: bool,
    has_api_key: bool,
}

struct OsModelSecrets {
    store: Arc<OsSecretStore>,
}

#[async_trait::async_trait]
impl ModelSecrets for OsModelSecrets {
    async fn api_key(&self, provider: &str) -> Result<Option<String>, PortError> {
        use novel_platform::SecretStore;
        self.store
            .get(provider)
            .await
            .map_err(|_| PortError::Storage("系统凭据暂时不可用".into()))
    }
}

fn data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("BIZ_NOVEL_DATA_DIR") {
        return PathBuf::from(dir);
    }
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Biz Novel Studio")
}

#[tauri::command]
async fn list_chapters(
    state: State<'_, AppState>,
    novel_id: String,
) -> Result<Vec<ChapterSnapshot>, AppError> {
    state.runtime.director.list_chapters(&novel_id).await
}

#[tauri::command]
async fn get_model_setup(state: State<'_, AppState>) -> Result<Option<ModelSetupView>, AppError> {
    let kernel: Arc<SqliteKernel> = state.kernel.clone();
    let secrets: Arc<OsSecretStore> = state.secrets.clone();
    let view =
        read_model_setup(kernel.as_ref(), secrets.as_ref()).await?;
    Ok(view)
}

#[tauri::command]
async fn save_model_setup(
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
async fn has_api_key(state: State<'_, AppState>) -> Result<bool, AppError> {
    let secrets: Arc<OsSecretStore> = state.secrets.clone();
    Ok(check_has_api_key(secrets.as_ref()).await?)
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
    let value = secrets
        .get(secret_ref)
        .await
        .map_err(|_| AppError::from(PortError::Storage("无法读取系统凭据。".into())))?;
    Ok(value.is_some())
}

#[tauri::command]
async fn cancel_generation(state: State<'_, AppState>, run_id: String) -> Result<(), AppError> {
    state.runtime.worker.cancel_generation(&run_id);
    Ok(())
}

#[tauri::command]
async fn list_novels(state: State<'_, AppState>) -> Result<Vec<NovelSummary>, AppError> {
    state.runtime.director.list_novels().await
}

#[tauri::command]
async fn create_novel(
    state: State<'_, AppState>,
    title: String,
    description: String,
) -> Result<NovelSummary, AppError> {
    state
        .runtime
        .director
        .create_novel(&title, &description)
        .await
}

#[tauri::command]
async fn create_chapter(
    state: State<'_, AppState>,
    novel_id: String,
    title: String,
    sort_order: i32,
) -> Result<ChapterSnapshot, AppError> {
    state
        .runtime
        .director
        .create_chapter(&novel_id, &title, sort_order)
        .await
}

#[tauri::command]
async fn get_chapter(
    state: State<'_, AppState>,
    novel_id: String,
    chapter_id: String,
) -> Result<ChapterSnapshot, AppError> {
    state
        .runtime
        .director
        .get_chapter(&novel_id, &chapter_id)
        .await
}

#[tauri::command]
async fn save_chapter(
    state: State<'_, AppState>,
    novel_id: String,
    chapter_id: String,
    expected_version: i64,
    title: String,
    content: String,
) -> Result<ChapterSnapshot, AppError> {
    state
        .runtime
        .director
        .save_chapter(&novel_id, &chapter_id, expected_version, &title, &content)
        .await
}

#[tauri::command]
async fn open_run(
    state: State<'_, AppState>,
    request: OpenRunRequest,
) -> Result<CommandOutcome, AppError> {
    state
        .runtime
        .director
        .open_run(OpenRunInput {
            command_id: request.command_id,
            novel_id: request.novel_id,
            target_chapter_id: request.target_chapter_id,
            provider: request.provider,
            model: request.model,
            prompt_versions: request.prompt_versions,
            token_budget: request.token_budget,
            max_automatic_retries: request.max_automatic_retries,
            endpoint: request.endpoint,
            allow_localhost_http: request.allow_localhost_http,
        })
        .await
}

#[tauri::command]
async fn approve_run(
    state: State<'_, AppState>,
    command_id: String,
    run_id: String,
) -> Result<CommandOutcome, AppError> {
    state.runtime.director.approve(&command_id, &run_id).await
}

#[tauri::command]
async fn cancel_run(
    state: State<'_, AppState>,
    command_id: String,
    run_id: String,
) -> Result<CommandOutcome, AppError> {
    state.runtime.director.cancel(&command_id, &run_id).await
}

#[tauri::command]
async fn resume_run(
    state: State<'_, AppState>,
    command_id: String,
    run_id: String,
    accept_candidate: bool,
) -> Result<CommandOutcome, AppError> {
    state
        .runtime
        .director
        .resume(&command_id, &run_id, accept_candidate)
        .await
}

#[tauri::command]
async fn replay_run(
    state: State<'_, AppState>,
    run_id: String,
    after_seq: i64,
) -> Result<Vec<GenerationEvent>, AppError> {
    state.runtime.director.replay(&run_id, after_seq).await
}

#[tauri::command]
async fn subscribe_run(
    state: State<'_, AppState>,
    run_id: String,
    channel: Channel<GenerationEvent>,
) -> Result<(), AppError> {
    let mut receiver = state.runtime.hub.subscribe();
    tauri::async_runtime::spawn(async move {
        loop {
            match receiver.recv().await {
                Ok(event) if event.run_id() == run_id => {
                    if channel.send(event).is_err() {
                        break;
                    }
                }
                Ok(_) => {}
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
    Ok(())
}

pub fn run() {
    novel_platform::init_logging();
    tauri::Builder::default()
        .setup(|app| {
            let dir = data_dir();
            std::fs::create_dir_all(&dir)
                .map_err(|error| format!("无法准备本地资料目录：{error}"))?;
            let db_path = dir.join("novel.sqlite");
            let secrets: Arc<OsSecretStore> = Arc::new(OsSecretStore);
            let model_secrets: Arc<dyn ModelSecrets> = Arc::new(OsModelSecrets {
                store: secrets.clone(),
            });
            let (kernel, runtime) = tauri::async_runtime::block_on(async move {
                let kernel = Arc::new(
                    SqliteKernel::open(&db_path)
                        .await
                        .map_err(|error| error.to_string())?,
                );
                let runtime = Arc::new(Runtime::new(RuntimeDeps {
                    novels: kernel.clone(),
                    runs: kernel.clone(),
                    llm: Arc::new(UnconfiguredProvider),
                    clock: Arc::new(SystemClock),
                    ids: Arc::new(UuidIds),
                    faults: Arc::new(NoFault),
                    secrets: model_secrets,
                    owner: format!("desktop-{}", std::process::id()),
                    lease_ms: 30_000,
                    concurrency: 1,
                }));
                Ok::<_, String>((kernel, runtime))
            })?;
            let worker = runtime.clone();
            app.manage(AppState {
                runtime: runtime.clone(),
                kernel: kernel.clone(),
                secrets: secrets.clone(),
            });
            tauri::async_runtime::spawn(async move {
                loop {
                    match worker.worker.tick().await {
                        Ok(TickOutcome::Stopped) => break,
                        Ok(_) => {}
                        Err(error) => tracing::error!(%error, "创作步骤没有完成"),
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_novels,
            list_chapters,
            create_novel,
            create_chapter,
            get_chapter,
            save_chapter,
            open_run,
            approve_run,
            cancel_run,
            cancel_generation,
            resume_run,
            replay_run,
            subscribe_run,
            get_model_setup,
            save_model_setup,
            has_api_key
        ])
        .build(tauri::generate_context!())
        .expect("无法打开本地创作窗口")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event
                && let Some(state) = app.try_state::<AppState>()
            {
                state.runtime.worker.stop();
            }
        });
}
