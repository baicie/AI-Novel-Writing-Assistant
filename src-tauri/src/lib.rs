use std::path::PathBuf;
use std::sync::Arc;

use novel_application::{
    AppError, ChapterSnapshot, CommandOutcome, GenerationEvent, NoFault, NovelSummary,
    OpenRunInput, Runtime, RuntimeDeps, SystemClock, TickOutcome, UuidIds,
};
use novel_llm::UnconfiguredProvider;
use novel_storage_sqlite::SqliteKernel;
use serde::Deserialize;
use tauri::ipc::Channel;
use tauri::{Manager, State};

struct AppState {
    runtime: Arc<Runtime>,
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

fn data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("BIZ_NOVEL_DATA_DIR") {
        return PathBuf::from(dir);
    }
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Biz Novel Studio")
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
            let runtime = tauri::async_runtime::block_on(async move {
                let kernel = Arc::new(
                    SqliteKernel::open(&db_path)
                        .await
                        .map_err(|error| error.to_string())?,
                );
                Ok::<_, String>(Arc::new(Runtime::new(RuntimeDeps {
                    novels: kernel.clone(),
                    runs: kernel,
                    llm: Arc::new(UnconfiguredProvider),
                    clock: Arc::new(SystemClock),
                    ids: Arc::new(UuidIds),
                    faults: Arc::new(NoFault),
                    owner: format!("desktop-{}", std::process::id()),
                    lease_ms: 30_000,
                    concurrency: 1,
                })))
            })?;
            let worker = runtime.clone();
            app.manage(AppState { runtime });
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
            create_novel,
            create_chapter,
            get_chapter,
            save_chapter,
            open_run,
            approve_run,
            cancel_run,
            resume_run,
            replay_run,
            subscribe_run
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
