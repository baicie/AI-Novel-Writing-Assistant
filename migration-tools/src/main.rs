use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use novel_platform::now_ms;
use novel_storage_sqlite::import_legacy;

fn main() -> ExitCode {
    let mut source = None;
    let mut target = None;
    let mut report_path = None;
    let mut args = env::args().skip(1);
    while let Some(flag) = args.next() {
        let value = args.next().map(PathBuf::from);
        match flag.as_str() {
            "--source" => source = value,
            "--target" => target = value,
            "--report" => report_path = value,
            "--help" | "-h" => {
                println!(
                    "用法: migration-tool --source 旧库.sqlite --target 新库.sqlite [--report 报告.json]"
                );
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!(
                    "无法识别的参数 {other}。用法: migration-tool --source 旧库 --target 新库 [--report 报告]"
                );
                return ExitCode::from(2);
            }
        }
    }
    let (Some(source), Some(target)) = (source, target) else {
        eprintln!("需要同时提供 --source 和 --target。旧库只会只读打开。");
        return ExitCode::from(2);
    };
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("无法准备导入运行时：{error}");
            return ExitCode::from(1);
        }
    };
    match runtime.block_on(import_legacy(&source, &target, now_ms())) {
        Ok(report) => {
            let json = match serde_json::to_string_pretty(&report) {
                Ok(json) => json,
                Err(error) => {
                    eprintln!("导入完成，但报告无法写出：{error}");
                    return ExitCode::from(1);
                }
            };
            if let Some(path) = report_path
                && let Err(error) = std::fs::write(&path, json.as_bytes())
            {
                eprintln!("导入完成，但报告没有写到 {}：{error}", path.display());
                return ExitCode::from(1);
            }
            println!("{json}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}
