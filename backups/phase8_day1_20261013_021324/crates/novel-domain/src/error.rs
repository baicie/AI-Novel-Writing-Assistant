use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DomainError {
    #[error("invalid transition from {from} via {event}")]
    InvalidTransition { from: String, event: String },
    #[error("version conflict expected {expected} actual {actual}")]
    VersionConflict { expected: i64, actual: i64 },
    #[error("protected chapter cannot be overwritten")]
    ProtectedContent,
    #[error("{0}")]
    Invariant(&'static str),
}
