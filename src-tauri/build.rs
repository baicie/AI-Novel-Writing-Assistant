fn main() {
    let attributes =
        tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(&[
            "list_novels",
            "create_novel",
            "create_chapter",
            "get_chapter",
            "save_chapter",
            "open_run",
            "approve_run",
            "cancel_run",
            "resume_run",
            "replay_run",
            "subscribe_run",
        ]));
    tauri_build::try_build(attributes).expect("无法准备桌面命令权限");
}
