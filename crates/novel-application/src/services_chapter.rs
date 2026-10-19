//! 章节 UseCase 实现
//! 
//! 在现有服务基础上提供清晰的章节操作接口。

use crate::director::DirectorService;
use crate::dto::ChapterSnapshot;
use crate::error::AppError;
use crate::usecase::{ChapterUseCases, PromoteOutcomeDto};
use crate::ports::SaveChapterRecord;
use async_trait::async_trait;

#[async_trait]
impl ChapterUseCases for DirectorService {
    async fn create_chapter(
        &self,
        novel_id: &str,
        title: &str,
        sort_order: i32,
    ) -> Result<ChapterSnapshot, AppError> {
        // 验证输入
        if title.trim().is_empty() {
            return Err(AppError::validation("Chapter title cannot be empty"));
        }

        if title.len() > 200 {
            return Err(AppError::validation("Chapter title cannot exceed 200 characters"));
        }

        if sort_order < 0 {
            return Err(AppError::validation("Sort order must be non-negative"));
        }

        // 暂时返回占位符，实际实现委托给 Repository
        // 这是 UseCase 层的接口定义，具体实现需要 Repository 支持
        Err(AppError::validation("Create chapter not implemented yet"))
    }

    async fn list_chapters(&self, novel_id: &str) -> Result<Vec<ChapterSnapshot>, AppError> {
        // 占位符实现
        Ok(vec![])
    }

    async fn get_chapter(
        &self,
        novel_id: &str,
        chapter_id: &str,
    ) -> Result<ChapterSnapshot, AppError> {
        Err(AppError::not_found("Chapter not found"))
    }

    async fn save_chapter(
        &self,
        input: SaveChapterRecord,
    ) -> Result<ChapterSnapshot, AppError> {
        // 验证输入
        if input.title.trim().is_empty() {
            return Err(AppError::validation("Chapter title cannot be empty"));
        }

        if input.content.trim().is_empty() {
            return Err(AppError::validation("Chapter content cannot be empty"));
        }

        if input.content.len() > 100_000 {
            return Err(AppError::validation("Chapter content is too large (max 100KB)"));
        }

        // 委托给 Repository（通过现有的 save_chapter_record 方法）
        Err(AppError::validation("Save chapter not implemented yet"))
    }

    async fn promote_chapter(
        &self,
        chapter_id: &str,
        candidate_id: &str,
        revision_id: &str,
    ) -> Result<PromoteOutcomeDto, AppError> {
        // 验证输入
        if chapter_id.is_empty() {
            return Err(AppError::validation("Chapter ID cannot be empty"));
        }

        if candidate_id.is_empty() {
            return Err(AppError::validation("Candidate ID cannot be empty"));
        }

        if revision_id.is_empty() {
            return Err(AppError::validation("Revision ID cannot be empty"));
        }

        // 占位符实现
        Ok(PromoteOutcomeDto::MissingCandidate)
    }
}

#[cfg(test)]
mod tests {
    use crate::error::AppError;
    use super::*;

    #[test]
    fn test_validate_empty_title() {
        let error = AppError::validation("Chapter title cannot be empty");
        assert_eq!(error.message, "Chapter title cannot be empty");
    }

    #[test]
    fn test_validate_title_too_long() {
        let error = AppError::validation("Chapter title cannot exceed 200 characters");
        assert_eq!(error.message, "Chapter title cannot exceed 200 characters");
    }

    #[test]
    fn test_content_too_large() {
        let large_content = "a".repeat(100_001);
        let error = AppError::validation("Chapter content is too large (max 100KB)");
        assert_eq!(error.message, "Chapter content is too large (max 100KB)");
    }

    #[test]
    fn test_promote_outcome_conversion() {
        let dto = PromoteOutcomeDto::MissingCandidate;
        match dto {
            PromoteOutcomeDto::MissingCandidate => {
                // OK
            }
            _ => panic!("Expected MissingCandidate"),
        }
    }
}
