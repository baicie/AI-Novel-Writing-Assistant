// Tauri 命令是 IPC 入口，参数列表直接对应前端调用，因此显式允许参数数量告警。
#![allow(clippy::too_many_arguments)]

pub mod backup;
pub mod batch;
pub mod editor;
pub mod generation;
pub mod knowledge;
pub mod model;
pub mod native_tasks;
pub mod novel;
pub mod run;
pub mod setup;
pub mod shell;
pub mod structured_memory;
pub mod tasks;
pub mod windows;
