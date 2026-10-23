use novel_workflow::{ArtifactView, book_scope};

use crate::error::AppError;

pub(crate) fn latest_payload(
    artifacts: &[ArtifactView],
    artifact_type: &str,
    scope: &str,
) -> Result<serde_json::Value, AppError> {
    artifacts
        .iter()
        .filter(|artifact| artifact.artifact_type == artifact_type && artifact.scope == scope)
        .max_by_key(|artifact| artifact.version)
        .and_then(|artifact| serde_json::from_str(&artifact.payload_json).ok())
        .ok_or_else(|| AppError::integrity(format!("缺少上游产物 {artifact_type}")))
}

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
    let text =
        serde_json::to_string(&context).map_err(|_| AppError::integrity("上下文无法序列化"))?;
    Ok(truncate_chars(&text, limit))
}

pub(crate) fn truncate_chars(value: &str, limit: usize) -> String {
    value.chars().take(limit).collect()
}

pub(crate) fn validate_chapter_plan_references(
    payload: &str,
    cast: &serde_json::Value,
    volume: &serde_json::Value,
) -> Result<String, AppError> {
    let plan: serde_json::Value =
        serde_json::from_str(payload).map_err(|_| AppError::integrity("章节计划无法解析"))?;
    let member_names: std::collections::BTreeSet<&str> = cast
        .get("members")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|member| member.get("name").and_then(serde_json::Value::as_str))
        .collect();
    let volume_titles: std::collections::BTreeSet<&str> = volume
        .get("volumes")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|volume| volume.get("title").and_then(serde_json::Value::as_str))
        .collect();
    let chapters = plan
        .get("chapters")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| AppError::integrity("章节计划缺少 chapters"))?;
    for chapter in chapters {
        let volume_title = chapter
            .get("volumeTitle")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if !volume_titles.contains(volume_title) {
            return Err(AppError::validation("章节计划引用了不存在的卷"));
        }
        if let Some(names) = chapter
            .get("characterNames")
            .and_then(serde_json::Value::as_array)
        {
            for name in names.iter().filter_map(serde_json::Value::as_str) {
                if !member_names.contains(name) {
                    return Err(AppError::validation("章节计划引用了不存在的人物"));
                }
            }
        }
    }
    Ok(payload.to_string())
}

fn required_string(value: &serde_json::Value, key: &str) -> Result<(), String> {
    if value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .is_some_and(|text| !text.trim().is_empty())
    {
        Ok(())
    } else {
        Err(format!("结构化产物缺少非空字段 {key}"))
    }
}

pub(crate) fn validate_structured_output(step_id: &str, raw: &str) -> Result<String, String> {
    let value: serde_json::Value =
        serde_json::from_str(raw).map_err(|_| "模型输出不是合法 JSON".to_string())?;
    match step_id {
        "story_macro" => {
            let expansion = value.get("expansion").ok_or("Story Macro 缺少 expansion")?;
            let decomposition = value
                .get("decomposition")
                .ok_or("Story Macro 缺少 decomposition")?;
            for key in [
                "expanded_premise",
                "protagonist_core",
                "conflict_engine",
                "mystery_box",
                "emotional_line",
                "tone_reference",
            ] {
                required_string(expansion, key)?;
            }
            for key in [
                "selling_point",
                "core_conflict",
                "main_hook",
                "progression_loop",
                "growth_path",
                "ending_flavor",
            ] {
                required_string(decomposition, key)?;
            }
            for key in ["conflict_layers", "setpiece_seeds"] {
                if !expansion
                    .get(key)
                    .is_some_and(|v| v.is_object() || v.is_array())
                {
                    return Err(format!("Story Macro expansion.{key} 结构不正确"));
                }
            }
            if !value
                .get("constraints")
                .is_some_and(serde_json::Value::is_array)
                || !value.get("issues").is_some_and(serde_json::Value::is_array)
            {
                return Err("Story Macro constraints/issues 必须是数组".into());
            }
        }
        "character_cast" => {
            let members = value
                .get("members")
                .and_then(serde_json::Value::as_array)
                .ok_or("人物阵容缺少 members")?;
            if members.len() < 3 {
                return Err("人物阵容至少需要 3 人".into());
            }
            for member in members {
                for key in ["name", "role", "castRole", "storyFunction"] {
                    required_string(member, key)?;
                }
            }
            let member_names: std::collections::BTreeSet<&str> = members
                .iter()
                .filter_map(|member| member.get("name").and_then(serde_json::Value::as_str))
                .collect();
            let relations = value
                .get("relations")
                .and_then(serde_json::Value::as_array)
                .ok_or("人物阵容缺少 relations")?;
            if relations.len() < 2 {
                return Err("人物阵容至少需要 2 条关系".into());
            }
            for relation in relations {
                for key in ["sourceName", "targetName", "surfaceRelation"] {
                    required_string(relation, key)?;
                }
                for key in ["sourceName", "targetName"] {
                    let name = relation
                        .get(key)
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default();
                    if !member_names.contains(name) {
                        return Err("人物关系引用了不存在的人物".into());
                    }
                }
            }
        }
        "volume_strategy" => {
            let volumes = value
                .get("volumes")
                .and_then(serde_json::Value::as_array)
                .ok_or("卷策略缺少 volumes")?;
            if volumes.is_empty() {
                return Err("卷策略不能为空".into());
            }
            let mut orders = std::collections::BTreeSet::new();
            for volume in volumes {
                for key in ["title", "roleLabel", "coreReward", "planningMode"] {
                    required_string(volume, key)?;
                }
                let order = volume
                    .get("volumeOrder")
                    .and_then(serde_json::Value::as_i64)
                    .ok_or("卷策略缺少 volumeOrder")?;
                if !orders.insert(order) {
                    return Err("卷策略存在重复 volumeOrder".into());
                }
            }
        }
        "chapter_list" => {
            let chapters = value
                .get("chapters")
                .and_then(serde_json::Value::as_array)
                .ok_or("章节计划缺少 chapters")?;
            if chapters.is_empty() {
                return Err("章节计划不能为空".into());
            }
            let mut orders = std::collections::BTreeSet::new();
            for chapter in chapters {
                for key in ["title", "volumeTitle", "summary"] {
                    required_string(chapter, key)?;
                }
                let order = chapter
                    .get("chapterOrder")
                    .and_then(serde_json::Value::as_i64)
                    .ok_or("章节计划缺少 chapterOrder")?;
                if !orders.insert(order) {
                    return Err("章节计划存在重复 chapterOrder".into());
                }
                if chapter.get("characterNames").is_some_and(|v| !v.is_array()) {
                    return Err("章节计划 characterNames 必须是数组".into());
                }
            }
        }
        "consistency_review" => {
            let issues = value
                .get("issues")
                .and_then(serde_json::Value::as_array)
                .ok_or("审校结果缺少 issues 数组")?;
            for issue in issues {
                for key in ["type", "severity", "evidence", "suggestion"] {
                    required_string(issue, key)?;
                }
            }
            let normalized = serde_json::json!({
                "issues": issues.len(),
                "issueDetails": issues,
                "status": if issues.is_empty() { "clean" } else { "issues_found" },
            });
            return serde_json::to_string(&normalized).map_err(|_| "审校结果无法序列化".into());
        }
        _ => return Err("未知的结构化步骤".into()),
    }
    serde_json::to_string(&value).map_err(|_| "结构化产物无法序列化".into())
}
