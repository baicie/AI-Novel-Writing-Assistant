//! 章节连续性投影：只读地把「截至目标章节已经确认的事实」整理给作者。
//!
//! 这里不新建记忆引擎：事实、候选、冲突全部来自既有仓储与既有 As-Of 查询；
//! 这一层只做来源校验、稳定排序与只读投影。任何来源无法确认时保留 unavailable/stale，
//! 不补造事实，也不把候选当成已确认。

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::dto::CharacterSnapshot;
use crate::error::AppError;
use crate::ports::{
    ChapterHeader, MemoryCandidateView, MemoryConflictView, MemoryRepository, NovelRepository,
    StoryFactView,
};
use crate::structured_memory::{
    StructuredFactWithSource, StructuredMemoryRepository, visible_facts_at,
};

pub const CONTINUITY_CANDIDATE_LIMIT: i64 = 200;
pub const CONTINUITY_CONFLICT_LIMIT: i64 = 100;
pub const CONTINUITY_STRUCTURED_FACT_LIMIT: i64 = 500;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuityWorld {
    pub id: String,
    pub name: String,
    pub description: String,
    pub overview_summary: String,
    pub status: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuityCharacterState {
    pub body: String,
    pub chapter_order: Option<i32>,
    pub source_chapter_id: Option<String>,
    pub evidence_quote: Option<String>,
    pub source_revision: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuityCharacter {
    pub id: String,
    pub name: String,
    pub role: String,
    pub current_state: Option<ContinuityCharacterState>,
    pub latest_change: Option<String>,
    pub pending_candidate_count: usize,
    pub fact_ids: Vec<String>,
}

/// 一条可审计的事实。`status` 只会是 valid / stale / unavailable。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuityFact {
    pub id: String,
    /// structured = 结构化人物事实；story = 角色状态/时间线/伏笔。
    pub kind: String,
    pub entity_id: Option<String>,
    pub entity_name: Option<String>,
    pub fact_type: String,
    pub value: String,
    pub value_json: Option<String>,
    pub truthfulness: String,
    pub effective_chapter_order: i32,
    pub source_chapter_id: Option<String>,
    pub source_chapter_order: Option<i32>,
    pub source_chapter_title: Option<String>,
    pub source_revision: Option<i64>,
    pub source_current_version: Option<i64>,
    pub evidence_quote: Option<String>,
    /// 旧证据是否仍存在于来源章节当前正式正文；不存在时必须重新选取。
    pub evidence_still_present: bool,
    /// 来源章节是否不晚于事实生效章节。
    pub source_order_valid: bool,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuityCandidate {
    pub id: String,
    pub chapter_id: String,
    pub chapter_order: Option<i32>,
    pub chapter_title: Option<String>,
    pub chapter_version: i64,
    pub current_chapter_version: Option<i64>,
    pub fact_type: String,
    pub truthfulness: String,
    pub entity_name: Option<String>,
    pub entity_id: Option<String>,
    pub value: String,
    pub evidence_quote: String,
    pub status: String,
    pub ambiguous: bool,
    pub version: i64,
    pub expired: bool,
    pub can_confirm: bool,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuityConflictSide {
    pub kind: String,
    pub id: String,
    pub value: Option<String>,
    pub source_chapter_id: Option<String>,
    pub source_chapter_order: Option<i32>,
    pub source_chapter_title: Option<String>,
    pub evidence_quote: Option<String>,
    pub status: String,
    pub available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuityConflict {
    pub id: String,
    pub conflict_type: String,
    pub entity_key: String,
    pub chapter_id: String,
    pub chapter_order: i32,
    pub status: String,
    pub version: i64,
    pub description: String,
    pub candidate_ids: Vec<String>,
    pub fact_ids: Vec<String>,
    pub sides: Vec<ContinuityConflictSide>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuityLimits {
    pub structured_fact_limit: i64,
    pub facts_truncated: bool,
    pub candidate_limit: i64,
    pub candidates_truncated: bool,
    pub conflict_limit: i64,
    pub conflicts_truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterContinuityProjection {
    pub novel_id: String,
    pub chapter_id: String,
    pub chapter_order: i32,
    pub chapter_version: i64,
    pub worlds: Vec<ContinuityWorld>,
    pub characters: Vec<ContinuityCharacter>,
    pub facts: Vec<ContinuityFact>,
    pub stale_facts: Vec<ContinuityFact>,
    pub candidates: Vec<ContinuityCandidate>,
    pub conflicts: Vec<ContinuityConflict>,
    pub limits: ContinuityLimits,
}

pub struct ChapterContinuityService {
    novels: Arc<dyn NovelRepository>,
    structured: Arc<dyn StructuredMemoryRepository>,
    memory: Arc<dyn MemoryRepository>,
}

impl ChapterContinuityService {
    pub fn new(
        novels: Arc<dyn NovelRepository>,
        structured: Arc<dyn StructuredMemoryRepository>,
        memory: Arc<dyn MemoryRepository>,
    ) -> Self {
        Self {
            novels,
            structured,
            memory,
        }
    }

    pub async fn project(
        &self,
        novel_id: &str,
        chapter_id: &str,
    ) -> Result<ChapterContinuityProjection, AppError> {
        let target = self.novels.get_chapter(novel_id, chapter_id).await?;
        let headers = self.novels.list_chapter_headers(novel_id).await?;
        let header_by_id: std::collections::HashMap<&str, &ChapterHeader> = headers
            .iter()
            .map(|header| (header.id.as_str(), header))
            .collect();
        let target_header = header_by_id
            .get(target.id.as_str())
            .copied()
            .ok_or_else(|| AppError::integrity("目标章节不在本书的章节目录里。"))?;
        let as_of = target.sort_order;

        let characters = self.novels.list_characters(novel_id).await?;
        let character_names: std::collections::HashMap<&str, &str> = characters
            .iter()
            .map(|character| (character.id.as_str(), character.name.as_str()))
            .collect();
        let worlds = self.novels.list_worlds_for_context(novel_id).await?;
        let story_facts = self.novels.list_story_facts(novel_id, as_of).await?;
        let structured = self
            .structured
            .list_structured_facts_with_source(
                novel_id,
                i64::from(as_of),
                CONTINUITY_STRUCTURED_FACT_LIMIT.saturating_add(1),
            )
            .await?;
        let candidates = self
            .memory
            .list_continuity_candidates(
                novel_id,
                as_of,
                CONTINUITY_CANDIDATE_LIMIT.saturating_add(1),
            )
            .await?;
        let conflicts = self
            .memory
            .list_continuity_conflicts(novel_id, as_of, CONTINUITY_CONFLICT_LIMIT.saturating_add(1))
            .await?;

        let facts_truncated = structured.len() > CONTINUITY_STRUCTURED_FACT_LIMIT as usize;
        let structured = structured
            .into_iter()
            .take(CONTINUITY_STRUCTURED_FACT_LIMIT as usize)
            .collect::<Vec<_>>();
        let (valid_structured, stale_structured, conflict_fact_pool) =
            split_structured_by_source(structured, &header_by_id, &character_names);
        let visible_ids: std::collections::HashSet<String> = visible_facts_at(
            valid_structured
                .iter()
                .map(|item| item.fact.clone())
                .collect(),
        )
        .into_iter()
        .map(|fact| fact.id)
        .collect();

        let story = story_facts
            .iter()
            .map(|fact| story_fact_to_continuity(fact, &header_by_id))
            .collect::<Vec<_>>();
        let mut facts = valid_structured
            .iter()
            .filter(|item| visible_ids.contains(&item.fact.id))
            .map(|item| {
                structured_fact_to_continuity(item, &header_by_id, &character_names, "valid")
            })
            .chain(story.iter().cloned())
            .collect::<Vec<_>>();
        sort_facts(&mut facts);
        let mut stale_facts = stale_structured
            .iter()
            .map(|item| {
                structured_fact_to_continuity(
                    item,
                    &header_by_id,
                    &character_names,
                    structured_fact_status(item),
                )
            })
            .collect::<Vec<_>>();
        sort_facts(&mut stale_facts);
        let conflict_fact_lookup: Vec<ContinuityFact> = conflict_fact_pool
            .into_iter()
            .chain(facts.iter().cloned())
            .chain(stale_facts.iter().cloned())
            .collect();

        let candidates_truncated = candidates.len() > CONTINUITY_CANDIDATE_LIMIT as usize;
        let conflicts_truncated = conflicts.len() > CONTINUITY_CONFLICT_LIMIT as usize;
        let candidates = candidates
            .into_iter()
            .take(CONTINUITY_CANDIDATE_LIMIT as usize)
            .map(|candidate| candidate_to_continuity(candidate, &header_by_id))
            .collect::<Vec<_>>();
        let conflicts = conflicts
            .into_iter()
            .take(CONTINUITY_CONFLICT_LIMIT as usize)
            .map(|conflict| conflict_to_continuity(conflict, &conflict_fact_lookup, &candidates))
            .collect::<Vec<_>>();

        let characters = build_characters(characters, &story_facts, &facts, &candidates);

        Ok(ChapterContinuityProjection {
            novel_id: target.novel_id.clone(),
            chapter_id: target.id.clone(),
            chapter_order: target_header.sort_order,
            chapter_version: target_header.official_version,
            worlds: worlds
                .into_iter()
                .map(|world| ContinuityWorld {
                    id: world.id,
                    name: world.name,
                    description: world.description,
                    overview_summary: world.overview_summary,
                    status: world.status,
                    updated_at: world.updated_at,
                })
                .collect(),
            characters,
            facts,
            stale_facts,
            candidates,
            conflicts,
            limits: ContinuityLimits {
                structured_fact_limit: CONTINUITY_STRUCTURED_FACT_LIMIT,
                facts_truncated,
                candidate_limit: CONTINUITY_CANDIDATE_LIMIT,
                candidates_truncated,
                conflict_limit: CONTINUITY_CONFLICT_LIMIT,
                conflicts_truncated,
            },
        })
    }
}

/// 返回（有效已确认事实、来源失效的已确认事实、用于冲突展示的全部事实）。
/// proposed/disputed 只进入冲突展示池，绝不进入有效事实列表。
fn split_structured_by_source(
    rows: Vec<StructuredFactWithSource>,
    headers: &std::collections::HashMap<&str, &ChapterHeader>,
    character_names: &std::collections::HashMap<&str, &str>,
) -> (
    Vec<StructuredFactWithSource>,
    Vec<StructuredFactWithSource>,
    Vec<ContinuityFact>,
) {
    let mut valid = Vec::new();
    let mut stale = Vec::new();
    let mut pool = Vec::new();
    for row in rows {
        let header = headers.get(row.fact.source_chapter_id.as_str()).copied();
        let confirmed = row.fact.truthfulness == "confirmed";
        let status = if !confirmed {
            "disputed"
        } else if header.map(|item| item.official_version) == Some(row.fact.source_revision)
            && !row.source_after_effective
        {
            "valid"
        } else {
            structured_fact_status(&row)
        };
        pool.push(structured_fact_to_continuity(
            &row,
            headers,
            character_names,
            status,
        ));
        if confirmed {
            if status == "valid" {
                valid.push(row);
            } else if row.fact.superseded_by.is_none() {
                // 已经被新事实替代的旧版本不再提示重新确认；否则确认一次后提示永远不会消失。
                stale.push(row);
            }
        }
    }
    (valid, stale, pool)
}

fn structured_fact_status(item: &StructuredFactWithSource) -> &'static str {
    if item.source_chapter_order.is_none() || item.source_current_version.is_none() {
        "unavailable"
    } else if item.source_after_effective {
        "invalid"
    } else {
        "stale"
    }
}

fn structured_fact_to_continuity(
    item: &StructuredFactWithSource,
    headers: &std::collections::HashMap<&str, &ChapterHeader>,
    character_names: &std::collections::HashMap<&str, &str>,
    status: &str,
) -> ContinuityFact {
    let fact = &item.fact;
    let header = headers.get(fact.source_chapter_id.as_str()).copied();
    ContinuityFact {
        id: fact.id.clone(),
        kind: "structured".into(),
        entity_id: Some(fact.entity_id.clone()),
        entity_name: character_names
            .get(fact.entity_id.as_str())
            .map(|name| (*name).to_string()),
        fact_type: fact.fact_type.clone(),
        value: fact.value_json.clone(),
        value_json: Some(fact.value_json.clone()),
        truthfulness: fact.truthfulness.clone(),
        effective_chapter_order: fact.effective_chapter_order as i32,
        source_chapter_id: Some(fact.source_chapter_id.clone()),
        source_chapter_order: header.map(|item| item.sort_order),
        source_chapter_title: header.map(|item| item.title.clone()),
        source_revision: Some(fact.source_revision),
        source_current_version: header.map(|item| item.official_version),
        evidence_quote: Some(fact.evidence_quote.clone()),
        evidence_still_present: item.evidence_present_in_current,
        source_order_valid: !item.source_after_effective,
        status: status.into(),
    }
}

fn story_fact_to_continuity(
    fact: &StoryFactView,
    headers: &std::collections::HashMap<&str, &ChapterHeader>,
) -> ContinuityFact {
    let header = fact
        .source_chapter_id
        .as_deref()
        .and_then(|id| headers.get(id).copied());
    ContinuityFact {
        id: fact.id.clone(),
        kind: "story".into(),
        entity_id: fact.character_id.clone(),
        entity_name: fact.character_name.clone(),
        fact_type: fact.fact_type.clone(),
        value: fact.body.clone(),
        value_json: None,
        truthfulness: "confirmed".into(),
        effective_chapter_order: fact.chapter_order,
        source_chapter_id: fact.source_chapter_id.clone(),
        source_chapter_order: header.map(|item| item.sort_order),
        source_chapter_title: header.map(|item| item.title.clone()),
        source_revision: None,
        source_current_version: header.map(|item| item.official_version),
        evidence_quote: fact.evidence_quote.clone(),
        evidence_still_present: fact.evidence_quote.is_some(),
        source_order_valid: header.is_some_and(|item| item.sort_order <= fact.chapter_order),
        // 没有来源章节的旧事实仍可展示内容，但不能冒充可追溯的有效证据。
        status: if header.is_some() {
            "valid"
        } else {
            "unavailable"
        }
        .into(),
    }
}

fn fact_sort_key(fact: &ContinuityFact) -> (std::cmp::Reverse<i32>, u8, String, String, String) {
    let kind_rank = match fact.fact_type.as_str() {
        "life_state" => 0,
        "location" => 1,
        "relationship" => 2,
        "alias" => 3,
        "character_state" => 4,
        "timeline_event" => 5,
        "plot_thread" => 6,
        _ => 7,
    };
    (
        std::cmp::Reverse(fact.effective_chapter_order),
        kind_rank,
        fact.entity_name.clone().unwrap_or_default(),
        fact.fact_type.clone(),
        fact.id.clone(),
    )
}

fn sort_facts(facts: &mut [ContinuityFact]) {
    facts.sort_by_key(fact_sort_key);
}

fn candidate_to_continuity(
    candidate: MemoryCandidateView,
    headers: &std::collections::HashMap<&str, &ChapterHeader>,
) -> ContinuityCandidate {
    let header = headers.get(candidate.chapter_id.as_str()).copied();
    let expired = header.is_none_or(|item| item.official_version != candidate.chapter_version);
    let can_confirm = !(expired
        || candidate.ambiguous
        || (candidate.fact_type == "character_state" && candidate.entity_id.is_none())
        || !matches!(candidate.status.as_str(), "proposed" | "deferred"));
    ContinuityCandidate {
        id: candidate.id,
        chapter_id: candidate.chapter_id,
        chapter_order: header.map(|item| item.sort_order),
        chapter_title: header.map(|item| item.title.clone()),
        chapter_version: candidate.chapter_version,
        current_chapter_version: header.map(|item| item.official_version),
        fact_type: candidate.fact_type,
        truthfulness: candidate.truthfulness,
        entity_name: candidate.entity_name,
        entity_id: candidate.entity_id,
        value: candidate.value,
        evidence_quote: candidate.evidence_quote,
        status: candidate.status,
        ambiguous: candidate.ambiguous,
        version: candidate.version,
        expired,
        can_confirm,
        created_at: candidate.created_at,
    }
}

fn conflict_to_continuity(
    conflict: MemoryConflictView,
    facts: &[ContinuityFact],
    candidates: &[ContinuityCandidate],
) -> ContinuityConflict {
    let fact_by_id: std::collections::HashMap<&str, &ContinuityFact> =
        facts.iter().map(|fact| (fact.id.as_str(), fact)).collect();
    let candidate_by_id: std::collections::HashMap<&str, &ContinuityCandidate> = candidates
        .iter()
        .map(|candidate| (candidate.id.as_str(), candidate))
        .collect();
    let mut sides = Vec::new();
    for fact_id in &conflict.fact_ids {
        let side = fact_by_id.get(fact_id.as_str()).map_or_else(
            || ContinuityConflictSide {
                kind: "fact".into(),
                id: fact_id.clone(),
                value: None,
                source_chapter_id: None,
                source_chapter_order: None,
                source_chapter_title: None,
                evidence_quote: None,
                status: "unavailable".into(),
                available: false,
            },
            |fact| ContinuityConflictSide {
                kind: "fact".into(),
                id: fact.id.clone(),
                value: Some(fact.value.clone()),
                source_chapter_id: fact.source_chapter_id.clone(),
                source_chapter_order: fact.source_chapter_order,
                source_chapter_title: fact.source_chapter_title.clone(),
                evidence_quote: fact.evidence_quote.clone(),
                status: fact.status.clone(),
                available: fact.status == "valid",
            },
        );
        sides.push(side);
    }
    for candidate_id in &conflict.candidate_ids {
        let side = candidate_by_id.get(candidate_id.as_str()).map_or_else(
            || ContinuityConflictSide {
                kind: "candidate".into(),
                id: candidate_id.clone(),
                value: None,
                source_chapter_id: None,
                source_chapter_order: None,
                source_chapter_title: None,
                evidence_quote: None,
                status: "unavailable".into(),
                available: false,
            },
            |candidate| ContinuityConflictSide {
                kind: "candidate".into(),
                id: candidate.id.clone(),
                value: Some(candidate.value.clone()),
                source_chapter_id: Some(candidate.chapter_id.clone()),
                source_chapter_order: candidate.chapter_order,
                source_chapter_title: candidate.chapter_title.clone(),
                evidence_quote: Some(candidate.evidence_quote.clone()),
                status: candidate.status.clone(),
                available: candidate.can_confirm,
            },
        );
        sides.push(side);
    }
    ContinuityConflict {
        id: conflict.id,
        conflict_type: conflict.conflict_type,
        entity_key: conflict.entity_key,
        chapter_id: conflict.chapter_id,
        chapter_order: conflict.chapter_order,
        status: conflict.status,
        version: conflict.version,
        description: conflict.description,
        candidate_ids: conflict.candidate_ids,
        fact_ids: conflict.fact_ids,
        sides,
    }
}

fn build_characters(
    mut characters: Vec<CharacterSnapshot>,
    story_facts: &[StoryFactView],
    facts: &[ContinuityFact],
    candidates: &[ContinuityCandidate],
) -> Vec<ContinuityCharacter> {
    characters.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.id.cmp(&right.id))
    });
    let mut state_by_character: std::collections::HashMap<&str, &StoryFactView> =
        std::collections::HashMap::new();
    for fact in story_facts {
        if fact.fact_type == "character_state"
            && let Some(character_id) = fact.character_id.as_deref()
        {
            state_by_character.insert(character_id, fact);
        }
    }
    characters
        .into_iter()
        .map(|character| {
            let state = state_by_character.get(character.id.as_str()).copied();
            let pending_candidate_count = candidates
                .iter()
                .filter(|candidate| {
                    candidate.entity_id.as_deref() == Some(character.id.as_str())
                        || candidate.entity_name.as_deref() == Some(character.name.as_str())
                })
                .count();
            let fact_ids = facts
                .iter()
                .filter(|fact| fact.entity_id.as_deref() == Some(character.id.as_str()))
                .map(|fact| fact.id.clone())
                .collect();
            let structured_state = state.is_none().then(|| {
                let mut parts = Vec::new();
                let mut source: Option<&ContinuityFact> = None;
                for fact_type in ["life_state", "location"] {
                    if let Some(fact) = facts.iter().rfind(|fact| {
                        fact.kind == "structured"
                            && fact.entity_id.as_deref() == Some(character.id.as_str())
                            && fact.fact_type == fact_type
                    }) {
                        let text = if fact_type == "life_state" {
                            let value: serde_json::Value =
                                serde_json::from_str(&fact.value).unwrap_or_default();
                            match value.get("state").and_then(|state| state.as_str()) {
                                Some("alive") => "存活".to_string(),
                                Some("dead") => "死亡".to_string(),
                                _ => fact.value.clone(),
                            }
                        } else {
                            let value: serde_json::Value =
                                serde_json::from_str(&fact.value).unwrap_or_default();
                            format!(
                                "位于 {}",
                                value
                                    .get("location")
                                    .and_then(|location| location.as_str())
                                    .unwrap_or(fact.value.as_str())
                            )
                        };
                        parts.push(text);
                        source = Some(fact);
                    }
                }
                source.map(|fact| ContinuityCharacterState {
                    body: parts.join(" · "),
                    chapter_order: fact.source_chapter_order,
                    source_chapter_id: fact.source_chapter_id.clone(),
                    evidence_quote: fact.evidence_quote.clone(),
                    source_revision: None,
                })
            });
            let current_state = state
                .map(|fact| ContinuityCharacterState {
                    body: fact.body.clone(),
                    chapter_order: Some(fact.chapter_order),
                    source_chapter_id: fact.source_chapter_id.clone(),
                    evidence_quote: fact.evidence_quote.clone(),
                    source_revision: fact.source_revision.clone(),
                })
                .or(structured_state.flatten());
            ContinuityCharacter {
                id: character.id,
                name: character.name,
                role: character.role,
                latest_change: current_state.as_ref().map(|state| state.body.clone()),
                current_state,
                pending_candidate_count,
                fact_ids,
            }
        })
        .collect()
}
