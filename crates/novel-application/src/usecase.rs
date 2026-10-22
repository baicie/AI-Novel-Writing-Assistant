//! 应用层 UseCase 抽象层
//!
//! 这层提供清晰的业务操作接口，连接 HTTP 路由和 Repository 实现。
//! 每个 UseCase 都：
//! - 验证输入
//! - 执行业务逻辑
//! - 转换结果为 DTO
//! - 映射错误

use crate::dto::{
    ChapterSnapshot, CreateNovelInput, ListNovelsQuery, ListNovelsResult, NovelDetail, RunSummary,
    UpdateNovelFields,
};
use crate::error::AppError;
use crate::ports::{PromoteOutcome as DomainPromoteOutcome, SaveChapterRecord};
use async_trait::async_trait;

/// 小说操作 UseCase 接口
#[async_trait]
pub trait NovelUseCases: Send + Sync {
    /// 创建小说
    async fn create_novel(&self, input: CreateNovelInput) -> Result<NovelDetail, AppError>;

    /// 列出小说（分页）
    async fn list_novels(&self, query: ListNovelsQuery) -> Result<ListNovelsResult, AppError>;

    /// 获取小说详情
    async fn get_novel(&self, id: &str) -> Result<NovelDetail, AppError>;

    /// 更新小说
    async fn update_novel(
        &self,
        id: &str,
        fields: &UpdateNovelFields,
    ) -> Result<NovelDetail, AppError>;

    /// 删除小说
    async fn delete_novel(&self, id: &str) -> Result<(), AppError>;

    /// 设置创作体验（onboarding）
    async fn set_creation_experience(
        &self,
        id: &str,
        experience: String,
    ) -> Result<NovelDetail, AppError>;
}

/// 章节操作 UseCase 接口
#[async_trait]
pub trait ChapterUseCases: Send + Sync {
    /// 创建章节
    async fn create_chapter(
        &self,
        novel_id: &str,
        title: &str,
        sort_order: i32,
    ) -> Result<ChapterSnapshot, AppError>;

    /// 列出章节
    async fn list_chapters(&self, novel_id: &str) -> Result<Vec<ChapterSnapshot>, AppError>;

    /// 获取章节
    async fn get_chapter(
        &self,
        novel_id: &str,
        chapter_id: &str,
    ) -> Result<ChapterSnapshot, AppError>;

    /// 保存章节内容
    async fn save_chapter(&self, input: SaveChapterRecord) -> Result<ChapterSnapshot, AppError>;

    /// 提升候选为正式版本
    async fn promote_chapter(
        &self,
        chapter_id: &str,
        candidate_id: &str,
        revision_id: &str,
    ) -> Result<PromoteOutcomeDto, AppError>;
}

/// 运行操作 UseCase 接口
#[async_trait]
pub trait RunUseCases: Send + Sync {
    /// 开启运行
    async fn start_run(&self, novel_id: &str, command_id: &str) -> Result<RunSummary, AppError>;

    /// 批准运行命令
    async fn approve_run_command(
        &self,
        run_id: &str,
        command_id: &str,
    ) -> Result<RunSummary, AppError>;
}

/// 提升结果 DTO
#[derive(Debug, Clone)]
pub enum PromoteOutcomeDto {
    Promoted { version: i64 },
    Blocked,
    MissingCandidate,
}

impl From<DomainPromoteOutcome> for PromoteOutcomeDto {
    fn from(outcome: DomainPromoteOutcome) -> Self {
        match outcome {
            DomainPromoteOutcome::Promoted { version } => Self::Promoted { version },
            DomainPromoteOutcome::Blocked => Self::Blocked,
            DomainPromoteOutcome::MissingCandidate => Self::MissingCandidate,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_promote_outcome_conversion() {
        let domain_outcome = DomainPromoteOutcome::Promoted { version: 42 };

        let dto_outcome: PromoteOutcomeDto = domain_outcome.into();

        match dto_outcome {
            PromoteOutcomeDto::Promoted { version } => {
                assert_eq!(version, 42);
            }
            _ => panic!("Expected Promoted outcome"),
        }
    }
}
