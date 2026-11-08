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
    pub source_event_id: Option<String>,
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
        source_event_id: optional_text(object.get("sourceEventId")),
    })
}

/// Normalize one chapter's obligations in place for a freshly generated plan.
///
/// `chapter-list-v2` requires every chapter to declare its obligations. A
/// missing array, a malformed array, an empty array, or a chapter with no hard
/// obligation is rejected: silently treating that as a legacy plan would let the
/// writer proceed without any structured plot contract. Historical plans stay
/// readable through [`parse_obligations`], which keeps the lenient read path.
pub(crate) fn normalize_chapter_obligations(
    chapter: &mut Value,
    chapter_order: i64,
    used_ids: &mut BTreeSet<String>,
) -> Result<(), String> {
    let Some(object) = chapter.as_object_mut() else {
        return Err("章节计划项必须是对象".into());
    };
    let Some(raw) = object.remove("obligations") else {
        return Err(format!(
            "章节计划第 {chapter_order} 章缺少 obligations 数组；chapter-list-v2 要求每章显式输出结构化义务，不能静默降级为旧计划。"
        ));
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
        let source_event_id = optional_text(item.get("sourceEventId"));
        normalized.push(json!({
            "id": id,
            "chapterOrder": chapter_order,
            "type": kind,
            "description": description,
            "enforcement": enforcement,
            "participants": participants,
            "source": source,
            "evidence": evidence,
            "sourceEventId": source_event_id,
        }));
    }
    if !normalized
        .iter()
        .any(|item| item.get("enforcement").and_then(Value::as_str) == Some("hard"))
    {
        return Err(format!(
            "章节计划第 {chapter_order} 章没有 hard 义务（obligations 为空或全部是 advisory）；每章至少需要一条不可改写的关键义务，不能在没有剧情契约的情况下继续生成正文。"
        ));
    }
    object.insert("obligations".into(), Value::Array(normalized));
    Ok(())
}

/// Normalize the optional `story_macro.keyEvents` array. These are the
/// chapter-level key events the planner must carry into `chapter_list`
/// obligations; an empty array is rejected so a planner cannot silently skip
/// coverage by emitting `keyEvents: []`.
pub(crate) fn normalize_story_key_events(value: &mut Value, require: bool) -> Result<(), String> {
    let Some(raw) = value.get_mut("keyEvents") else {
        if require {
            return Err(
                "story_macro 缺少 keyEvents；story-macro-v2 要求至少一条合法关键事件".into(),
            );
        }
        return Ok(());
    };
    let items = raw
        .as_array()
        .ok_or_else(|| "story_macro.keyEvents 必须是数组".to_string())?;
    if items.is_empty() {
        return Err("story_macro.keyEvents 不能为空；至少列出一条关键事件".into());
    }
    let mut normalized = Vec::with_capacity(items.len());
    let mut used = BTreeSet::new();
    for (index, item) in items.iter().enumerate() {
        let object = item
            .as_object()
            .ok_or_else(|| "story_macro.keyEvents 项必须是对象".to_string())?;
        let chapter_order = object
            .get("chapterOrder")
            .and_then(Value::as_i64)
            .ok_or_else(|| "story_macro.keyEvents 项缺少 chapterOrder".to_string())?;
        if chapter_order < 1 {
            return Err("story_macro.keyEvents.chapterOrder 必须从 1 开始".into());
        }
        let kind = normalize_kind(object.get("type").or_else(|| object.get("kind")))?;
        let description = required_text(object.get("description"), "description")?;
        let enforcement = normalize_enforcement(object.get("enforcement"))?;
        let participants = normalize_participants(object.get("participants"))?;
        let explicit = object
            .get("id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty());
        let id = match explicit {
            Some(id) => {
                if !used.insert(id.to_string()) {
                    return Err(format!("story_macro.keyEvents ID 重复：{id}"));
                }
                id.to_string()
            }
            None => {
                let mut suffix = index + 1;
                loop {
                    let candidate = format!("ch{chapter_order}-event-{suffix}");
                    if used.insert(candidate.clone()) {
                        break candidate;
                    }
                    suffix += 1;
                }
            }
        };
        normalized.push(json!({
            "id": id,
            "chapterOrder": chapter_order,
            "type": kind,
            "description": description,
            "enforcement": enforcement,
            "participants": participants,
        }));
    }
    if !normalized
        .iter()
        .any(|item| item.get("enforcement").and_then(Value::as_str) == Some("hard"))
    {
        return Err(
            "story_macro.keyEvents 至少需要一条 hard 关键事件；不能全部用 advisory 规避约束".into(),
        );
    }
    value["keyEvents"] = Value::Array(normalized);
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

/// Render obligations for the writer, reviewer, and repair prompts. Hard and
/// advisory obligations are separate sections with different wording: only hard
/// obligations are plot contracts; advisory items are suggestions that a repair
/// may drop.
pub(crate) fn render_obligations(items: &[ChapterObligation]) -> String {
    let mut output = String::new();
    let hard: Vec<&ChapterObligation> = items
        .iter()
        .filter(|item| item.enforcement == "hard")
        .collect();
    let advisory: Vec<&ChapterObligation> = items
        .iter()
        .filter(|item| item.enforcement != "hard")
        .collect();
    if !hard.is_empty() {
        output.push_str("【硬约束｜必须遵守，不可擅自改写】\n");
        for item in hard {
            push_obligation_line(&mut output, item);
        }
    }
    if !advisory.is_empty() {
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str("【创作建议｜可合理取舍，不构成本章失败】\n");
        for item in advisory {
            push_obligation_line(&mut output, item);
        }
    }
    output
}

fn push_obligation_line(output: &mut String, item: &ChapterObligation) {
    output.push_str(&format!(
        "- [{}][{}] {}\n",
        item.id, item.kind, item.description
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
    if let Some(event_id) = &item.source_event_id {
        output.push_str(&format!("  来源事件：{event_id}\n"));
    }
}

/// Classify review issues by the enforcement of the obligation they cite and
/// rewrite the review payload so the workflow guard only counts hard issues.
/// Advisory-only feedback stays visible in `issueDetails` but must not trigger a
/// repair or block the chapter.
pub(crate) fn annotate_review_obligations(
    payload: &str,
    obligations: &[ChapterObligation],
) -> Result<String, String> {
    let enforcement: std::collections::BTreeMap<&str, &str> = obligations
        .iter()
        .map(|item| (item.id.as_str(), item.enforcement.as_str()))
        .collect();
    let mut value: Value =
        serde_json::from_str(payload).map_err(|_| "审校结果不是合法 JSON".to_string())?;
    let Some(issues) = value.get_mut("issueDetails").and_then(Value::as_array_mut) else {
        return Ok(payload.to_string());
    };
    let mut hard = 0usize;
    let mut advisory = 0usize;
    for issue in issues.iter_mut() {
        let Some(object) = issue.as_object_mut() else {
            continue;
        };
        let obligation_id = object
            .get("obligationId")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|id| !id.is_empty());
        match obligation_id {
            Some(id) => {
                let Some(level) = enforcement.get(id) else {
                    return Err(format!("审校引用了不存在的章节义务：{id}"));
                };
                object.insert(
                    "obligationEnforcement".into(),
                    Value::String((*level).to_string()),
                );
                if *level == "hard" {
                    hard += 1;
                } else {
                    advisory += 1;
                }
            }
            // 没有引用义务的普通审校问题仍按需要修复的问题处理。
            None => hard += 1,
        }
    }
    if let Some(object) = value.as_object_mut() {
        object.insert("issues".into(), Value::from(hard));
        object.insert("advisoryIssueCount".into(), Value::from(advisory));
        let status = if hard == 0 && advisory == 0 {
            "clean"
        } else if hard == 0 {
            "advisory_only"
        } else {
            "issues_found"
        };
        object.insert("status".into(), Value::String(status.into()));
        object.insert(
            "reviewOutcome".into(),
            Value::String(
                if hard == 0 && advisory == 0 {
                    "review_succeeded_no_issues"
                } else if hard == 0 {
                    "review_succeeded_with_advisory_notes"
                } else {
                    "review_succeeded_with_issues"
                }
                .into(),
            ),
        );
    }
    serde_json::to_string(&value).map_err(|_| "审校结果无法序列化".into())
}

/// Hard-obligation violations must quote the current candidate. The check is
/// deliberately narrow: it only proves the quoted text exists in the candidate
/// after whitespace normalization; it does not judge semantics.
pub(crate) fn validate_review_evidence(
    payload: &str,
    candidate: &str,
    obligations: &[ChapterObligation],
) -> Result<(), String> {
    let hard_ids: BTreeSet<&str> = obligations
        .iter()
        .filter(|item| item.enforcement == "hard")
        .map(|item| item.id.as_str())
        .collect();
    let value: Value =
        serde_json::from_str(payload).map_err(|_| "审校结果不是合法 JSON".to_string())?;
    let Some(issues) = value.get("issueDetails").and_then(Value::as_array) else {
        return Ok(());
    };
    let candidate = normalize_quote(candidate);
    for issue in issues {
        let Some(obligation_id) = issue
            .get("obligationId")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|id| !id.is_empty())
        else {
            continue;
        };
        if !hard_ids.contains(obligation_id) {
            continue;
        }
        let evidence = issue
            .get("evidence")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .ok_or_else(|| format!("审校问题 {obligation_id} 缺少正文证据"))?;
        if !quote_is_located(evidence, &candidate) {
            return Err(format!(
                "审校问题 {obligation_id} 的证据无法在当前候选正文中定位：{evidence}"
            ));
        }
    }
    Ok(())
}

fn normalize_quote(text: &str) -> String {
    text.chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>()
}

fn trim_quote_marks(text: &str) -> &str {
    text.trim_matches(|ch: char| {
        matches!(
            ch,
            '"' | '\''
                | '“'
                | '”'
                | '‘'
                | '’'
                | '「'
                | '」'
                | '『'
                | '』'
                | '，'
                | '。'
                | '、'
                | '；'
                | '：'
                | '！'
                | '？'
                | ','
                | '.'
                | ';'
                | ':'
                | '!'
                | '?'
        )
    })
}

/// Ellipsis-separated quotes are accepted only when every segment is present in
/// order; this keeps multi-sentence quotes verifiable without treating a single
/// keyword as proof.
fn quote_is_located(evidence: &str, normalized_candidate: &str) -> bool {
    let segments: Vec<String> = evidence
        .replace("...", "…")
        .split('…')
        .map(trim_quote_marks)
        .map(normalize_quote)
        .filter(|segment| !segment.is_empty())
        .collect();
    if segments.is_empty() {
        return false;
    }
    let mut cursor = 0usize;
    for segment in segments {
        let Some(found) = normalized_candidate[cursor..].find(&segment) else {
            return false;
        };
        cursor += found + segment.len();
    }
    true
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
    fn story_key_events_get_stable_ids_and_fields_normalized() {
        let mut macro_value = json!({
            "keyEvents": [{
                "chapterOrder": 1,
                "type": "requiredEvent",
                "description": "林动亲眼撞见老铁匠被灭口",
                "participants": ["林动", "老铁匠"],
                "enforcement": "hard"
            }]
        });
        normalize_story_key_events(&mut macro_value, true).unwrap();
        let event = &macro_value["keyEvents"][0];
        assert_eq!(event["id"], "ch1-event-1");
        assert_eq!(event["type"], "required_event");
        assert_eq!(event["enforcement"], "hard");
        assert_eq!(event["participants"][0], "林动");
    }

    #[test]
    fn empty_story_key_events_are_rejected() {
        let mut macro_value = json!({"keyEvents": []});
        let error = normalize_story_key_events(&mut macro_value, true).unwrap_err();
        assert!(error.contains("不能为空"), "{error}");
    }

    #[test]
    fn obligation_source_event_id_is_preserved() {
        let mut chapter = json!({
            "chapterOrder": 1,
            "obligations": [{
                "type": "required_event",
                "description": "林动亲眼撞见老铁匠被灭口",
                "sourceEventId": "ch1-event-1"
            }]
        });
        let mut used = BTreeSet::new();
        normalize_chapter_obligations(&mut chapter, 1, &mut used).unwrap();
        let obligation = &chapter["obligations"][0];
        assert_eq!(obligation["sourceEventId"], "ch1-event-1");
        let parsed = parse_obligations(&chapter);
        assert_eq!(parsed[0].source_event_id.as_deref(), Some("ch1-event-1"));
    }

    #[test]
    fn rejects_missing_empty_or_advisory_only_obligations() {
        let mut missing = json!({"chapterOrder": 1});
        let mut used = BTreeSet::new();
        let error = normalize_chapter_obligations(&mut missing, 1, &mut used).unwrap_err();
        assert!(error.contains("缺少 obligations"), "{error}");

        let mut empty = json!({"chapterOrder": 1, "obligations": []});
        let mut used = BTreeSet::new();
        let error = normalize_chapter_obligations(&mut empty, 1, &mut used).unwrap_err();
        assert!(error.contains("没有 hard 义务"), "{error}");

        let mut advisory_only = json!({
            "chapterOrder": 1,
            "obligations": [{
                "type": "continuity_constraint",
                "description": "保持文风一致",
                "enforcement": "advisory"
            }]
        });
        let mut used = BTreeSet::new();
        let error = normalize_chapter_obligations(&mut advisory_only, 1, &mut used).unwrap_err();
        assert!(error.contains("没有 hard 义务"), "{error}");
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

    fn obligation(id: &str, enforcement: &str) -> ChapterObligation {
        ChapterObligation {
            id: id.into(),
            chapter_order: 1,
            kind: "required_event".into(),
            description: format!("{id} 描述"),
            enforcement: enforcement.into(),
            participants: vec!["林动".into()],
            source: "chapter_plan".into(),
            evidence: None,
            source_event_id: None,
        }
    }

    #[test]
    fn rendering_separates_hard_and_advisory_wording() {
        let items = vec![
            obligation("ch1-req-1", "hard"),
            obligation("ch1-adv-1", "advisory"),
        ];
        let rendered = render_obligations(&items);
        assert!(rendered.contains("【硬约束｜必须遵守"));
        assert!(rendered.contains("【创作建议｜可合理取舍"));
        let advisory_section = rendered
            .split("【创作建议")
            .nth(1)
            .expect("advisory section");
        assert!(!advisory_section.contains("必须"));
        assert!(!advisory_section.contains("禁止改写"));
    }

    #[test]
    fn rendering_omits_an_empty_advisory_section() {
        let rendered = render_obligations(&[obligation("ch1-req-1", "hard")]);
        assert!(rendered.contains("【硬约束"));
        assert!(!rendered.contains("【创作建议"));
        assert!(render_obligations(&[]).is_empty());
    }

    #[test]
    fn advisory_issues_do_not_count_as_repairable_hard_issues() {
        let obligations = vec![
            obligation("ch1-req-1", "hard"),
            obligation("ch1-adv-1", "advisory"),
        ];
        let payload = r#"{"issues":2,"issueDetails":[
            {"obligationId":"ch1-adv-1","type":"style","severity":"low","evidence":"a","suggestion":"b"},
            {"obligationId":"ch1-req-1","type":"timeline","severity":"high","evidence":"c","suggestion":"d"}
        ]}"#;
        let annotated = annotate_review_obligations(payload, &obligations).unwrap();
        let value: Value = serde_json::from_str(&annotated).unwrap();
        assert_eq!(value["issues"], 1);
        assert_eq!(value["advisoryIssueCount"], 1);
        assert_eq!(
            value["issueDetails"][0]["obligationEnforcement"],
            "advisory"
        );
        assert_eq!(value["issueDetails"][1]["obligationEnforcement"], "hard");

        let advisory_only = r#"{"issues":1,"issueDetails":[
            {"obligationId":"ch1-adv-1","type":"style","severity":"low","evidence":"a","suggestion":"b"}
        ]}"#;
        let annotated = annotate_review_obligations(advisory_only, &obligations).unwrap();
        let value: Value = serde_json::from_str(&annotated).unwrap();
        assert_eq!(value["issues"], 0);
        assert_eq!(value["status"], "advisory_only");
    }

    #[test]
    fn hard_issue_evidence_must_be_locatable_in_the_candidate() {
        let obligations = vec![
            obligation("ch1-req-1", "hard"),
            obligation("ch1-adv-1", "advisory"),
        ];
        let candidate = "雪夜里，林动赶到时老铁匠已经死亡。";
        let matching = r#"{"issueDetails":[
            {"obligationId":"ch1-req-1","type":"timeline","severity":"high","evidence":"“林动 赶到时老铁匠已经死亡”","suggestion":"改为亲眼撞见"}
        ]}"#;
        assert!(validate_review_evidence(matching, candidate, &obligations).is_ok());

        let invented = r#"{"issueDetails":[
            {"obligationId":"ch1-req-1","type":"timeline","severity":"high","evidence":"林动亲手杀死了老铁匠","suggestion":"改为亲眼撞见"}
        ]}"#;
        assert!(validate_review_evidence(invented, candidate, &obligations).is_err());

        let advisory = r#"{"issueDetails":[
            {"obligationId":"ch1-adv-1","type":"style","severity":"low","evidence":"这条建议没有采用","suggestion":"可忽略"}
        ]}"#;
        assert!(validate_review_evidence(advisory, candidate, &obligations).is_ok());
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
