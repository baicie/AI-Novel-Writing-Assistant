//! 阶段 2 的 setup 服务：世界观 CRUD、查表、onboarding、quick-setup。
//!
//! 放在 `novel-application` 下是为了让 HTTP 层能直接通过 `Runtime` 拿到这些用例。
//! `DirectorService` 保持聚焦在运行生命周期，这里专门管 setup 阶段。

use std::sync::Arc;

use novel_domain::{FirstNovelOnboarding, Genre, QuickSetupStatus, StoryMode, World};

use crate::error::AppError;
use crate::ports::{
    Clock, Ids, LookupRepository, ModelSecrets, NewWorld, UpdateWorldFields, WorldRepository,
};

pub struct SetupService {
    worlds: Arc<dyn WorldRepository>,
    lookups: Arc<dyn LookupRepository>,
    ids: Arc<dyn Ids>,
    clock: Arc<dyn Clock>,
    secrets: Arc<dyn ModelSecrets>,
}

impl SetupService {
    pub fn new(
        worlds: Arc<dyn WorldRepository>,
        lookups: Arc<dyn LookupRepository>,
        ids: Arc<dyn Ids>,
        clock: Arc<dyn Clock>,
        secrets: Arc<dyn ModelSecrets>,
    ) -> Self {
        Self {
            worlds,
            lookups,
            ids,
            clock,
            secrets,
        }
    }

    pub async fn list_worlds(&self) -> Result<Vec<World>, AppError> {
        Ok(self.worlds.list_worlds().await?)
    }

    pub async fn get_world(&self, id: &str) -> Result<World, AppError> {
        self.worlds
            .get_world(id)
            .await?
            .ok_or_else(|| AppError::not_found("没有找到这个世界观。"))
    }

    pub async fn create_world(
        &self,
        name: &str,
        description: &str,
        overview_summary: &str,
        source_template_id: Option<String>,
        owner_novel_id: Option<String>,
    ) -> Result<World, AppError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(AppError::validation("请填写世界观名称。"));
        }
        Ok(self
            .worlds
            .create_world(NewWorld {
                id: self.ids.new_id(),
                name: name.to_string(),
                description: description.trim().to_string(),
                overview_summary: overview_summary.trim().to_string(),
                source_template_id,
                owner_novel_id,
                now_ms: self.clock.now_ms(),
            })
            .await?)
    }

    pub async fn update_world(
        &self,
        id: &str,
        fields: &UpdateWorldFields,
    ) -> Result<World, AppError> {
        Ok(self
            .worlds
            .update_world(id, fields, self.clock.now_ms())
            .await?)
    }

    pub async fn delete_world(&self, id: &str) -> Result<(), AppError> {
        Ok(self.worlds.delete_world(id).await?)
    }

    pub async fn list_genres(&self) -> Result<Vec<Genre>, AppError> {
        Ok(self.lookups.list_genres().await?)
    }

    pub async fn list_story_modes(&self) -> Result<Vec<StoryMode>, AppError> {
        Ok(self.lookups.list_story_modes().await?)
    }

    pub async fn get_first_novel_onboarding(&self) -> Result<FirstNovelOnboarding, AppError> {
        Ok(self.lookups.get_first_novel_onboarding().await?)
    }

    /// quick-setup 状态：从 setup 的模型配置和密钥存储里聚合。
    ///
    /// 这一步只读 `provider_credential` + 系统密钥存储，不做修改。前端的快速上手面板
    /// 用它来显示"是否已经配好模型"。
    pub async fn get_quick_setup_status(&self) -> Result<QuickSetupStatus, AppError> {
        // 阶段 1 的 model_setup 直接挂在 `SqliteKernel` 上，但 SetupService 不持有 SqliteKernel
        // （保持只通过 port 协作）。这里通过 ModelSecrets 探一下默认 provider 的 key 是否存在，
        // 真正的模型 endpoint / model 由前端从 `get_model_setup` 单独拿。
        let has_api_key = self.secrets.api_key("openai-compatible").await?.is_some();
        let ready = has_api_key;
        let next_step = if ready {
            "可以创建第一本书并开始创作。"
        } else {
            "先在设置里填好模型地址和 API Key。"
        };
        Ok(QuickSetupStatus {
            ready_for_creation: ready,
            model_configured: has_api_key,
            api_key_present: has_api_key,
            endpoint: None,
            model: None,
            allow_localhost_http: false,
            next_step: next_step.to_string(),
        })
    }
}
