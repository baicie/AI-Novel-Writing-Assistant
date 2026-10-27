use novel_application::AppError;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

#[tauri::command]
pub async fn open_editor_window(
    app: AppHandle,
    novel_id: String,
    chapter_id: String,
) -> Result<String, AppError> {
    if let Some(window) = app.get_webview_window("editor-b") {
        let _ = window.set_focus();
        return Ok("editor-b".into());
    }
    let url = format!("#/novels/{novel_id}/chapters/{chapter_id}?writer=editor-b");
    WebviewWindowBuilder::new(&app, "editor-b", WebviewUrl::App(url.into()))
        .title("Biz Novel Studio · 第二编辑窗口")
        .inner_size(1180.0, 820.0)
        .build()
        .map_err(|error| AppError::validation(format!("无法打开第二编辑窗口：{error}")))?;
    Ok("editor-b".into())
}

#[tauri::command]
pub fn list_app_windows(app: AppHandle) -> Vec<String> {
    let mut labels: Vec<String> = app.webview_windows().keys().cloned().collect();
    labels.sort();
    labels
}

#[tauri::command]
pub fn close_editor_window(app: AppHandle) -> Result<(), AppError> {
    if let Some(window) = app.get_webview_window("editor-b") {
        window
            .close()
            .map_err(|error| AppError::validation(format!("无法关闭第二编辑窗口：{error}")))?;
    }
    Ok(())
}
