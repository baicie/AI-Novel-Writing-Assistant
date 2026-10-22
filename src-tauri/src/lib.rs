mod commands;
mod state;

use std::sync::Arc;

use novel_application::{Runtime, RuntimeDeps, SystemClock, TickOutcome, UuidIds};
use novel_llm::OpenAiCompatibleProvider;
use novel_platform::OsSecretStore;
use novel_storage_sqlite::SqliteKernel;
use state::AppState;
use tauri::Manager;

pub fn run() {
    novel_platform::init_logging();

    tauri::Builder::default()
        .setup(|app| {
            let data_dir = std::env::var_os("BIZ_NOVEL_DATA_DIR")
                .map(std::path::PathBuf::from)
                .map(Ok)
                .unwrap_or_else(|| {
                    app.path()
                        .app_data_dir()
                        .map_err(|error| format!("无法准备本地资料目录：{error}"))
                })?;
            std::fs::create_dir_all(&data_dir)
                .map_err(|error| format!("无法准备本地资料目录：{error}"))?;
            std::fs::create_dir_all(data_dir.join("logs"))
                .map_err(|error| format!("无法准备日志目录：{error}"))?;

            let db_path = data_dir.join("novel.sqlite");
            let secrets: Arc<OsSecretStore> = Arc::new(OsSecretStore);
            let model_secrets: Arc<dyn novel_application::ModelSecrets> =
                Arc::new(commands::model::OsModelSecrets {
                    store: secrets.clone(),
                });
            let llm: Arc<dyn novel_llm::LlmProvider> = Arc::new(OpenAiCompatibleProvider::new());
            let (kernel, runtime) = tauri::async_runtime::block_on(async {
                let kernel = Arc::new(
                    SqliteKernel::open(&db_path)
                        .await
                        .map_err(|error| error.to_string())?,
                );
                let runtime = Arc::new(Runtime::new(RuntimeDeps {
                    novels: kernel.clone(),
                    runs: kernel.clone(),
                    worlds: kernel.clone(),
                    lookups: kernel.clone(),
                    task_center: kernel.clone(),
                    llm: llm.clone(),
                    clock: Arc::new(SystemClock),
                    ids: Arc::new(UuidIds),
                    faults: Arc::new(novel_application::NoFault),
                    secrets: model_secrets,
                    owner: format!("desktop-{}", std::process::id()),
                    lease_ms: 30_000,
                    concurrency: 1,
                }));
                Ok::<_, String>((kernel, runtime))
            })?;

            app.manage(AppState {
                runtime: runtime.clone(),
                kernel,
                secrets,
                llm,
                cancellations: std::sync::Mutex::new(std::collections::HashMap::new()),
            });

            let worker = runtime.clone();
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

            let start_route = std::env::var("BIZ_NOVEL_START_ROUTE").ok();
            let route_script = start_route
                .as_ref()
                .filter(|route| route.starts_with('/'))
                .map(|route| format!("window.location.hash = {};", serde_json::json!(format!("#{route}"))))
                .unwrap_or_default();
            let save_smoke_script = if std::env::var("BIZ_NOVEL_SMOKE_SAVE").is_ok() {
                "window.__AI_NOVEL_SMOKE_SAVE__ = true;"
            } else {
                ""
            };
            let adapter_smoke_script = if std::env::var("BIZ_NOVEL_SMOKE_ADAPTER").is_ok() {
                "window.__AI_NOVEL_SMOKE_ADAPTER__ = true;"
            } else {
                ""
            };
            let runtime_script = format!(
                "window.__AI_NOVEL_RUNTIME__ = {}; window.__AI_NOVEL_START_ROUTE__ = {}; {route_script}",
                serde_json::json!({
                    "mode": "desktop",
                    "isPackaged": !tauri::is_dev(),
                    "appVersion": app.package_info().version.to_string(),
                    "isPortable": false,
                    "updateChannel": "beta",
                }),
                serde_json::json!(start_route)
            );
            let runtime_script = format!("{runtime_script}{save_smoke_script}{adapter_smoke_script}");
            let webview_url = if tauri::is_dev() {
                let dev_url = app
                    .config()
                    .build
                    .dev_url
                    .clone()
                    .ok_or("开发模式缺少前端地址")?;
                tauri::WebviewUrl::External(dev_url)
            } else {
                tauri::WebviewUrl::App(std::path::PathBuf::from("index.html"))
            };
            tauri::WebviewWindowBuilder::new(app.handle(), "main", webview_url)
                .title("Biz Novel Studio")
                .inner_size(1400.0, 900.0)
                .min_inner_size(1000.0, 700.0)
                .initialization_script(runtime_script)
                .build()
                .map_err(|error| format!("无法创建主窗口：{error}"))?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::novel::list_novels,
            commands::novel::get_novel,
            commands::novel::create_novel,
            commands::novel::create_novel_with_setup,
            commands::novel::update_novel,
            commands::novel::delete_novel,
            commands::novel::list_chapters,
            commands::novel::list_characters,
            commands::novel::create_chapter,
            commands::novel::get_chapter,
            commands::novel::save_chapter,
            commands::editor::get_chapter_editor_workspace,
            commands::novel::delete_chapter,
            commands::novel::update_chapter_meta,
            commands::novel::export_novel,
            commands::novel::list_chapter_revisions,
            commands::novel::list_volumes,
            commands::novel::create_volume,
            commands::novel::update_volume,
            commands::novel::delete_volume,
            commands::run::list_runs,
            commands::run::list_all_runs,
            commands::run::open_run,
            commands::run::approve_run,
            commands::run::cancel_run,
            commands::run::resume_run,
            commands::run::replay_run,
            commands::run::subscribe_run,
            commands::run::cancel_generation,
            commands::model::get_model_setup,
            commands::model::save_model_setup,
            commands::model::has_api_key,
            commands::knowledge::index_knowledge,
            commands::knowledge::search_knowledge,
            commands::knowledge::search_knowledge_vector,
            commands::setup::list_genres,
            commands::setup::list_story_modes,
            commands::tasks::get_task_overview,
            commands::tasks::list_recovery_candidates,
            commands::setup::get_quick_setup_status,
            commands::setup::get_first_novel_onboarding,
            commands::setup::list_worlds,
            commands::setup::create_world,
            commands::setup::delete_world,
            commands::generation::generate_text,
            commands::generation::cancel_text_generation,
            commands::shell::get_desktop_bootstrap_snapshot,
            commands::shell::report_client_runtime,
            commands::shell::open_desktop_logs_directory,
            commands::shell::restart_desktop_app,
        ])
        .build(tauri::generate_context!())
        .expect("无法打开本地创作窗口")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                if let Some(state) = app.try_state::<AppState>() {
                    state.runtime.worker.stop();
                }
            }
        });
}
