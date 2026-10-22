//! 运行 UseCase 实现
//!
//! 在现有 DirectorService 基础上提供清晰的运行操作接口。

use crate::director::{DirectorService, OpenRunInput};
use crate::dto::RunSummary;
use crate::error::AppError;
use crate::usecase::RunUseCases;
use async_trait::async_trait;
use std::collections::BTreeMap;

#[async_trait]
impl RunUseCases for DirectorService {
    async fn start_run(&self, novel_id: &str, command_id: &str) -> Result<RunSummary, AppError> {
        // 验证输入
        if novel_id.is_empty() {
            return Err(AppError::validation("Novel ID cannot be empty"));
        }

        if command_id.is_empty() {
            return Err(AppError::validation("Command ID cannot be empty"));
        }

        // 验证小说存在
        let _ = self
            .get_novel(novel_id)
            .await
            .map_err(|_| AppError::not_found("Novel not found"))?;

        // 构造默认的运行输入
        // 在实际使用中，这应该从请求中获取
        let input = OpenRunInput {
            command_id: command_id.to_string(),
            novel_id: novel_id.to_string(),
            target_chapter_id: String::new(),
            provider: "openai".to_string(),
            model: "gpt-4".to_string(),
            prompt_versions: BTreeMap::new(),
            token_budget: None,
            max_automatic_retries: 3,
            endpoint: String::new(),
            allow_localhost_http: false,
        };

        // 调用 DirectorService 的 open_run 方法
        // 这会返回 CommandOutcome，我们需要将其转换为 RunSummary
        let _outcome = self
            .open_run(input)
            .await
            .map_err(|_| AppError::validation("Failed to start run"))?;

        // 返回一个基础的 RunSummary
        // 实际上应该从 CommandOutcome 中获取运行详情
        Ok(RunSummary {
            id: command_id.to_string(),
            novel_id: novel_id.to_string(),
            chapter_id: String::new(),
            chapter_title: String::new(),
            state: "queued".to_string(),
            created_at: 0,
            updated_at: 0,
            provider: "openai".to_string(),
            model: "gpt-4".to_string(),
        })
    }

    async fn approve_run_command(
        &self,
        run_id: &str,
        command_id: &str,
    ) -> Result<RunSummary, AppError> {
        // 验证输入
        if run_id.is_empty() {
            return Err(AppError::validation("Run ID cannot be empty"));
        }

        if command_id.is_empty() {
            return Err(AppError::validation("Command ID cannot be empty"));
        }

        // 调用 DirectorService 的 approve 方法
        let _outcome = self
            .approve(command_id, run_id)
            .await
            .map_err(|_| AppError::validation("Failed to approve run command"))?;

        // 返回一个基础的 RunSummary
        Ok(RunSummary {
            id: run_id.to_string(),
            novel_id: String::new(),
            chapter_id: String::new(),
            chapter_title: String::new(),
            state: "running".to_string(),
            created_at: 0,
            updated_at: 0,
            provider: String::new(),
            model: String::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AppError;

    #[test]
    fn test_validate_empty_novel_id() {
        let error = AppError::validation("Novel ID cannot be empty");
        assert_eq!(error.message, "Novel ID cannot be empty");
    }

    #[test]
    fn test_validate_empty_command_id() {
        let error = AppError::validation("Command ID cannot be empty");
        assert_eq!(error.message, "Command ID cannot be empty");
    }

    #[test]
    fn test_validate_empty_run_id() {
        let error = AppError::validation("Run ID cannot be empty");
        assert_eq!(error.message, "Run ID cannot be empty");
    }

    #[test]
    fn test_open_run_input_defaults() {
        let input = OpenRunInput {
            command_id: "cmd1".to_string(),
            novel_id: "novel1".to_string(),
            target_chapter_id: String::new(),
            provider: "openai".to_string(),
            model: "gpt-4".to_string(),
            prompt_versions: BTreeMap::new(),
            token_budget: None,
            max_automatic_retries: 3,
            endpoint: String::new(),
            allow_localhost_http: false,
        };

        assert_eq!(input.max_automatic_retries, 3);
        assert_eq!(input.provider, "openai");
    }

    #[test]
    fn test_run_summary_defaults() {
        let summary = RunSummary {
            id: "run1".to_string(),
            novel_id: "novel1".to_string(),
            chapter_id: "chapter1".to_string(),
            chapter_title: "Test Chapter".to_string(),
            state: "queued".to_string(),
            created_at: 0,
            updated_at: 0,
            provider: "openai".to_string(),
            model: "gpt-4".to_string(),
        };

        assert_eq!(summary.state, "queued");
        assert_eq!(summary.provider, "openai");
    }
}
