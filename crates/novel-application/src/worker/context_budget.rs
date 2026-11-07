use novel_workflow::{ArtifactView, book_scope};

use super::worker_validation::latest_payload;
use crate::error::AppError;

pub(crate) fn context_payload(
    artifacts: &[ArtifactView],
    types: &[&str],
    limit: usize,
) -> Result<String, AppError> {
    let mut context = serde_json::Map::new();
    for artifact_type in types {
        let value = latest_payload(artifacts, artifact_type, book_scope())?;
        context.insert((*artifact_type).to_string(), value);
    }
    let full = serde_json::Value::Object(context);
    let text = serialize_context(&full)?;
    if text.chars().count() <= limit {
        return Ok(text);
    }
    for trim in [300usize, 160, 80, 40] {
        let candidate = trim_context(&full, trim);
        let text = serialize_context(&candidate)?;
        if text.chars().count() <= limit {
            return Ok(text);
        }
    }
    let minimal = minimal_context(&full);
    let text = serialize_context(&minimal)?;
    if text.chars().count() <= limit {
        return Ok(text);
    }
    Err(AppError::integrity(format!(
        "Planner 上下文预算不足：必选的 keyEvents/constraints/人物/卷无法放入 {limit} 字符预算，不能截断成无效 JSON。"
    )))
}

fn serialize_context(value: &serde_json::Value) -> Result<String, AppError> {
    serde_json::to_string(value).map_err(|_| AppError::integrity("Planner 上下文无法序列化"))
}

fn trim_context(full: &serde_json::Value, trim: usize) -> serde_json::Value {
    let mut out = full.clone();
    if let Some(map) = out.as_object_mut() {
        if let Some(object) = map
            .get_mut("story_macro")
            .and_then(serde_json::Value::as_object_mut)
        {
            for key in ["expansion", "decomposition"] {
                if let Some(value) = object.get_mut(key) {
                    trim_strings(value, trim);
                }
            }
            object.remove("issues");
        }
        if let Some(cast) = map.get_mut("character_cast") {
            trim_strings(cast, trim.max(80));
        }
        if let Some(volume) = map.get_mut("volume_strategy") {
            trim_strings(volume, trim);
        }
    }
    out
}

fn trim_strings(value: &mut serde_json::Value, limit: usize) {
    match value {
        serde_json::Value::String(text) => {
            if text.chars().count() > limit {
                *text = text.chars().take(limit).collect();
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                trim_strings(item, limit);
            }
        }
        serde_json::Value::Object(map) => {
            for (key, item) in map.iter_mut() {
                if key == "keyEvents" || key == "constraints" {
                    continue;
                }
                trim_strings(item, limit);
            }
        }
        _ => {}
    }
}

fn minimal_context(full: &serde_json::Value) -> serde_json::Value {
    let mut out = serde_json::Map::new();
    if let Some(story_macro) = full.get("story_macro") {
        let mut keep = serde_json::Map::new();
        for key in ["keyEvents", "constraints"] {
            if let Some(value) = story_macro.get(key) {
                keep.insert(key.to_string(), value.clone());
            }
        }
        if let Some(expansion) = story_macro
            .get("expansion")
            .and_then(serde_json::Value::as_object)
        {
            let mut trimmed = serde_json::Map::new();
            for key in ["expanded_premise", "protagonist_core"] {
                if let Some(value) = expansion.get(key) {
                    let mut value = value.clone();
                    trim_strings(&mut value, 200);
                    trimmed.insert(key.to_string(), value);
                }
            }
            if !trimmed.is_empty() {
                keep.insert("expansion".into(), serde_json::Value::Object(trimmed));
            }
        }
        if let Some(decomposition) = story_macro
            .get("decomposition")
            .and_then(serde_json::Value::as_object)
        {
            let mut trimmed = serde_json::Map::new();
            for key in ["selling_point", "core_conflict"] {
                if let Some(value) = decomposition.get(key) {
                    let mut value = value.clone();
                    trim_strings(&mut value, 120);
                    trimmed.insert(key.to_string(), value);
                }
            }
            if !trimmed.is_empty() {
                keep.insert("decomposition".into(), serde_json::Value::Object(trimmed));
            }
        }
        out.insert("story_macro".into(), serde_json::Value::Object(keep));
    }
    if let Some(cast) = full.get("character_cast") {
        let mut members = Vec::new();
        for member in cast
            .get("members")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
        {
            let mut keep = serde_json::Map::new();
            for key in ["name", "role", "castRole", "storyFunction"] {
                if let Some(value) = member.get(key) {
                    let mut value = value.clone();
                    trim_strings(&mut value, 120);
                    keep.insert(key.to_string(), value);
                }
            }
            members.push(serde_json::Value::Object(keep));
        }
        let mut relations = Vec::new();
        for relation in cast
            .get("relations")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
        {
            let mut keep = serde_json::Map::new();
            for key in ["sourceName", "targetName", "surfaceRelation"] {
                if let Some(value) = relation.get(key) {
                    let mut value = value.clone();
                    trim_strings(&mut value, 80);
                    keep.insert(key.to_string(), value);
                }
            }
            relations.push(serde_json::Value::Object(keep));
        }
        out.insert(
            "character_cast".into(),
            serde_json::json!({ "members": members, "relations": relations }),
        );
    }
    if let Some(volume) = full.get("volume_strategy") {
        let mut volumes = Vec::new();
        for item in volume
            .get("volumes")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
        {
            let mut keep = serde_json::Map::new();
            for key in ["title", "roleLabel", "planningMode", "coreReward"] {
                if let Some(value) = item.get(key) {
                    let mut value = value.clone();
                    trim_strings(&mut value, 80);
                    keep.insert(key.to_string(), value);
                }
            }
            volumes.push(serde_json::Value::Object(keep));
        }
        out.insert(
            "volume_strategy".into(),
            serde_json::json!({ "volumes": volumes }),
        );
    }
    if let Some(map) = full.as_object() {
        for (key, value) in map {
            if out.contains_key(key) {
                continue;
            }
            let mut value = value.clone();
            trim_strings(&mut value, 80);
            out.insert(key.clone(), value);
        }
    }
    serde_json::Value::Object(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use novel_domain::ArtifactStatus;

    fn artifact(ty: &str, payload: serde_json::Value) -> ArtifactView {
        ArtifactView {
            artifact_type: ty.into(),
            scope: "book".into(),
            version: 1,
            run_id: None,
            status: novel_domain::ArtifactStatus::Confirmed,
            protected_user_content: false,
            payload_json: payload.to_string(),
        }
    }

    #[test]
    fn planner_context_keeps_key_events_and_never_truncates_json() {
        let long = "很长的解释性文字".repeat(2000);
        let artifacts = vec![
            artifact(
                "story_macro",
                serde_json::json!({
                    "expansion": {
                        "expanded_premise": long, "protagonist_core": long,
                        "conflict_engine": long, "conflict_layers": {"external": long},
                        "mystery_box": long, "emotional_line": long,
                        "setpiece_seeds": [long], "tone_reference": long
                    },
                    "decomposition": {
                        "selling_point": long, "core_conflict": long, "main_hook": long,
                        "progression_loop": long, "growth_path": long, "ending_flavor": long
                    },
                    "constraints": ["第1章必须目击灭口"],
                    "keyEvents": [{
                        "id": "ch1-event-1", "chapterOrder": 1, "type": "required_event",
                        "description": "林动亲眼撞见老铁匠被灭口",
                        "participants": ["林动", "老铁匠"], "enforcement": "hard"
                    }]
                }),
            ),
            artifact(
                "character_cast",
                serde_json::json!({
                    "members": [{"name": "林动", "role": "主角", "castRole": "protagonist", "storyFunction": long}],
                    "relations": [{"sourceName": "林动", "targetName": "老铁匠", "surfaceRelation": long}]
                }),
            ),
            artifact(
                "volume_strategy",
                serde_json::json!({
                    "volumes": [{"volumeOrder": 1, "title": "山门卷", "roleLabel": long, "coreReward": long, "planningMode": "hard"}]
                }),
            ),
        ];
        let text = context_payload(
            &artifacts,
            &["story_macro", "volume_strategy", "character_cast"],
            4000,
        )
        .unwrap();
        assert!(text.chars().count() <= 4000);
        let value: serde_json::Value =
            serde_json::from_str(&text).expect("context must stay valid JSON");
        assert_eq!(value["story_macro"]["keyEvents"][0]["id"], "ch1-event-1");
        assert!(
            value["story_macro"]["keyEvents"][0]["description"]
                .as_str()
                .is_some_and(|text| text.contains("老铁匠"))
        );
    }

    #[test]
    fn planner_context_fails_when_mandatory_events_exceed_budget() {
        let long = "必选事件描述".repeat(3000);
        let artifacts = vec![artifact(
            "story_macro",
            serde_json::json!({
                "expansion": {}, "decomposition": {},
                "constraints": [],
                "keyEvents": [{
                    "id": "ch1-event-1", "chapterOrder": 1, "type": "required_event",
                    "description": long, "participants": ["林动"], "enforcement": "hard"
                }]
            }),
        )];
        let error = context_payload(&artifacts, &["story_macro"], 500).unwrap_err();
        assert!(error.to_string().contains("预算不足"), "{error}");
    }
}
