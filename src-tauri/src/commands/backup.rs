use std::path::{Path, PathBuf};

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

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RestoreIntent {
    source: String,
    staged: String,
    original: String,
    created_at: u64,
}

fn restore_marker(data_dir: &Path) -> PathBuf {
    data_dir.join("restore-intent.json")
}

fn copy_with_sidecars(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::rename(from, to).map_err(|error| format!("无法保护原数据库：{error}"))?;
    for suffix in ["-wal", "-shm"] {
        let source = PathBuf::from(format!("{}{}", from.display(), suffix));
        if source.exists() {
            let target = PathBuf::from(format!("{}{}", to.display(), suffix));
            let _ = std::fs::rename(source, target);
        }
    }
    Ok(())
}

/// Apply a previously requested restore before any Runtime opens the database.
/// The active database is never overwritten while a connection exists.
pub async fn recover_pending_restore(
    data_dir: &Path,
    db_path: &Path,
) -> Result<Option<String>, AppError> {
    let marker_path = restore_marker(data_dir);
    if !marker_path.exists() {
        return Ok(None);
    }
    let intent: RestoreIntent = serde_json::from_slice(
        &std::fs::read(&marker_path).map_err(|error| AppError::integrity(error.to_string()))?,
    )
    .map_err(|error| AppError::integrity(format!("恢复意图无法解析：{error}")))?;
    let staged = PathBuf::from(&intent.staged);
    let original = PathBuf::from(&intent.original);
    if !staged.exists() {
        let _ = std::fs::rename(&marker_path, data_dir.join("restore-intent.failed.json"));
        return Ok(None);
    }
    // Recover from a crash that happened after moving the original away.
    if !db_path.exists() && original.exists() {
        copy_with_sidecars(&original, db_path).map_err(AppError::integrity)?;
        let _ = std::fs::remove_file(&marker_path);
        return Ok(None);
    }
    let integrity = novel_storage_sqlite::SqliteKernel::verify_database_file(&staged)
        .await
        .map_err(|error| AppError::integrity(format!("恢复备份校验失败：{error}")))?;
    if integrity != "ok" {
        return Err(AppError::integrity("恢复备份完整性检查未通过。"));
    }
    let schema = novel_storage_sqlite::SqliteKernel::schema_version(&staged)
        .await
        .map_err(|error| AppError::integrity(format!("恢复备份版本读取失败：{error}")))?;
    if schema.as_str() > novel_storage_sqlite::SqliteKernel::latest_schema_version() {
        return Err(AppError::integrity(
            "备份来自更新版本的程序，当前程序无法安全恢复。",
        ));
    }
    std::fs::rename(db_path, &original)
        .map_err(|error| AppError::integrity(format!("无法保存当前数据库：{error}")))?;
    for suffix in ["-wal", "-shm"] {
        let source = PathBuf::from(format!("{}{}", db_path.display(), suffix));
        if source.exists() {
            let target = PathBuf::from(format!("{}{}", original.display(), suffix));
            let _ = std::fs::rename(source, target);
        }
    }
    if let Err(error) = std::fs::rename(&staged, db_path) {
        let _ = std::fs::rename(&original, db_path);
        return Err(AppError::integrity(format!("切换恢复数据库失败：{error}")));
    }
    let _ = std::fs::remove_file(&marker_path);
    Ok(Some(schema))
}

#[tauri::command]
pub async fn request_restore(
    state: State<'_, AppState>,
    file_name: String,
) -> Result<String, AppError> {
    let source = safe_backup_path(&state, &file_name)?;
    if !source.is_file() {
        return Err(AppError::validation("备份文件不存在。"));
    }
    let integrity = novel_storage_sqlite::SqliteKernel::verify_database_file(&source)
        .await
        .map_err(|error| AppError::integrity(format!("备份校验失败：{error}")))?;
    if integrity != "ok" {
        return Err(AppError::integrity("备份完整性检查未通过。"));
    }
    let schema = novel_storage_sqlite::SqliteKernel::schema_version(&source)
        .await
        .map_err(|error| AppError::integrity(format!("备份版本读取失败：{error}")))?;
    if schema.as_str() > novel_storage_sqlite::SqliteKernel::latest_schema_version() {
        return Err(AppError::integrity(
            "备份来自更新版本的程序，当前程序无法安全恢复。",
        ));
    }
    let dir = backups_dir(&state);
    std::fs::create_dir_all(&dir).map_err(|error| AppError::integrity(error.to_string()))?;
    let safety = dir.join(format!("pre-restore-{}.sqlite", novel_platform::now_ms()));
    state
        .kernel
        .backup_to(&safety)
        .await
        .map_err(|error| AppError::integrity(format!("恢复前保护备份失败：{error}")))?;
    let staged = state.data_dir.join("restore-staged.sqlite");
    std::fs::copy(&source, &staged)
        .map_err(|error| AppError::integrity(format!("准备恢复数据库失败：{error}")))?;
    let original = state.data_dir.join(format!(
        "pre-restore-live-{}.sqlite",
        novel_platform::now_ms()
    ));
    let intent = RestoreIntent {
        source: file_name,
        staged: staged.to_string_lossy().to_string(),
        original: original.to_string_lossy().to_string(),
        created_at: novel_platform::now_ms() as u64,
    };
    let marker = restore_marker(&state.data_dir);
    let temp = state.data_dir.join("restore-intent.tmp");
    std::fs::write(
        &temp,
        serde_json::to_vec(&intent).map_err(|error| AppError::integrity(error.to_string()))?,
    )
    .map_err(|error| AppError::integrity(format!("写入恢复意图失败：{error}")))?;
    std::fs::rename(&temp, &marker)
        .map_err(|error| AppError::integrity(format!("提交恢复意图失败：{error}")))?;
    Ok("恢复已准备完成。请关闭并重新打开应用，在启动时完成安全切换。".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn startup_restore_switches_staged_database_and_keeps_protection_copy() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("novel.sqlite");
        let kernel = novel_storage_sqlite::SqliteKernel::open(&db).await.unwrap();
        novel_application::NovelRepository::create_novel(
            &kernel,
            novel_application::NewNovel {
                id: "n1".into(),
                title: "恢复书".into(),
                description: String::new(),
                now_ms: 1,
            },
        )
        .await
        .unwrap();
        let backup = dir.path().join("backup.sqlite");
        kernel.backup_to(&backup).await.unwrap();
        kernel.pool().close().await;
        let staged = dir.path().join("restore-staged.sqlite");
        std::fs::copy(&backup, &staged).unwrap();
        let original = dir.path().join("pre-restore-live.sqlite");
        let intent = RestoreIntent {
            source: "backup.sqlite".into(),
            staged: staged.to_string_lossy().to_string(),
            original: original.to_string_lossy().to_string(),
            created_at: 1,
        };
        std::fs::write(
            dir.path().join("restore-intent.json"),
            serde_json::to_vec(&intent).unwrap(),
        )
        .unwrap();
        let schema = recover_pending_restore(dir.path(), &db).await.unwrap();
        assert_eq!(
            schema.as_deref(),
            Some(novel_storage_sqlite::SqliteKernel::latest_schema_version())
        );
        assert!(db.exists());
        assert!(original.exists());
        assert!(!dir.path().join("restore-intent.json").exists());
        let reopened = novel_storage_sqlite::SqliteKernel::open(&db).await.unwrap();
        let novels = novel_application::NovelRepository::list_novels(&reopened)
            .await
            .unwrap();
        assert_eq!(novels.len(), 1);
    }

    #[tokio::test]
    async fn crash_between_original_move_and_staged_switch_restores_original() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("novel.sqlite");
        let kernel = novel_storage_sqlite::SqliteKernel::open(&db).await.unwrap();
        novel_application::NovelRepository::create_novel(
            &kernel,
            novel_application::NewNovel {
                id: "n1".into(),
                title: "原书".into(),
                description: String::new(),
                now_ms: 1,
            },
        )
        .await
        .unwrap();
        kernel.pool().close().await;
        let staged = dir.path().join("restore-staged.sqlite");
        std::fs::copy(&db, &staged).unwrap();
        let original = dir.path().join("pre-restore-live.sqlite");
        std::fs::rename(&db, &original).unwrap();
        let intent = RestoreIntent {
            source: "x".into(),
            staged: staged.to_string_lossy().to_string(),
            original: original.to_string_lossy().to_string(),
            created_at: 1,
        };
        std::fs::write(
            dir.path().join("restore-intent.json"),
            serde_json::to_vec(&intent).unwrap(),
        )
        .unwrap();
        let result = recover_pending_restore(dir.path(), &db).await.unwrap();
        assert!(result.is_none());
        assert!(db.exists());
        assert!(!dir.path().join("restore-intent.json").exists());
    }
}
