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

    fn intent_paths(dir: &Path, staged: &Path, original: &Path) -> PathBuf {
        let intent = RestoreIntent {
            source: "backup.sqlite".into(),
            staged: staged.to_string_lossy().to_string(),
            original: original.to_string_lossy().to_string(),
            created_at: 1,
        };
        std::fs::write(
            dir.join("restore-intent.json"),
            serde_json::to_vec(&intent).unwrap(),
        )
        .unwrap();
        dir.join("restore-intent.json")
    }

    async fn seed(kernel: &novel_storage_sqlite::SqliteKernel, id: &str, title: &str) {
        novel_application::NovelRepository::create_novel(
            kernel,
            novel_application::NewNovel {
                id: id.into(),
                title: title.into(),
                description: String::new(),
                now_ms: 1,
            },
        )
        .await
        .unwrap();
    }

    async fn titles(path: &Path) -> Vec<String> {
        let kernel = novel_storage_sqlite::SqliteKernel::open(path)
            .await
            .unwrap();
        let novels = novel_application::NovelRepository::list_novels(&kernel)
            .await
            .unwrap();
        let mut titles: Vec<String> = novels.into_iter().map(|item| item.title).collect();
        kernel.pool().close().await;
        titles.sort();
        titles
    }

    #[tokio::test]
    async fn no_intent_leaves_the_live_database_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("novel.sqlite");
        let kernel = novel_storage_sqlite::SqliteKernel::open(&db).await.unwrap();
        seed(&kernel, "n-live", "现有书").await;
        kernel.pool().close().await;
        // 崩溃在意图提交之前：只剩临时文件，没有正式 marker。
        std::fs::write(dir.path().join("restore-intent.tmp"), b"{\"partial\":").unwrap();

        let result = recover_pending_restore(dir.path(), &db).await.unwrap();
        assert!(result.is_none());
        assert_eq!(titles(&db).await, vec!["现有书".to_string()]);
        assert!(dir.path().join("restore-intent.tmp").exists());
    }

    #[tokio::test]
    async fn missing_staged_database_keeps_the_live_database() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("novel.sqlite");
        let kernel = novel_storage_sqlite::SqliteKernel::open(&db).await.unwrap();
        seed(&kernel, "n-live", "现有书").await;
        kernel.pool().close().await;
        let staged = dir.path().join("restore-staged.sqlite");
        let original = dir.path().join("pre-restore-live.sqlite");
        let marker = intent_paths(dir.path(), &staged, &original);

        let result = recover_pending_restore(dir.path(), &db).await.unwrap();
        assert!(result.is_none());
        assert!(!marker.exists());
        assert!(dir.path().join("restore-intent.failed.json").exists());
        assert_eq!(titles(&db).await, vec!["现有书".to_string()]);
    }

    #[tokio::test]
    async fn corrupt_staged_database_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("novel.sqlite");
        let kernel = novel_storage_sqlite::SqliteKernel::open(&db).await.unwrap();
        seed(&kernel, "n-live", "现有书").await;
        kernel.pool().close().await;
        let staged = dir.path().join("restore-staged.sqlite");
        std::fs::write(&staged, b"this is not a sqlite database").unwrap();
        let original = dir.path().join("pre-restore-live.sqlite");
        let marker = intent_paths(dir.path(), &staged, &original);

        let result = recover_pending_restore(dir.path(), &db).await;
        assert!(result.is_err(), "a corrupt staged copy must be refused");
        assert!(marker.exists(), "the intent stays for a later fix");
        assert!(!original.exists(), "the live database must not be moved");
        assert_eq!(titles(&db).await, vec!["现有书".to_string()]);
    }

    #[tokio::test]
    async fn safety_snapshot_includes_uncheckpointed_wal_rows() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("novel.sqlite");
        let kernel = novel_storage_sqlite::SqliteKernel::open(&db).await.unwrap();
        seed(&kernel, "n-wal", "还在 WAL 里的书").await;
        let wal = PathBuf::from(format!("{}-wal", db.display()));
        assert!(wal.exists(), "WAL 模式必须留下 -wal");
        assert!(
            std::fs::metadata(&wal).unwrap().len() > 0,
            "未 checkpoint 的提交必须写在 WAL 里"
        );
        let safety = dir.path().join("pre-restore-safety.sqlite");
        kernel.backup_to(&safety).await.unwrap();
        assert_eq!(
            titles(&safety).await,
            vec!["还在 WAL 里的书".to_string()],
            "保护副本必须包含尚未 checkpoint 的已提交数据"
        );
        kernel.pool().close().await;
    }

    #[tokio::test]
    async fn switch_moves_wal_sidecars_and_keeps_both_databases_readable() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("novel.sqlite");
        let live = novel_storage_sqlite::SqliteKernel::open(&db).await.unwrap();
        seed(&live, "n-live", "原书").await;
        let backup = dir.path().join("backup.sqlite");
        live.backup_to(&backup).await.unwrap();
        live.pool().close().await;
        let staged = dir.path().join("restore-staged.sqlite");
        std::fs::copy(&backup, &staged).unwrap();
        let original = dir.path().join("pre-restore-live.sqlite");
        intent_paths(dir.path(), &staged, &original);
        // 崩溃现场：主库旁边还留着未合并的 WAL/SHM。
        let live_wal = PathBuf::from(format!("{}-wal", db.display()));
        let live_shm = PathBuf::from(format!("{}-shm", db.display()));
        std::fs::write(&live_wal, b"stale-wal").unwrap();
        std::fs::write(&live_shm, b"stale-shm").unwrap();

        let schema = recover_pending_restore(dir.path(), &db).await.unwrap();
        assert_eq!(
            schema.as_deref(),
            Some(novel_storage_sqlite::SqliteKernel::latest_schema_version())
        );
        assert!(!live_wal.exists(), "旧 WAL 不能留在恢复后的主库旁边");
        assert!(!live_shm.exists());
        assert!(PathBuf::from(format!("{}-wal", original.display())).exists());
        assert!(PathBuf::from(format!("{}-shm", original.display())).exists());
        assert_eq!(titles(&db).await, vec!["原书".to_string()]);
        assert_eq!(
            titles(&original).await,
            vec!["原书".to_string()],
            "保护副本必须能独立打开"
        );
    }

    #[tokio::test]
    async fn a_newer_intent_replaces_the_pending_source() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("novel.sqlite");
        let live = novel_storage_sqlite::SqliteKernel::open(&db).await.unwrap();
        seed(&live, "n-live", "原书").await;
        live.pool().close().await;
        let older = dir.path().join("restore-staged-old.sqlite");
        let newer = dir.path().join("restore-staged-new.sqlite");
        std::fs::copy(&db, &older).unwrap();
        std::fs::copy(&db, &newer).unwrap();
        let original = dir.path().join("pre-restore-live.sqlite");
        intent_paths(dir.path(), &older, &original);
        intent_paths(dir.path(), &newer, &original);

        let schema = recover_pending_restore(dir.path(), &db).await.unwrap();
        assert!(schema.is_some());
        assert_eq!(titles(&db).await, vec!["原书".to_string()]);
        assert!(older.exists(), "被替换的旧 staged 文件不做处理");
        assert!(!newer.exists(), "新的 staged 文件被切换进主库");
        assert!(!dir.path().join("restore-intent.json").exists());
    }

    #[tokio::test]
    async fn backup_into_a_read_only_directory_fails_without_losing_the_source() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("novel.sqlite");
        let live = novel_storage_sqlite::SqliteKernel::open(&db).await.unwrap();
        seed(&live, "n-live", "原书").await;
        let read_only = dir.path().join("readonly");
        std::fs::create_dir_all(&read_only).unwrap();
        let mut permissions = std::fs::metadata(&read_only).unwrap().permissions();
        permissions.set_mode(0o555);
        std::fs::set_permissions(&read_only, permissions).unwrap();

        let target = read_only.join("backup.sqlite");
        let failed = live.backup_to(&target).await;
        let mut restore = std::fs::metadata(&read_only).unwrap().permissions();
        restore.set_mode(0o755);
        std::fs::set_permissions(&read_only, restore).unwrap();

        assert!(failed.is_err(), "只读目录下的备份必须失败");
        assert!(!target.exists());
        live.pool().close().await;
        assert_eq!(titles(&db).await, vec!["原书".to_string()]);
    }

    #[tokio::test]
    async fn restore_switch_failure_keeps_the_live_database_and_the_intent() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("novel.sqlite");
        let live = novel_storage_sqlite::SqliteKernel::open(&db).await.unwrap();
        seed(&live, "n-live", "原书").await;
        live.pool().close().await;
        let staged = dir.path().join("restore-staged.sqlite");
        std::fs::copy(&db, &staged).unwrap();
        let original = dir.path().join("pre-restore-live.sqlite");
        let marker = intent_paths(dir.path(), &staged, &original);

        // 数据目录只读：切换时的重命名必须失败，而不是把库切坏。
        let mut permissions = std::fs::metadata(dir.path()).unwrap().permissions();
        permissions.set_mode(0o555);
        std::fs::set_permissions(dir.path(), permissions).unwrap();
        let failed = recover_pending_restore(dir.path(), &db).await;
        let mut restore = std::fs::metadata(dir.path()).unwrap().permissions();
        restore.set_mode(0o755);
        std::fs::set_permissions(dir.path(), restore).unwrap();

        assert!(failed.is_err(), "切换失败必须显式报错");
        assert!(db.exists(), "原库必须还在原位");
        assert!(marker.exists(), "intent 保留以便修复后重试");
        assert!(staged.exists());
        assert_eq!(titles(&db).await, vec!["原书".to_string()]);
    }
}
