//! 双 binary 回归门禁：桌面包同时含主程序与 M1 验收执行器时，Tauri CLI 必须能
//! 通过 `package.default-run` 找到打包入口。缺失时 Linux Bundle 会失败于
//! "failed to find main binary, make sure you have a package > default-run"。

#[test]
fn tauri_package_has_an_explicit_main_binary() {
    let manifest = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))
        .expect("src-tauri/Cargo.toml should be readable");
    assert!(
        manifest.contains("default-run = \"novel-desktop\""),
        "package.default-run must point at the desktop binary, not the M1 acceptance runner"
    );
    assert!(
        std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/src/main.rs")).is_file(),
        "the desktop entrypoint src/main.rs must stay in place"
    );
    assert!(
        std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/src/bin/m1_run.rs")).is_file(),
        "the M1 acceptance runner stays available as an explicit `--bin m1_run` target"
    );
}
