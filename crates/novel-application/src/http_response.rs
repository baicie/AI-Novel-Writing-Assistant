//! HTTP 层 - 响应 DTO
//!
//! 定义发送给客户端的响应数据结构。

use serde::{Deserialize, Serialize};

// ============================================================================
// 小说响应 DTO
// ============================================================================

/// 小说详情响应
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NovelDetailResponse {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub narrative_form: String,
    pub genre: String,
    pub setting: String,
    pub chapter_count: i32,
}

/// 小说摘要响应
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NovelSummaryResponse {
    pub id: String,
    pub title: String,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub chapter_count: i32,
}

/// 小说列表响应
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListNovelsResponse {
    pub items: Vec<NovelSummaryResponse>,
    pub total: i64,
    pub page: i64,
    pub limit: i64,
}

// ============================================================================
// 章节响应 DTO
// ============================================================================

/// 章节快照响应
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterSnapshotResponse {
    pub id: String,
    pub novel_id: String,
    pub title: String,
    pub sort_order: i32,
    pub state: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// 章节详情响应
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterDetailResponse {
    pub id: String,
    pub novel_id: String,
    pub title: String,
    pub sort_order: i32,
    pub content: String,
    pub state: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// 章节列表响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListChaptersResponse {
    pub items: Vec<ChapterSnapshotResponse>,
    pub total: i64,
}

/// 提升结果响应
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PromoteOutcomeResponse {
    #[serde(rename = "accepted")]
    Accepted,
    #[serde(rename = "rejected")]
    Rejected { reason: String },
    #[serde(rename = "missing_candidate")]
    MissingCandidate,
}

// ============================================================================
// 运行响应 DTO
// ============================================================================

/// 运行摘要响应
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunSummaryResponse {
    pub id: String,
    pub novel_id: String,
    pub chapter_id: String,
    pub chapter_title: String,
    pub state: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub provider: String,
    pub model: String,
}

/// 运行列表响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListRunsResponse {
    pub items: Vec<RunSummaryResponse>,
    pub total: i64,
}

// ============================================================================
// 错误响应 DTO
// ============================================================================

/// HTTP 错误响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub code: String,
    pub message: String,
    pub details: Option<String>,
}

impl ErrorResponse {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }
}

// ============================================================================
// 转换 trait
// ============================================================================

pub trait ToResponse {
    type Response;
    fn to_response(self) -> Self::Response;
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_response_creation() {
        let error = ErrorResponse::new("VALIDATION", "Invalid input");
        assert_eq!(error.code, "VALIDATION");
        assert_eq!(error.message, "Invalid input");
        assert!(error.details.is_none());
    }

    #[test]
    fn test_error_response_with_details() {
        let error =
            ErrorResponse::new("NOT_FOUND", "Novel not found").with_details("Novel ID: 123");
        assert_eq!(error.code, "NOT_FOUND");
        assert_eq!(error.message, "Novel not found");
        assert_eq!(error.details, Some("Novel ID: 123".to_string()));
    }

    #[test]
    fn test_novel_summary_response_serialization() {
        let response = NovelSummaryResponse {
            id: "novel1".to_string(),
            title: "Test Novel".to_string(),
            status: "draft".to_string(),
            created_at: 0,
            updated_at: 0,
            chapter_count: 5,
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("novel1"));
        assert!(json.contains("Test Novel"));
    }

    #[test]
    fn test_chapter_snapshot_response_serialization() {
        let response = ChapterSnapshotResponse {
            id: "ch1".to_string(),
            novel_id: "novel1".to_string(),
            title: "Chapter 1".to_string(),
            sort_order: 1,
            state: "draft".to_string(),
            created_at: 0,
            updated_at: 0,
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("ch1"));
        assert!(json.contains("Chapter 1"));
    }

    #[test]
    fn test_run_summary_response_serialization() {
        let response = RunSummaryResponse {
            id: "run1".to_string(),
            novel_id: "novel1".to_string(),
            chapter_id: "ch1".to_string(),
            chapter_title: "Chapter 1".to_string(),
            state: "queued".to_string(),
            created_at: 0,
            updated_at: 0,
            provider: "openai".to_string(),
            model: "gpt-4".to_string(),
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("run1"));
        assert!(json.contains("queued"));
    }

    #[test]
    fn test_promote_outcome_response_accepted() {
        let response = PromoteOutcomeResponse::Accepted;
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("accepted"));
    }

    #[test]
    fn test_promote_outcome_response_rejected() {
        let response = PromoteOutcomeResponse::Rejected {
            reason: "Quality too low".to_string(),
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("rejected"));
        assert!(json.contains("Quality too low"));
    }

    #[test]
    fn test_promote_outcome_response_missing_candidate() {
        let response = PromoteOutcomeResponse::MissingCandidate;
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("missing_candidate"));
    }
}
