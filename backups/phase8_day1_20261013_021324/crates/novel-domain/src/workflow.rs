use serde::{Deserialize, Serialize};

use crate::error::DomainError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactStatus {
    Draft,
    Confirmed,
    UserEdited,
    Stale,
}

impl ArtifactStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Confirmed => "confirmed",
            Self::UserEdited => "user_edited",
            Self::Stale => "stale",
        }
    }

    pub fn parse(value: &str) -> Result<Self, DomainError> {
        match value {
            "draft" => Ok(Self::Draft),
            "confirmed" => Ok(Self::Confirmed),
            "user_edited" => Ok(Self::UserEdited),
            "stale" => Ok(Self::Stale),
            _ => Err(DomainError::Invariant("unknown artifact status")),
        }
    }

    pub fn satisfies_dependency(self, gateable: bool) -> bool {
        match self {
            Self::Confirmed | Self::UserEdited => true,
            Self::Draft => !gateable,
            Self::Stale => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactView {
    pub artifact_type: String,
    pub scope: String,
    pub version: i64,
    pub status: ArtifactStatus,
    pub protected_user_content: bool,
    pub payload_json: String,
}
