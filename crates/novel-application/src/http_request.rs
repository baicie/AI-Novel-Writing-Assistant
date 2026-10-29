//! HTTP 层 - 请求 DTO
//!
//! 定义从客户端发送的请求数据结构。

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ============================================================================
// 小说请求 DTO
// ============================================================================

/// 创建小说请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateNovelRequest {
    pub title: String,
    pub description: Option<String>,
    pub narrative_form: Option<String>,
    pub genre: Option<String>,
    pub setting: Option<String>,
}

/// 更新小说请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateNovelRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub narrative_form: Option<String>,
    pub genre: Option<String>,
    pub setting: Option<String>,
}

/// 小说列表查询请求
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ListNovelsRequest {
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub sort_by: Option<String>,
}

/// 设置创作体验请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetCreationExperienceRequest {
    pub experience: String,
}

// ============================================================================
// 章节请求 DTO
// ============================================================================

/// 创建章节请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateChapterRequest {
    pub title: String,
    pub sort_order: i32,
}

/// 保存章节请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveChapterRequest {
    pub title: String,
    pub content: String,
    pub expected_version: i64,
}

/// 提升章节候选版本请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromoteChapterRequest {
    pub candidate_id: String,
    pub revision_id: String,
}

// ============================================================================
// 运行请求 DTO
// ============================================================================

/// 启动运行请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartRunRequest {
    pub command_id: String,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub token_budget: Option<u64>,
    pub max_automatic_retries: Option<u8>,
    pub prompt_versions: Option<BTreeMap<String, String>>,
}

/// 批准运行请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApproveRunRequest {
    pub command_id: String,
}

// ============================================================================
// 验证 trait
// ============================================================================

pub trait ValidateRequest {
    fn validate(&self) -> Result<(), String>;
}

impl ValidateRequest for CreateNovelRequest {
    fn validate(&self) -> Result<(), String> {
        if self.title.trim().is_empty() {
            return Err("Title cannot be empty".to_string());
        }
        if self.title.len() > 200 {
            return Err("Title cannot exceed 200 characters".to_string());
        }
        if let Some(desc) = &self.description {
            if desc.len() > 2000 {
                return Err("Description cannot exceed 2000 characters".to_string());
            }
        }
        Ok(())
    }
}

impl ValidateRequest for UpdateNovelRequest {
    fn validate(&self) -> Result<(), String> {
        if let Some(title) = &self.title {
            if title.trim().is_empty() {
                return Err("Title cannot be empty".to_string());
            }
            if title.len() > 200 {
                return Err("Title cannot exceed 200 characters".to_string());
            }
        }
        if let Some(desc) = &self.description {
            if desc.len() > 2000 {
                return Err("Description cannot exceed 2000 characters".to_string());
            }
        }
        Ok(())
    }
}

impl ValidateRequest for CreateChapterRequest {
    fn validate(&self) -> Result<(), String> {
        if self.title.trim().is_empty() {
            return Err("Chapter title cannot be empty".to_string());
        }
        if self.title.len() > 200 {
            return Err("Chapter title cannot exceed 200 characters".to_string());
        }
        if self.sort_order < 0 {
            return Err("Sort order must be non-negative".to_string());
        }
        Ok(())
    }
}

impl ValidateRequest for SaveChapterRequest {
    fn validate(&self) -> Result<(), String> {
        if self.title.trim().is_empty() {
            return Err("Chapter title cannot be empty".to_string());
        }
        if self.content.trim().is_empty() {
            return Err("Chapter content cannot be empty".to_string());
        }
        if self.content.len() > 100_000 {
            return Err("Chapter content is too large (max 100KB)".to_string());
        }
        Ok(())
    }
}

impl ValidateRequest for StartRunRequest {
    fn validate(&self) -> Result<(), String> {
        if self.command_id.trim().is_empty() {
            return Err("Command ID cannot be empty".to_string());
        }
        Ok(())
    }
}

impl ValidateRequest for ApproveRunRequest {
    fn validate(&self) -> Result<(), String> {
        if self.command_id.trim().is_empty() {
            return Err("Command ID cannot be empty".to_string());
        }
        Ok(())
    }
}

impl ValidateRequest for PromoteChapterRequest {
    fn validate(&self) -> Result<(), String> {
        if self.candidate_id.trim().is_empty() {
            return Err("Candidate ID cannot be empty".to_string());
        }
        if self.revision_id.trim().is_empty() {
            return Err("Revision ID cannot be empty".to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_create_novel_empty_title() {
        let req = CreateNovelRequest {
            title: "".to_string(),
            description: None,
            narrative_form: None,
            genre: None,
            setting: None,
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_validate_create_novel_title_too_long() {
        let req = CreateNovelRequest {
            title: "a".repeat(201),
            description: None,
            narrative_form: None,
            genre: None,
            setting: None,
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_validate_create_novel_valid() {
        let req = CreateNovelRequest {
            title: "Test Novel".to_string(),
            description: Some("A test description".to_string()),
            narrative_form: None,
            genre: None,
            setting: None,
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_validate_create_chapter_empty_title() {
        let req = CreateChapterRequest {
            title: "".to_string(),
            sort_order: 0,
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_validate_create_chapter_negative_sort_order() {
        let req = CreateChapterRequest {
            title: "Chapter 1".to_string(),
            sort_order: -1,
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_validate_save_chapter_empty_content() {
        let req = SaveChapterRequest {
            title: "Chapter 1".to_string(),
            content: "".to_string(),
            expected_version: 1,
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_validate_save_chapter_content_too_large() {
        let req = SaveChapterRequest {
            title: "Chapter 1".to_string(),
            content: "a".repeat(100_001),
            expected_version: 1,
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_validate_start_run_empty_command_id() {
        let req = StartRunRequest {
            command_id: "".to_string(),
            provider: None,
            model: None,
            token_budget: None,
            max_automatic_retries: None,
            prompt_versions: None,
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_validate_promote_chapter_valid() {
        let req = PromoteChapterRequest {
            candidate_id: "cand1".to_string(),
            revision_id: "rev1".to_string(),
        };
        assert!(req.validate().is_ok());
    }
}
