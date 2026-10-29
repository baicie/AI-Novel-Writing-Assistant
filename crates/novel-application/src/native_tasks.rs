use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::PortError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NativeTaskSummary {
    pub task_id: String,
    pub task_type: String,
    pub novel_id: String,
    pub chapter_id: Option<String>,
    pub batch_id: Option<String>,
    pub title: String,
    pub status: String,
    pub current_stage: Option<String>,
    pub progress: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_error: Option<String>,
    pub requires_user_action: bool,
    pub available_actions: Vec<String>,
    pub detail_route: String,
}

#[derive(Debug, Clone, Default)]
pub struct NativeTaskQuery {
    pub novel_id: Option<String>,
    pub task_type: Option<String>,
    pub status: Option<String>,
    pub limit: i64,
    pub offset: i64,
}

#[async_trait]
pub trait NativeTaskRepository: Send + Sync {
    async fn list_native_tasks(
        &self,
        query: &NativeTaskQuery,
    ) -> Result<Vec<NativeTaskSummary>, PortError>;
}

pub struct NativeTaskService {
    repository: std::sync::Arc<dyn NativeTaskRepository>,
}

impl NativeTaskService {
    pub fn new(repository: std::sync::Arc<dyn NativeTaskRepository>) -> Self {
        Self { repository }
    }

    pub async fn list(
        &self,
        query: NativeTaskQuery,
    ) -> Result<Vec<NativeTaskSummary>, crate::error::AppError> {
        Ok(self.repository.list_native_tasks(&query).await?)
    }
}
