use std::collections::BTreeMap;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::director::{DirectorService, OpenRunInput};
use crate::error::{AppError, PortError};
use crate::hash::json_hash;
use crate::ports::Ids;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchRunRequest {
    pub provider: String,
    pub model: String,
    pub prompt_versions: BTreeMap<String, String>,
    pub token_budget: Option<u64>,
    pub context_token_budget: Option<u64>,
    pub max_automatic_retries: u8,
    pub repair_budget: Option<u8>,
    pub endpoint: String,
    pub allow_localhost_http: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BatchItemView {
    pub id: String,
    pub chapter_id: String,
    pub ordinal: i32,
    pub run_id: Option<String>,
    pub status: String,
    pub version: i64,
    pub last_error: Option<String>,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BatchRunView {
    pub id: String,
    pub novel_id: String,
    pub command_id: String,
    pub request_hash: String,
    pub status: String,
    pub cursor: i32,
    pub version: i64,
    pub claim_epoch: i64,
    pub failure_reason: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub items: Vec<BatchItemView>,
}

#[derive(Debug, Clone)]
pub struct NewBatchRun {
    pub id: String,
    pub novel_id: String,
    pub command_id: String,
    pub request_hash: String,
    pub request_json: String,
    pub items: Vec<(String, String, i32)>,
    pub now_ms: i64,
}

#[derive(Debug, Clone)]
pub struct BatchFence {
    pub owner: String,
    pub claim_epoch: i64,
    pub version: i64,
}

#[async_trait]
pub trait BatchRepository: Send + Sync {
    async fn create_batch(&self, record: NewBatchRun) -> Result<BatchRunView, PortError>;
    async fn get_batch(&self, id: &str) -> Result<BatchRunView, PortError>;
    async fn get_batch_request_json(&self, id: &str) -> Result<String, PortError>;
    async fn list_batches(&self, novel_id: &str) -> Result<Vec<BatchRunView>, PortError>;
    async fn list_runnable_batch_ids(&self) -> Result<Vec<String>, PortError>;
    async fn claim_batch(
        &self,
        id: &str,
        owner: &str,
        lease_until: i64,
        now_ms: i64,
    ) -> Result<Option<BatchRunView>, PortError>;
    async fn update_batch(
        &self,
        fence: &BatchFence,
        status: &str,
        cursor: i32,
        failure: Option<&str>,
        now_ms: i64,
    ) -> Result<BatchRunView, PortError>;
    async fn bind_item_run(
        &self,
        fence: &BatchFence,
        item_id: &str,
        run_id: &str,
        now_ms: i64,
    ) -> Result<BatchRunView, PortError>;
    async fn update_item(
        &self,
        fence: &BatchFence,
        item_id: &str,
        status: &str,
        error: Option<&str>,
        now_ms: i64,
    ) -> Result<BatchRunView, PortError>;
    async fn set_batch_status(
        &self,
        id: &str,
        expected_version: i64,
        status: &str,
        now_ms: i64,
    ) -> Result<BatchRunView, PortError>;
}

pub struct BatchService {
    repository: std::sync::Arc<dyn BatchRepository>,
    ids: std::sync::Arc<dyn Ids>,
}

impl BatchService {
    pub fn new(
        repository: std::sync::Arc<dyn BatchRepository>,
        ids: std::sync::Arc<dyn Ids>,
    ) -> Self {
        Self { repository, ids }
    }

    pub async fn create(
        &self,
        novel_id: &str,
        chapter_ids: &[String],
        request: BatchRunRequest,
        command_id: &str,
        now_ms: i64,
    ) -> Result<BatchRunView, AppError> {
        if chapter_ids.len() < 2 {
            return Err(AppError::validation("批量创作至少需要两个章节。"));
        }
        if chapter_ids.len() > 50 {
            return Err(AppError::validation("一次批量创作最多支持 50 个章节。"));
        }
        let mut seen = std::collections::HashSet::new();
        for chapter_id in chapter_ids {
            if chapter_id.trim().is_empty() || !seen.insert(chapter_id.clone()) {
                return Err(AppError::validation("批量章节列表包含重复或空章节。"));
            }
        }
        let request_hash = json_hash(&(
            novel_id,
            chapter_ids,
            &request.provider,
            &request.model,
            &request.prompt_versions,
            request.token_budget,
            request.context_token_budget,
            request.max_automatic_retries,
            request.repair_budget,
            &request.endpoint,
            request.allow_localhost_http,
        ))?;
        let items = chapter_ids
            .iter()
            .enumerate()
            .map(|(index, chapter_id)| (self.ids.new_id(), chapter_id.clone(), index as i32))
            .collect();
        Ok(self
            .repository
            .create_batch(NewBatchRun {
                id: self.ids.new_id(),
                novel_id: novel_id.to_string(),
                command_id: command_id.to_string(),
                request_hash,
                request_json: serde_json::to_string(&request)
                    .map_err(|_| AppError::integrity("批量请求无法保存。"))?,
                items,
                now_ms,
            })
            .await?)
    }

    pub async fn list(&self, novel_id: &str) -> Result<Vec<BatchRunView>, AppError> {
        Ok(self.repository.list_batches(novel_id).await?)
    }

    pub async fn get(&self, id: &str) -> Result<BatchRunView, AppError> {
        Ok(self.repository.get_batch(id).await?)
    }

    pub async fn pause(
        &self,
        id: &str,
        expected_version: i64,
        now_ms: i64,
    ) -> Result<BatchRunView, AppError> {
        self.set_status_retry(id, expected_version, "paused", now_ms)
            .await
    }

    pub async fn resume(
        &self,
        id: &str,
        expected_version: i64,
        now_ms: i64,
    ) -> Result<BatchRunView, AppError> {
        let current = self.repository.get_batch(id).await?;
        if current.status == "cancelled"
            || current.status == "completed"
            || current.status == "failed"
        {
            return Err(AppError::conflict("这个批量任务已经结束，不能继续。"));
        }
        self.set_status_retry(id, expected_version, "running", now_ms)
            .await
    }

    pub async fn cancel(
        &self,
        director: &DirectorService,
        id: &str,
        expected_version: i64,
        now_ms: i64,
    ) -> Result<BatchRunView, AppError> {
        // Close the scheduling boundary before cancelling an attached Run.
        let cancelled = self
            .set_status_retry(id, expected_version, "cancelled", now_ms)
            .await?;
        let current = self.repository.get_batch(id).await?;
        let current_item = current.items.get(current.cursor.max(0) as usize);
        if let Some(run_id) = current_item.and_then(|item| item.run_id.as_deref()) {
            let _ = director
                .cancel(&format!("batch:{id}:cancel-current-run"), run_id)
                .await;
        } else if let Some(item) = current_item {
            // An open_run may have committed just before cancellation.
            let command = format!("batch:{}:item:{}:open", id, item.id);
            if let Some(run_id) = director.run_id_for_command(&command).await?
                && !current
                    .items
                    .iter()
                    .any(|entry| entry.run_id.as_deref() == Some(run_id.as_str()))
            {
                let _ = director
                    .cancel(&format!("batch:{id}:cancel-orphan"), &run_id)
                    .await;
            }
        }
        Ok(cancelled)
    }

    async fn set_status_retry(
        &self,
        id: &str,
        expected_version: i64,
        status: &str,
        now_ms: i64,
    ) -> Result<BatchRunView, AppError> {
        let mut version = expected_version;
        for _ in 0..4 {
            match self
                .repository
                .set_batch_status(id, version, status, now_ms)
                .await
            {
                Ok(view) => return Ok(view),
                Err(PortError::Conflict(_)) => {
                    version = self.repository.get_batch(id).await?.version;
                }
                Err(error) => return Err(error.into()),
            }
        }
        Err(AppError::conflict("批量任务持续变化，请刷新后再试。"))
    }

    pub async fn tick(
        &self,
        director: &DirectorService,
        owner: &str,
        now_ms: i64,
    ) -> Result<Vec<BatchRunView>, AppError> {
        let mut changed = Vec::new();
        for id in self.repository.list_runnable_batch_ids().await? {
            let Some(batch) = self
                .repository
                .claim_batch(&id, owner, now_ms + 30_000, now_ms)
                .await?
            else {
                continue;
            };
            if let Some(view) = self.advance(director, batch, owner, now_ms).await? {
                changed.push(view);
            }
        }
        Ok(changed)
    }

    async fn advance(
        &self,
        director: &DirectorService,
        batch: BatchRunView,
        owner: &str,
        now_ms: i64,
    ) -> Result<Option<BatchRunView>, AppError> {
        if matches!(
            batch.status.as_str(),
            "paused" | "cancelled" | "completed" | "failed"
        ) {
            return Ok(None);
        }
        let fence = BatchFence {
            owner: owner.to_string(),
            claim_epoch: batch.claim_epoch,
            version: batch.version,
        };
        // Note: claim epoch is stored in version only for this minimal scheduler; the
        // repository additionally checks lease_owner and lease_until.
        let request: BatchRunRequest =
            serde_json::from_str(&self.repository.get_batch_request_json(&batch.id).await?)
                .map_err(|_| AppError::integrity("批量请求无法恢复。"))?;
        let cursor = batch.cursor.max(0) as usize;
        let Some(item) = batch.items.get(cursor) else {
            return Ok(Some(
                self.repository
                    .update_batch(&fence, "completed", batch.items.len() as i32, None, now_ms)
                    .await?,
            ));
        };
        if item.status == "succeeded" {
            let next = (cursor + 1) as i32;
            let status = if next as usize >= batch.items.len() {
                "completed"
            } else {
                "running"
            };
            return Ok(Some(
                self.repository
                    .update_batch(&fence, status, next, None, now_ms)
                    .await?,
            ));
        }
        if let Some(run_id) = &item.run_id {
            let detail = director.get_run_detail(&batch.novel_id, run_id).await?;
            match detail.status.as_str() {
                "succeeded" => {
                    let chapter = director
                        .get_chapter(&batch.novel_id, &item.chapter_id)
                        .await?;
                    if chapter.official_version < 1 || chapter.content.trim().is_empty() {
                        self.repository
                            .update_item(
                                &fence,
                                &item.id,
                                "failed",
                                Some("章节没有形成正式正文。"),
                                now_ms,
                            )
                            .await?;
                        return Ok(Some(
                            self.repository
                                .update_batch(
                                    &fence,
                                    "failed",
                                    batch.cursor,
                                    Some("章节没有形成正式正文。"),
                                    now_ms,
                                )
                                .await?,
                        ));
                    }
                    self.repository
                        .update_item(&fence, &item.id, "succeeded", None, now_ms)
                        .await?;
                    let next = (cursor + 1) as i32;
                    let status = if next as usize >= batch.items.len() {
                        "completed"
                    } else {
                        "running"
                    };
                    return Ok(Some(
                        self.repository
                            .update_batch(&fence, status, next, None, now_ms)
                            .await?,
                    ));
                }
                "waiting_approval" => {
                    return Ok(Some(
                        self.repository
                            .update_batch(&fence, "waiting_approval", batch.cursor, None, now_ms)
                            .await?,
                    ));
                }
                "paused" => {
                    return Ok(Some(
                        self.repository
                            .update_batch(&fence, "paused", batch.cursor, None, now_ms)
                            .await?,
                    ));
                }
                "failed" | "cancelled" => {
                    self.repository
                        .update_item(
                            &fence,
                            &item.id,
                            if detail.status == "cancelled" {
                                "cancelled"
                            } else {
                                "failed"
                            },
                            Some("当前章节创作失败。"),
                            now_ms,
                        )
                        .await?;
                    return Ok(Some(
                        self.repository
                            .update_batch(
                                &fence,
                                "failed",
                                batch.cursor,
                                Some("当前章节创作失败。"),
                                now_ms,
                            )
                            .await?,
                    ));
                }
                _ => {
                    return Ok(Some(
                        self.repository
                            .update_batch(&fence, "running", batch.cursor, None, now_ms)
                            .await?,
                    ));
                }
            }
        }
        // A chapter can only start after the prior item committed.
        if cursor > 0 && batch.items[cursor - 1].status != "succeeded" {
            return Ok(Some(
                self.repository
                    .update_batch(&fence, "waiting_approval", batch.cursor, None, now_ms)
                    .await?,
            ));
        }
        let input = OpenRunInput {
            command_id: format!("batch:{}:item:{}:open", batch.id, item.id),
            novel_id: batch.novel_id.clone(),
            target_chapter_id: item.chapter_id.clone(),
            provider: request.provider,
            model: request.model,
            prompt_versions: request.prompt_versions,
            token_budget: request.token_budget,
            context_token_budget: request.context_token_budget,
            max_automatic_retries: request.max_automatic_retries,
            repair_budget: request.repair_budget,
            endpoint: request.endpoint,
            allow_localhost_http: request.allow_localhost_http,
            protocol: None,
            credential_ref: None,
        };
        let outcome = director.open_run(input).await?;
        #[cfg(debug_assertions)]
        if let Some(dir) = std::env::var_os("BIZ_NOVEL_TEST_BATCH_BARRIER_DIR") {
            let dir = std::path::PathBuf::from(dir);
            let _ = std::fs::create_dir_all(&dir);
            let marker = dir.join(format!("open-{}-{}.json", batch.id, item.id));
            let _ = std::fs::write(
                &marker,
                serde_json::json!({
                    "batchId": batch.id,
                    "itemId": item.id,
                    "runId": outcome.run_id,
                    "commandId": format!("batch:{}:item:{}:open", batch.id, item.id),
                })
                .to_string(),
            );
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        }
        match self
            .repository
            .bind_item_run(&fence, &item.id, &outcome.run_id, now_ms)
            .await
        {
            Ok(updated) => Ok(Some(updated)),
            Err(PortError::Conflict(_)) => {
                // A lost fence never proves the Run is orphaned: a newer Scheduler may
                // legally adopt the same deterministic Run. Only a cancelled Batch
                // with no persisted binding may be cleaned up.
                let current = self.repository.get_batch(&batch.id).await?;
                let bound = current
                    .items
                    .iter()
                    .any(|entry| entry.run_id.as_deref() == Some(outcome.run_id.as_str()));
                if current.status == "cancelled" && !bound {
                    let _ = director
                        .cancel(
                            &format!("batch:{}:cancelled:{}", batch.id, outcome.run_id),
                            &outcome.run_id,
                        )
                        .await;
                }
                Ok(None)
            }
            Err(error) => Err(error.into()),
        }
    }
}
