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
    pub version: i64,
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

/// 带来源定位的结构化事实：用于连续性投影判断「来源修订是否仍然有效」。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StructuredFactWithSource {
    pub fact: StructuredFactView,
    /// 来源章节被删除时为 None：界面必须显示 unavailable，不能补一个假来源。
    pub source_chapter_order: Option<i64>,
    pub source_chapter_title: Option<String>,
    /// 来源章节当前的正式版本；与 `fact.source_revision` 不同即说明事实已过期。
    pub source_current_version: Option<i64>,
    /// 旧证据是否仍存在于当前正式正文；不存在时必须重新选取证据。
    pub evidence_present_in_current: bool,
    /// 来源章节是否晚于事实生效章节；为真时事实不能作为已知事实。
    pub source_after_effective: bool,
}

#[async_trait]
pub trait StructuredMemoryRepository: Send + Sync {
    async fn list_structured_facts(
        &self,
        novel_id: &str,
        entity_id: Option<&str>,
        as_of_chapter_order: Option<i64>,
    ) -> Result<Vec<StructuredFactView>, PortError>;
    /// 截至该章的结构化事实（含 proposed/disputed，供冲突双方展示）。
    /// 调用方只能把 confirmed 且来源修订仍有效的条目放进有效事实。
    async fn list_structured_facts_with_source(
        &self,
        novel_id: &str,
        as_of_chapter_order: i64,
        limit: i64,
    ) -> Result<Vec<StructuredFactWithSource>, PortError>;
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

fn relationship_value(fact: &StructuredFactView) -> (String, String, bool) {
    let value = parse_value(&fact.value_json);
    let target = value
        .get("targetEntityId")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let relation = value
        .get("relation")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let active = value
        .get("active")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    (target, relation, active)
}

fn parse_value(raw: &str) -> serde_json::Value {
    serde_json::from_str(raw).unwrap_or_else(|_| serde_json::Value::String(raw.to_string()))
}

/// As-Of 可见事实的纯选择规则：单值类型保留最新一条，关系按有向键保留最新一条，
/// 别名可以共存。仓储与连续性投影共用这一份规则，避免两套口径。
pub fn visible_facts_at(mut rows: Vec<StructuredFactView>) -> Vec<StructuredFactView> {
    rows.sort_by(|left, right| {
        right
            .effective_chapter_order
            .cmp(&left.effective_chapter_order)
            .then_with(|| right.updated_at.cmp(&left.updated_at))
            .then_with(|| right.created_at.cmp(&left.created_at))
            .then_with(|| right.id.cmp(&left.id))
    });
    let mut seen_single: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut seen_relation: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut out = Vec::new();
    for fact in rows {
        match fact.fact_type.as_str() {
            "location" | "life_state" => {
                let key = format!("{}:{}", fact.entity_id, fact.fact_type);
                if seen_single.insert(key) {
                    out.push(fact);
                }
            }
            "alias" => out.push(fact),
            "relationship" => {
                let (target, relation, active) = relationship_value(&fact);
                let key = format!("{}:{}:{}", fact.entity_id, target, relation);
                if seen_relation.insert(key) && active {
                    out.push(fact);
                }
            }
            _ => {}
        }
    }
    out
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
