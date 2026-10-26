use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::PortError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StructuredFactView {
    pub id: String,
    pub novel_id: String,
    pub entity_id: String,
    pub fact_type: String,
    pub value_json: String,
    pub source_chapter_id: String,
    pub source_revision: i64,
    pub evidence_quote: String,
    pub truthfulness: String,
    pub effective_chapter_order: i64,
    pub status: String,
    pub superseded_by: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewStructuredFact {
    pub id: String,
    pub novel_id: String,
    pub entity_id: String,
    pub fact_type: String,
    pub value_json: String,
    pub source_chapter_id: String,
    pub source_revision: i64,
    pub evidence_quote: String,
    pub effective_chapter_order: i64,
    pub now_ms: i64,
}

#[async_trait]
pub trait StructuredMemoryRepository: Send + Sync {
    async fn list_structured_facts(
        &self,
        novel_id: &str,
        entity_id: Option<&str>,
        as_of_chapter_order: Option<i64>,
    ) -> Result<Vec<StructuredFactView>, PortError>;
    async fn confirm_structured_fact(
        &self,
        fact: NewStructuredFact,
    ) -> Result<StructuredFactView, PortError>;
    async fn supersede_structured_fact(
        &self,
        novel_id: &str,
        fact_id: &str,
        replacement_id: &str,
        now_ms: i64,
    ) -> Result<(), PortError>;
}

pub struct StructuredMemoryService {
    repository: std::sync::Arc<dyn StructuredMemoryRepository>,
}

impl StructuredMemoryService {
    pub fn new(repository: std::sync::Arc<dyn StructuredMemoryRepository>) -> Self {
        Self { repository }
    }

    pub async fn list(
        &self,
        novel_id: &str,
        entity_id: Option<String>,
        as_of_chapter_order: Option<i64>,
    ) -> Result<Vec<StructuredFactView>, crate::error::AppError> {
        Ok(self
            .repository
            .list_structured_facts(novel_id, entity_id.as_deref(), as_of_chapter_order)
            .await?)
    }

    pub async fn confirm(
        &self,
        fact: NewStructuredFact,
    ) -> Result<StructuredFactView, crate::error::AppError> {
        match fact.fact_type.as_str() {
            "location" | "life_state" | "alias" | "relationship" => {}
            _ => return Err(crate::error::AppError::validation("不支持的事实类型。")),
        }
        if fact.evidence_quote.trim().is_empty() {
            return Err(crate::error::AppError::validation("请填写来源证据。"));
        }
        Ok(self.repository.confirm_structured_fact(fact).await?)
    }
}
