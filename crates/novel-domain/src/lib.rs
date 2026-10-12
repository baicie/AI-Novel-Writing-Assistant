//! 纯领域模型。这里不允许引用 Tauri、SQLx、HTTP 客户端或系统时间。
//! 版本号、序号和租赁到期时间都由调用方传入。

mod chapter;
mod error;
mod run;
mod workflow;

pub use chapter::{ChapterProtection, can_mark_stale, ensure_can_promote};
pub use error::DomainError;
pub use run::{
    ControlEvent, PauseKind, RunControl, RunGate, RunPause, RunStatus, StopSignalKind, apply_event,
    initial_control, note_lease, parse_status,
};
pub use workflow::{ArtifactStatus, ArtifactView};
