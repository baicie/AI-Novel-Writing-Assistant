fn main() {
    let attributes =
        tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(&[
            "list_novels",
            "list_chapters",
            "create_novel",
            "create_chapter",
            "get_chapter",
            "save_chapter",
            "open_run",
            "approve_run",
            "cancel_run",
            "cancel_generation",
            "resume_run",
            "replay_run",
            "subscribe_run",
            "get_model_setup",
            "save_model_setup",
            "has_api_key",
            "get_desktop_bootstrap_snapshot",
            "open_desktop_logs_directory",
            "restart_desktop_app",
        ]));
    tauri_build::try_build(attributes).expect("无法准备桌面命令权限");
}
