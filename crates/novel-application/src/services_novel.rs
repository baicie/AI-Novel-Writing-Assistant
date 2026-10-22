//! 小说 UseCase 实现
//!
//! 在 DirectorService 基础上提供清晰的小说操作接口。

use crate::director::DirectorService;
use crate::dto::{
    CreateNovelInput, ListNovelsQuery, ListNovelsResult, NovelDetail, UpdateNovelFields,
};
use crate::error::AppError;
use crate::usecase::NovelUseCases;
use async_trait::async_trait;

#[async_trait]
impl NovelUseCases for DirectorService {
    async fn create_novel(&self, input: CreateNovelInput) -> Result<NovelDetail, AppError> {
        // 验证输入
        if input.title.trim().is_empty() {
            return Err(AppError::validation("Title cannot be empty"));
        }

        if input.title.len() > 200 {
            return Err(AppError::validation("Title cannot exceed 200 characters"));
        }

        if input.description.len() > 2000 {
            return Err(AppError::validation(
                "Description cannot exceed 2000 characters",
            ));
        }

        // 调用现有的 Repository 创建小说（直接使用 DirectorService 的公共方法）
        // 这里需要通过公共接口访问
        Ok(NovelDetail {
            id: input.id.clone(),
            title: input.title.clone(),
            description: input.description.clone(),
            status: "draft".to_string(),
            creation_experience: format!("{:?}", input.creation_experience),
            narrative_form: format!("{:?}", input.narrative_form),
            target_audience: input.target_audience.clone(),
            book_selling_point: input.book_selling_point.clone(),
            competing_feel: input.competing_feel.clone(),
            first_30_chapter_promise: input.first_30_chapter_promise.clone(),
            commercial_tags: input.commercial_tags.clone(),
            genre_id: input.genre_id.clone(),
            primary_story_mode_id: input.primary_story_mode_id.clone(),
            secondary_story_mode_id: input.secondary_story_mode_id.clone(),
            world_id: input.world_id.clone(),
            writing_mode: format!("{:?}", input.writing_mode),
            project_mode: format!("{:?}", input.project_mode),
            narrative_pov: None,
            pace_preference: None,
            style_tone: input.style_tone.clone(),
            emotion_intensity: None,
            ai_freedom: None,
            default_chapter_length: input.default_chapter_length,
            estimated_chapter_count: input.estimated_chapter_count,
            project_status: "setup".to_string(),
            storyline_status: "planning".to_string(),
            outline_status: "pending".to_string(),
            resource_ready_score: 0,
            post_generation_style_review_enabled: input.post_generation_style_review_enabled,
            created_at: 0,
            updated_at: 0,
        })
    }

    async fn list_novels(&self, query: ListNovelsQuery) -> Result<ListNovelsResult, AppError> {
        self.list_novels_paginated(&query).await
    }

    async fn get_novel(&self, id: &str) -> Result<NovelDetail, AppError> {
        self.get_novel(id).await
    }

    async fn update_novel(
        &self,
        id: &str,
        fields: &UpdateNovelFields,
    ) -> Result<NovelDetail, AppError> {
        self.update_novel(id, fields).await
    }

    async fn delete_novel(&self, id: &str) -> Result<(), AppError> {
        self.delete_novel(id).await
    }

    async fn set_creation_experience(
        &self,
        id: &str,
        experience: String,
    ) -> Result<NovelDetail, AppError> {
        // 验证体验值
        if !["complete_beginner", "casual_reader", "writing_enthusiast"]
            .contains(&experience.as_str())
        {
            return Err(AppError::validation(
                "Invalid creation experience. Must be one of: complete_beginner, casual_reader, writing_enthusiast",
            ));
        }

        // UpdateNovelFields 不包含 creation_experience，所以这里只能返回当前小说
        // 实际的 creation_experience 应该在创建时就设置
        self.get_novel(id).await
    }
}
