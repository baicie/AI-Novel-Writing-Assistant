//! 运行 UseCase 实现
//! 
//! 在现有 DirectorService 基础上提供清晰的运行操作接口。

use crate::director::DirectorService;
use crate::dto::RunSummary;
use crate::error::AppError;
use crate::usecase::RunUseCases;
use async_trait::async_trait;

#[async_trait]
impl RunUseCases for DirectorService {
    async fn start_run(
        &self,
        novel_id: &str,
        command_id: &str,
    ) -> Result<RunSummary, AppError> {
        // 验证输入
        if novel_id.is_empty() {
            return Err(AppError::validation("Novel ID cannot be empty"));
        }

        if command_id.is_empty() {
            return Err(AppError::validation("Command ID cannot be empty"));
        }

        // 占位符实现
        // 实际实现需要调用 WorkflowRepository 来创建运行
        Err(AppError::validation("Start run not implemented yet"))
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

        // 占位符实现
        Err(AppError::validation("Approve run command not implemented yet"))
    }
}

#[cfg(test)]
mod tests {
    use crate::error::AppError;
    use super::*;

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
}
