//! Chapter obligations are structured planning constraints that must travel from
//! the planner artifact into the writer, reviewer, and repair prompts.
//!
//! The contract is intentionally narrow: it carries event/participant/causality
//! constraints and their provenance. It does not attempt to decide whether prose
//! is semantically correct; that remains a reviewer/model decision. These helpers
//! only normalize, render, and check references deterministically.

use std::collections::BTreeSet;

use serde_json::{Value, json};

pub(crate) const OBLIGATION_TYPES: [&str; 6] = [
    "required_event",
    "forbidden_event",
    "participant",
    "causal_constraint",
    "ordering_constraint",
    "continuity_constraint",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChapterObligation {
    pub id: String,
    pub chapter_order: i64,
    pub kind: String,
    pub description: String,
    pub enforcement: String,
    pub participants: Vec<String>,
    pub source: String,
    pub evidence: Option<String>,
}

/// Parse already-normalized obligations from a target chapter plan. Older plans
/// without the field produce an empty list and remain readable.
pub(crate) fn parse_obligations(plan: &Value) -> Vec<ChapterObligation> {
    plan.get("obligations")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(parse_one).collect())
        .unwrap_or_default()
}

fn parse_one(value: &Value) -> Option<ChapterObligation> {
    let object = value.as_object()?;
    let id = object.get("id")?.as_str()?.trim();
    if id.is_empty() {
        return None;
    }
    let kind = object
        .get("type")
        .or_else(|| object.get("kind"))?
        .as_str()?
        .trim();
    let description = object.get("description")?.as_str()?.trim();
    if description.is_empty() {
        return None;
    }
    let enforcement = object
        .get("enforcement")
        .and_then(Value::as_str)
        .unwrap_or("hard");
    let participants = object
        .get("participants")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    Some(ChapterObligation {
        id: id.to_string(),
        chapter_order: object
            .get("chapterOrder")
            .and_then(Value::as_i64)
            .unwrap_or(0),
        kind: kind.to_string(),
        description: description.to_string(),
        enforcement: enforcement.to_string(),
        participants,
        source: object
            .get("source")
            .and_then(Value::as_str)
            .unwrap_or("chapter_plan")
            .to_string(),
        evidence: optional_text(object.get("evidence")),
    })
}

/// Normalize one chapter's obligations in place. Missing IDs are generated from
/// the chapter order and position; duplicate explicit IDs are rejected because a
/// reviewer reference would be ambiguous.
pub(crate) fn normalize_chapter_obligations(
    chapter: &mut Value,
    chapter_order: i64,
    used_ids: &mut BTreeSet<String>,
) -> Result<(), String> {
    let Some(object) = chapter.as_object_mut() else {
        return Err("章节计划项必须是对象".into());
    };
    let Some(raw) = object.remove("obligations") else {
        return Ok(());
    };
    let items = raw
        .as_array()
        .ok_or_else(|| "章节 obligations 必须是数组".to_string())?;
    let mut normalized = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let item = item
            .as_object()
            .ok_or_else(|| "章节义务必须是对象".to_string())?;
        let kind = normalize_kind(item.get("type").or_else(|| item.get("kind")))?;
        let description = required_text(item.get("description"), "description")?;
        let enforcement = normalize_enforcement(item.get("enforcement"))?;
        let participants = normalize_participants(item.get("participants"))?;
        let explicit_id = item
            .get("id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty());
        let id = match explicit_id {
            Some(id) => {
                if used_ids.contains(id) {
                    return Err(format!("章节义务 ID 重复：{id}"));
                }
                id.to_string()
            }
            None => generated_id(chapter_order, &kind, index + 1, used_ids),
        };
        used_ids.insert(id.clone());
        let source = optional_text(item.get("source")).unwrap_or_else(|| "chapter_plan".into());
        let evidence = optional_text(item.get("evidence"));
        normalized.push(json!({
            "id": id,
            "chapterOrder": chapter_order,
            "type": kind,
            "description": description,
            "enforcement": enforcement,
            "participants": participants,
            "source": source,
            "evidence": evidence,
        }));
    }
    object.insert("obligations".into(), Value::Array(normalized));
    Ok(())
}

fn normalize_kind(value: Option<&Value>) -> Result<String, String> {
    let raw = value
        .and_then(Value::as_str)
        .ok_or_else(|| "章节义务缺少 type".to_string())?
        .trim();
    let kind = match raw {
        "required_event" | "requiredEvent" | "required_events" | "requiredEvents" => {
            "required_event"
        }
        "forbidden_event" | "forbiddenEvent" | "forbidden_events" | "forbiddenEvents" => {
            "forbidden_event"
        }
        "participant" | "participants" => "participant",
        "causal_constraint" | "causalConstraint" | "causal" => "causal_constraint",
        "ordering_constraint" | "orderingConstraint" | "ordering" => "ordering_constraint",
        "continuity_constraint" | "continuityConstraint" | "continuity" => "continuity_constraint",
        other => {
            return Err(format!(
                "章节义务 type 不受支持：{other}；允许值：{}",
                OBLIGATION_TYPES.join("、")
            ));
        }
    };
    Ok(kind.to_string())
}

fn normalize_enforcement(value: Option<&Value>) -> Result<String, String> {
    let Some(value) = value else {
        return Ok("hard".into());
    };
    let raw = value
        .as_str()
        .ok_or_else(|| "章节义务 enforcement 必须是字符串".to_string())?
        .trim();
    match raw {
        "" | "hard" | "required" | "must" => Ok("hard".into()),
        "advisory" | "soft" | "suggestion" => Ok("advisory".into()),
        other => Err(format!(
            "章节义务 enforcement 不受支持：{other}；允许 hard/advisory"
        )),
    }
}

fn normalize_participants(value: Option<&Value>) -> Result<Vec<String>, String> {
    match value {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::String(text)) => {
            let text = text.trim();
            Ok(if text.is_empty() {
                Vec::new()
            } else {
                vec![text.to_string()]
            })
        }
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| {
                item.as_str()
                    .map(str::trim)
                    .filter(|text| !text.is_empty())
                    .map(str::to_string)
                    .ok_or_else(|| "章节义务 participants 必须是字符串数组".to_string())
            })
            .collect(),
        Some(_) => Err("章节义务 participants 必须是字符串数组".into()),
    }
}

fn required_text(value: Option<&Value>, field: &str) -> Result<String, String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("章节义务缺少非空 {field}"))
}

fn optional_text(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(text) => {
            let text = text.trim();
            (!text.is_empty()).then(|| text.to_string())
        }
        Value::Array(items) => {
            let text = items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .collect::<Vec<_>>()
                .join("；");
            (!text.is_empty()).then_some(text)
        }
        Value::Number(_) | Value::Bool(_) => Some(value?.to_string()),
        _ => None,
    }
}

fn generated_id(
    chapter_order: i64,
    kind: &str,
    index: usize,
    used_ids: &BTreeSet<String>,
) -> String {
    let short = match kind {
        "required_event" => "req",
        "forbidden_event" => "forbid",
        "participant" => "cast",
        "causal_constraint" => "causal",
        "ordering_constraint" => "order",
        "continuity_constraint" => "continuity",
        _ => "ob",
    };
    let base = format!("ch{chapter_order}-{short}-{index}");
    if !used_ids.contains(&base) {
        return base;
    }
    let mut suffix = 2;
    loop {
        let candidate = format!("{base}-{suffix}");
        if !used_ids.contains(&candidate) {
            return candidate;
        }
        suffix += 1;
    }
}

pub(crate) fn render_obligations(items: &[ChapterObligation]) -> String {
    let mut output = String::new();
    for item in items {
        let label = if item.enforcement == "hard" {
            "硬约束"
        } else {
            "创作建议"
        };
        output.push_str(&format!(
            "- [{}][{}][{}] {}\n",
            item.id, label, item.kind, item.description
        ));
        if !item.participants.is_empty() {
            output.push_str(&format!("  参与者：{}\n", item.participants.join("、")));
        }
        if item.source != "chapter_plan" {
            output.push_str(&format!("  来源：{}\n", item.source));
        }
        if let Some(evidence) = &item.evidence {
            output.push_str(&format!("  依据：{evidence}\n"));
        }
    }
    output
}

pub(crate) fn obligation_ids(items: &[ChapterObligation]) -> Vec<String> {
    items.iter().map(|item| item.id.clone()).collect()
}

/// A reviewer may only cite obligations that exist in the frozen target plan.
/// An absent `obligationId` remains valid for older/general review output.
pub(crate) fn validate_review_obligation_ids(
    payload: &str,
    allowed: &BTreeSet<String>,
) -> Result<String, String> {
    let value: Value =
        serde_json::from_str(payload).map_err(|_| "审校结果不是合法 JSON".to_string())?;
    let issues = value
        .get("issueDetails")
        .and_then(Value::as_array)
        .or_else(|| value.get("issues").and_then(Value::as_array));
    let Some(issues) = issues else {
        return Ok(payload.to_string());
    };
    for issue in issues {
        match issue.get("obligationId") {
            None | Some(Value::Null) => {}
            Some(Value::String(id)) => {
                let id = id.trim();
                if id.is_empty() {
                    return Err("审校 obligationId 不能为空字符串".into());
                }
                if !allowed.contains(id) {
                    return Err(format!("审校引用了不存在的章节义务：{id}"));
                }
            }
            Some(_) => return Err("审校 obligationId 必须是字符串".into()),
        }
    }
    Ok(payload.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_missing_ids_and_preserves_participants() {
        let mut chapter = json!({
            "chapterOrder": 1,
            "obligations": [
                {
                    "type": "requiredEvent",
                    "description": "林动亲眼撞见老铁匠被杀",
                    "participants": ["林动", "老铁匠"],
                    "evidence": ["第1章计划"]
                },
                {
                    "id": "ch1-forbid-1",
                    "type": "forbidden_events",
                    "description": "老铁匠提前死亡",
                    "enforcement": "advisory"
                }
            ]
        });
        let mut used = BTreeSet::new();
        normalize_chapter_obligations(&mut chapter, 1, &mut used).unwrap();
        let obligations = parse_obligations(&chapter);
        assert_eq!(obligations.len(), 2);
        assert_eq!(obligations[0].id, "ch1-req-1");
        assert_eq!(obligations[0].participants, vec!["林动", "老铁匠"]);
        assert_eq!(obligations[0].evidence.as_deref(), Some("第1章计划"));
        assert_eq!(obligations[1].enforcement, "advisory");
    }

    #[test]
    fn rejects_duplicate_explicit_ids() {
        let mut chapter = json!({
            "chapterOrder": 1,
            "obligations": [
                {"id": "same", "type": "required_event", "description": "A"},
                {"id": "same", "type": "required_event", "description": "B"}
            ]
        });
        let mut used = BTreeSet::new();
        assert!(normalize_chapter_obligations(&mut chapter, 1, &mut used).is_err());
    }

    #[test]
    fn review_reference_must_exist() {
        let allowed = BTreeSet::from(["ch1-req-1".to_string()]);
        let valid = r#"{"issueDetails":[{"obligationId":"ch1-req-1","type":"timeline","severity":"high","evidence":"x","suggestion":"y"}]}"#;
        assert!(validate_review_obligation_ids(valid, &allowed).is_ok());
        let invalid = r#"{"issueDetails":[{"obligationId":"missing","type":"timeline","severity":"high","evidence":"x","suggestion":"y"}]}"#;
        assert!(validate_review_obligation_ids(invalid, &allowed).is_err());
        let empty = r#"{"issueDetails":[{"obligationId":" ","type":"timeline","severity":"high","evidence":"x","suggestion":"y"}]}"#;
        assert!(validate_review_obligation_ids(empty, &allowed).is_err());
    }
}
