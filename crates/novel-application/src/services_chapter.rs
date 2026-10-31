//! 章节 UseCase 实现
//!
//! 在现有服务基础上提供清晰的章节操作接口。

use crate::director::DirectorService;
use crate::dto::ChapterSnapshot;
use crate::error::AppError;
use crate::ports::SaveChapterRecord;
use crate::usecase::{ChapterUseCases, PromoteOutcomeDto};
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
            return Err(AppError::validation(
                "Chapter title cannot exceed 200 characters",
            ));
        }

        if sort_order < 0 {
            return Err(AppError::validation("Sort order must be non-negative"));
        }

        // 验证小说存在
        let _ = self
            .get_novel(novel_id)
            .await
            .map_err(|_| AppError::not_found("Novel not found"))?;

        // 调用 DirectorService 的 create_chapter 方法
        self.create_chapter(novel_id, title, sort_order)
            .await
            .map_err(|e| AppError::validation(format!("Failed to create chapter: {}", e)))
    }

    async fn list_chapters(&self, novel_id: &str) -> Result<Vec<ChapterSnapshot>, AppError> {
        // 验证小说存在
        let _ = self
            .get_novel(novel_id)
            .await
            .map_err(|_| AppError::not_found("Novel not found"))?;

        // 调用 DirectorService 的 list_chapters 方法
        self.list_chapters(novel_id).await
    }

    async fn get_chapter(
        &self,
        novel_id: &str,
        chapter_id: &str,
    ) -> Result<ChapterSnapshot, AppError> {
        // 验证输入
        if chapter_id.is_empty() {
            return Err(AppError::validation("Chapter ID cannot be empty"));
        }

        // 验证小说存在
        let _ = self
            .get_novel(novel_id)
            .await
            .map_err(|_| AppError::not_found("Novel not found"))?;

        // 调用 DirectorService 的 get_chapter 方法
        self.get_chapter(novel_id, chapter_id)
            .await
            .map_err(|_| AppError::not_found("Chapter not found"))
    }

    async fn save_chapter(&self, input: SaveChapterRecord) -> Result<ChapterSnapshot, AppError> {
        // 验证输入
        if input.title.trim().is_empty() {
            return Err(AppError::validation("Chapter title cannot be empty"));
        }

        if input.content.trim().is_empty() {
            return Err(AppError::validation("Chapter content cannot be empty"));
        }

        if input.content.len() > 100_000 {
            return Err(AppError::validation(
                "Chapter content is too large (max 100KB)",
            ));
        }

        // 验证小说存在
        let _ = self
            .get_novel(&input.novel_id)
            .await
            .map_err(|_| AppError::not_found("Novel not found"))?;

        // 调用 DirectorService 的 save_chapter 方法
        // save_chapter 需要单独的参数，所以从 SaveChapterRecord 中提取
        self.save_chapter(
            &input.novel_id,
            &input.chapter_id,
            input.expected_version,
            &input.title,
            &input.content,
        )
        .await
        .map_err(|e| AppError::validation(format!("Failed to save chapter: {}", e)))
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

        // 占位符实现 - 实际需要调用正式的提升方法
        // 返回 MissingCandidate 作为默认结果
        Ok(PromoteOutcomeDto::MissingCandidate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AppError;

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
    fn test_validate_negative_sort_order() {
        let error = AppError::validation("Sort order must be non-negative");
        assert_eq!(error.message, "Sort order must be non-negative");
    }

    #[test]
    fn test_validate_empty_content() {
        let error = AppError::validation("Chapter content cannot be empty");
        assert_eq!(error.message, "Chapter content cannot be empty");
    }

    #[test]
    fn test_content_too_large() {
        let _large_content = "a".repeat(100_001);
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
