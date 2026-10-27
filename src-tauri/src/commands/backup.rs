use std::path::PathBuf;

use novel_application::AppError;
use serde::Serialize;
use tauri::State;

use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub file_name: String,
    pub bytes: u64,
    pub modified_at: u64,
}

fn backups_dir(state: &AppState) -> PathBuf {
    state.data_dir.join("backups")
}

fn safe_backup_path(state: &AppState, file_name: &str) -> Result<PathBuf, AppError> {
    if file_name.is_empty()
        || file_name.contains('/')
        || file_name.contains('\\')
        || file_name.contains("..")
    {
        return Err(AppError::validation("备份文件名无效。"));
    }
    Ok(backups_dir(state).join(file_name))
}

#[tauri::command]
pub async fn list_backups(state: State<'_, AppState>) -> Result<Vec<BackupInfo>, AppError> {
    let dir = backups_dir(&state);
    std::fs::create_dir_all(&dir)
        .map_err(|error| AppError::integrity(format!("无法读取备份目录：{error}")))?;
    let mut items = Vec::new();
    for entry in std::fs::read_dir(&dir)
        .map_err(|error| AppError::integrity(format!("无法读取备份目录：{error}")))?
    {
        let entry = entry.map_err(|error| AppError::integrity(error.to_string()))?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("sqlite") {
            continue;
        }
        let metadata = entry
            .metadata()
            .map_err(|error| AppError::integrity(error.to_string()))?;
        let modified = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or(0);
        items.push(BackupInfo {
            file_name: entry.file_name().to_string_lossy().to_string(),
            bytes: metadata.len(),
            modified_at: modified,
        });
    }
    items.sort_by(|left, right| right.modified_at.cmp(&left.modified_at));
    Ok(items)
}

#[tauri::command]
pub async fn create_backup(state: State<'_, AppState>) -> Result<BackupInfo, AppError> {
    let dir = backups_dir(&state);
    std::fs::create_dir_all(&dir)
        .map_err(|error| AppError::integrity(format!("无法创建备份目录：{error}")))?;
    let file_name = format!("novel-{}.sqlite", novel_platform::now_ms());
    let target = dir.join(&file_name);
    let temp = dir.join(format!(".{file_name}.tmp"));
    let _ = std::fs::remove_file(&temp);
    state
        .kernel
        .backup_to(&temp)
        .await
        .map_err(|error| AppError::integrity(format!("创建备份失败：{error}")))?;
    if let Err(error) = std::fs::rename(&temp, &target) {
        let _ = std::fs::remove_file(&temp);
        return Err(AppError::integrity(format!("保存备份失败：{error}")));
    }
    let metadata =
        std::fs::metadata(&target).map_err(|error| AppError::integrity(error.to_string()))?;
    Ok(BackupInfo {
        file_name,
        bytes: metadata.len(),
        modified_at: novel_platform::now_ms() as u64,
    })
}

#[tauri::command]
pub async fn verify_backup(
    state: State<'_, AppState>,
    file_name: String,
) -> Result<String, AppError> {
    let path = safe_backup_path(&state, &file_name)?;
    let integrity =
        novel_storage_sqlite::SqliteKernel::verify_database_file(std::path::Path::new(&path))
            .await
            .map_err(|error| AppError::integrity(format!("备份校验失败：{error}")))?;
    if integrity != "ok" {
        return Err(AppError::integrity("备份完整性检查未通过。"));
    }
    Ok(integrity)
}
