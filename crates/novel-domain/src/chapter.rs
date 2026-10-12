use serde::{Deserialize, Serialize};

use crate::error::DomainError;
use crate::workflow::ArtifactStatus;

/// 章节正文的覆盖保护。导入的非空旧正文按已人工保存处理，避免新内核自动改写。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChapterProtection {
    None,
    UserEdited,
    Published,
}

impl ChapterProtection {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::UserEdited => "user_edited",
            Self::Published => "published",
        }
    }

    pub fn parse(value: &str) -> Result<Self, DomainError> {
        match value {
            "none" => Ok(Self::None),
            "user_edited" => Ok(Self::UserEdited),
            "published" => Ok(Self::Published),
            _ => Err(DomainError::Invariant("unknown chapter protection")),
        }
    }
}

pub fn ensure_can_promote(protection: ChapterProtection) -> Result<(), DomainError> {
    match protection {
        ChapterProtection::None => Ok(()),
        ChapterProtection::UserEdited | ChapterProtection::Published => {
            Err(DomainError::ProtectedContent)
        }
    }
}

pub fn can_mark_stale(status: ArtifactStatus, protected_user_content: bool) -> bool {
    !protected_user_content && status != ArtifactStatus::UserEdited
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_and_published_text_cannot_be_promoted_over() {
        assert!(ensure_can_promote(ChapterProtection::None).is_ok());
        assert_eq!(
            ensure_can_promote(ChapterProtection::UserEdited),
            Err(DomainError::ProtectedContent)
        );
        assert_eq!(
            ensure_can_promote(ChapterProtection::Published),
            Err(DomainError::ProtectedContent)
        );
    }

    #[test]
    fn protected_artifacts_are_not_stale_candidates() {
        assert!(!can_mark_stale(ArtifactStatus::Confirmed, true));
        assert!(!can_mark_stale(ArtifactStatus::UserEdited, false));
        assert!(can_mark_stale(ArtifactStatus::Confirmed, false));
    }
}
