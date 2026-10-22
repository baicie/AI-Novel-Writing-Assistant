use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::Manager;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapSnapshot {
    pub state: &'static str,
    pub stage: &'static str,
    pub title: String,
    pub detail: String,
    pub log_dir: String,
    pub log_file: String,
    pub updated_at_ms: i64,
    pub can_retry: bool,
}

fn logs_dir(app: &tauri::AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("logs")
}

#[tauri::command]
pub fn get_desktop_bootstrap_snapshot(app: tauri::AppHandle) -> BootstrapSnapshot {
    tracing::info!("desktop bootstrap snapshot requested by webview");
    let dir = logs_dir(&app);
    let log_file = dir.join("desktop.log");
    BootstrapSnapshot {
        state: "ready",
        stage: "server-healthy",
        title: "创作工作台已就绪".into(),
        detail: "本地书稿资料已打开。".into(),
        log_dir: dir.display().to_string(),
        log_file: log_file.display().to_string(),
        updated_at_ms: novel_platform::now_ms(),
        can_retry: false,
    }
}

#[tauri::command]
pub fn open_desktop_logs_directory(app: tauri::AppHandle) -> Result<(), String> {
    open_directory(&logs_dir(&app)).map_err(|error| format!("无法打开日志目录：{error}"))
}

#[tauri::command]
pub fn restart_desktop_app(app: tauri::AppHandle) {
    app.request_restart();
}

fn open_directory(path: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(path)?;
    let program = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    };
    let mut child = std::process::Command::new(program).arg(path).spawn()?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}
