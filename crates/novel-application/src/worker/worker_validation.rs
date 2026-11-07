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

pub(crate) fn truncate_chars(value: &str, limit: usize) -> String {
    value.chars().take(limit).collect()
}

pub(crate) fn validate_chapter_plan_references(
    payload: &str,
    cast: &serde_json::Value,
    volume: &serde_json::Value,
    macro_payload: &serde_json::Value,
    enforce_key_events: bool,
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
    let key_events: Vec<&serde_json::Value> = if enforce_key_events {
        macro_payload
            .get("keyEvents")
            .and_then(serde_json::Value::as_array)
            .map(|items| items.iter().collect())
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let mut key_events_by_id = std::collections::BTreeMap::new();
    for event in &key_events {
        if let Some(id) = event.get("id").and_then(serde_json::Value::as_str) {
            key_events_by_id.insert(id, *event);
        }
    }
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
        if !enforce_key_events {
            continue;
        }
        let chapter_order = chapter
            .get("chapterOrder")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or_default();
        let obligations = chapter
            .get("obligations")
            .and_then(serde_json::Value::as_array);
        if let Some(obligations) = obligations {
            for obligation in obligations {
                let Some(source_event_id) = obligation
                    .get("sourceEventId")
                    .and_then(serde_json::Value::as_str)
                else {
                    continue;
                };
                let event = key_events_by_id.get(source_event_id).ok_or_else(|| {
                    AppError::validation(format!(
                        "章节计划第 {chapter_order} 章引用了不存在的 story_macro.keyEvents：{source_event_id}"
                    ))
                })?;
                if event
                    .get("chapterOrder")
                    .and_then(serde_json::Value::as_i64)
                    != Some(chapter_order)
                {
                    return Err(AppError::validation(format!(
                        "章节计划第 {chapter_order} 章的 sourceEventId {source_event_id} 指向了其他章节的关键事件"
                    )));
                }
                if event.get("type").and_then(serde_json::Value::as_str)
                    != obligation.get("type").and_then(serde_json::Value::as_str)
                {
                    return Err(AppError::validation(format!(
                        "章节计划第 {chapter_order} 章的关键事件 {source_event_id} 类型与义务不一致"
                    )));
                }
                if event.get("enforcement").and_then(serde_json::Value::as_str) == Some("hard")
                    && obligation
                        .get("enforcement")
                        .and_then(serde_json::Value::as_str)
                        != Some("hard")
                {
                    return Err(AppError::validation(format!(
                        "章节计划第 {chapter_order} 章把 hard 关键事件 {source_event_id} 降级为非 hard 义务"
                    )));
                }
                if normalized_event_text(
                    event
                        .get("description")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default(),
                ) != normalized_event_text(
                    obligation
                        .get("description")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default(),
                ) {
                    return Err(AppError::validation(format!(
                        "章节计划第 {chapter_order} 章的关键事件 {source_event_id} 描述被改写，不能覆盖规范事件"
                    )));
                }
                if event_participants(event) != event_participants(obligation) {
                    return Err(AppError::validation(format!(
                        "章节计划第 {chapter_order} 章的关键事件 {source_event_id} 参与者被替换"
                    )));
                }
            }
        }
        for event in &key_events {
            if event
                .get("chapterOrder")
                .and_then(serde_json::Value::as_i64)
                != Some(chapter_order)
            {
                continue;
            }
            if event.get("enforcement").and_then(serde_json::Value::as_str) != Some("hard") {
                continue;
            }
            let event_id = event
                .get("id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            let covered = obligations.is_some_and(|items| {
                items.iter().any(|obligation| {
                    obligation
                        .get("sourceEventId")
                        .and_then(serde_json::Value::as_str)
                        == Some(event_id)
                        && obligation
                            .get("enforcement")
                            .and_then(serde_json::Value::as_str)
                            == Some("hard")
                })
            });
            if !covered {
                let description = event
                    .get("description")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default();
                return Err(AppError::validation(format!(
                    "章节计划第 {chapter_order} 章未覆盖 story_macro 关键事件 {event_id}：{description}"
                )));
            }
        }
    }
    Ok(payload.to_string())
}

fn normalized_event_text(text: &str) -> String {
    text.chars().filter(|ch| !ch.is_whitespace()).collect()
}

fn event_participants(value: &serde_json::Value) -> std::collections::BTreeSet<String> {
    value
        .get("participants")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
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

const EXPANSION_ALIASES: &[(&str, &str)] = &[
    ("expandedPremise", "expanded_premise"),
    ("protagonistCore", "protagonist_core"),
    ("conflictEngine", "conflict_engine"),
    ("conflictLayers", "conflict_layers"),
    ("mysteryBox", "mystery_box"),
    ("emotionalLine", "emotional_line"),
    ("setpieceSeeds", "setpiece_seeds"),
    ("toneReference", "tone_reference"),
];

const DECOMPOSITION_ALIASES: &[(&str, &str)] = &[
    ("sellingPoint", "selling_point"),
    ("coreConflict", "core_conflict"),
    ("mainHook", "main_hook"),
    ("progressionLoop", "progression_loop"),
    ("growthPath", "growth_path"),
    ("majorPayoffs", "major_payoffs"),
    ("endingFlavor", "ending_flavor"),
];

fn rename_alias_keys(value: &mut serde_json::Value, aliases: &[(&str, &str)]) {
    let Some(map) = value.as_object_mut() else {
        return;
    };
    for (camel, snake) in aliases {
        if !map.contains_key(*snake)
            && let Some(alias_value) = map.remove(*camel)
        {
            map.insert((*snake).to_string(), alias_value);
        }
    }
}

/// 文本字段被模型写成数组时按“；”连接；数字/布尔转字符串。对象/空值保持原样，
/// 交给校验器报错，避免凭空发明内容。
fn coerce_text_field(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::String(text) => {
            *text = text.trim().to_string();
        }
        serde_json::Value::Array(items) => {
            let joined = items
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .collect::<Vec<_>>()
                .join("；");
            if !joined.is_empty() {
                *value = serde_json::Value::String(joined);
            }
        }
        serde_json::Value::Number(_) | serde_json::Value::Bool(_) => {
            *value = serde_json::Value::String(value.to_string());
        }
        _ => {}
    }
}

/// 模型常把 story_macro 的字段写成 camelCase、把文本字段写成数组，或把
/// conflict_layers/setpiece_seeds 写成字符串；下游按固定 snake_case/结构使用。
/// 这是对已解析结构化输出的确定性归一化，不改变模型决策。
fn normalize_story_macro(value: &mut serde_json::Value) {
    if let Some(expansion) = value.get_mut("expansion") {
        rename_alias_keys(expansion, EXPANSION_ALIASES);
        for key in [
            "expanded_premise",
            "protagonist_core",
            "conflict_engine",
            "mystery_box",
            "emotional_line",
            "tone_reference",
        ] {
            if let Some(field) = expansion.get_mut(key) {
                coerce_text_field(field);
            }
        }
        if let Some(layers) = expansion.get_mut("conflict_layers")
            && let Some(text) = layers.as_str()
        {
            *layers = serde_json::json!({ "external": text });
        }
        if let Some(seeds) = expansion.get_mut("setpiece_seeds")
            && let Some(text) = seeds.as_str()
        {
            *seeds = serde_json::json!([text]);
        }
    }
    if let Some(decomposition) = value.get_mut("decomposition") {
        rename_alias_keys(decomposition, DECOMPOSITION_ALIASES);
        for key in [
            "selling_point",
            "core_conflict",
            "main_hook",
            "progression_loop",
            "growth_path",
            "ending_flavor",
        ] {
            if let Some(field) = decomposition.get_mut(key) {
                coerce_text_field(field);
            }
        }
    }
}

#[allow(dead_code)] // used by legacy validation tests; production uses the strict variant
pub(crate) fn validate_structured_output(step_id: &str, raw: &str) -> Result<String, String> {
    validate_structured_output_with(step_id, raw, false)
}

pub(crate) fn validate_structured_output_with(
    step_id: &str,
    raw: &str,
    require_story_key_events: bool,
) -> Result<String, String> {
    let mut value: serde_json::Value =
        serde_json::from_str(raw).map_err(|_| "模型输出不是合法 JSON".to_string())?;
    if step_id == "story_macro" {
        normalize_story_macro(&mut value);
    }
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
            crate::obligations::normalize_story_key_events(&mut value, require_story_key_events)?;
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
                .get_mut("chapters")
                .and_then(serde_json::Value::as_array_mut)
                .ok_or("章节计划缺少 chapters")?;
            if chapters.is_empty() {
                return Err("章节计划不能为空".into());
            }
            let mut orders = std::collections::BTreeSet::new();
            let mut obligation_ids = std::collections::BTreeSet::new();
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
                crate::obligations::normalize_chapter_obligations(
                    chapter,
                    order,
                    &mut obligation_ids,
                )?;
            }
        }
        "consistency_review" => {
            let issues = value
                .get("issues")
                .and_then(serde_json::Value::as_array)
                .ok_or("审校结果缺少 issues 数组")?;
            let mut normalized_issues = Vec::with_capacity(issues.len());
            let mut seen_ids: std::collections::BTreeMap<String, String> =
                std::collections::BTreeMap::new();
            for (index, issue) in issues.iter().enumerate() {
                for key in ["type", "severity", "evidence", "suggestion"] {
                    required_string(issue, key)?;
                }
                if issue
                    .get("obligationId")
                    .is_some_and(|value| !value.is_null() && !value.is_string())
                {
                    return Err("审校 obligationId 必须是字符串".into());
                }
                if issue
                    .get("needsAuthorConfirmation")
                    .is_some_and(|value| !value.is_null() && !value.is_boolean())
                {
                    return Err("审校 needsAuthorConfirmation 必须是布尔值".into());
                }
                let issue_id = issue
                    .get("issueId")
                    .or_else(|| issue.get("id"))
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| {
                        format!(
                            "issue-{index}-{}",
                            crate::hash::sha256_hex(issue.to_string().as_bytes())
                        )
                    });
                let identity = format!(
                    "{}|{}|{}|{}",
                    issue
                        .get("type")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default(),
                    issue
                        .get("evidence")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default(),
                    issue
                        .get("suggestion")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default(),
                    issue
                        .get("obligationId")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default(),
                );
                if let Some(previous) = seen_ids.get(&issue_id) {
                    if previous == &identity {
                        continue;
                    }
                    return Err("审校返回了重复 issueId，但问题内容不同；不能合并不同问题".into());
                }
                seen_ids.insert(issue_id.clone(), identity);
                let description = issue
                    .get("description")
                    .and_then(serde_json::Value::as_str)
                    .filter(|text| !text.trim().is_empty())
                    .unwrap_or_else(|| {
                        issue
                            .get("evidence")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or_default()
                    });
                normalized_issues.push(serde_json::json!({
                    "issueId": issue_id,
                    "type": issue["type"],
                    "severity": issue["severity"],
                    "description": description,
                    "evidence": issue["evidence"],
                    "suggestion": issue["suggestion"],
                    "obligationId": issue.get("obligationId").cloned().unwrap_or(serde_json::Value::Null),
                    "violationType": issue.get("violationType").cloned().unwrap_or(serde_json::Value::Null),
                    "needsAuthorConfirmation": issue
                        .get("needsAuthorConfirmation")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null),
                    "sourceRef": issue.get("sourceRef").cloned().unwrap_or(serde_json::Value::Null),
                }));
            }
            let normalized = serde_json::json!({
                "issues": normalized_issues.len(),
                "issueDetails": normalized_issues,
                "status": if normalized_issues.is_empty() { "clean" } else { "issues_found" },
                "reviewOutcome": if normalized_issues.is_empty() {
                    "review_succeeded_no_issues"
                } else {
                    "review_succeeded_with_issues"
                },
            });
            return serde_json::to_string(&normalized).map_err(|_| "审校结果无法序列化".into());
        }
        _ => return Err("未知的结构化步骤".into()),
    }
    serde_json::to_string(&value).map_err(|_| "结构化产物无法序列化".into())
}

#[cfg(test)]
mod tests {
    use super::{
        ArtifactView, context_payload, validate_chapter_plan_references,
        validate_structured_output, validate_structured_output_with,
    };

    #[test]
    fn story_macro_normalizes_mystery_box_array_to_string() {
        let raw = serde_json::json!({
            "expansion": {
                "expanded_premise": "前提",
                "protagonist_core": "主角",
                "conflict_engine": "冲突",
                "conflict_layers": { "external": "外部压力" },
                "mystery_box": ["谜一", "谜二"],
                "emotional_line": "成长",
                "setpiece_seeds": ["场面"],
                "tone_reference": "冷峻"
            },
            "decomposition": {
                "selling_point": "卖点",
                "core_conflict": "冲突",
                "main_hook": "钩子",
                "progression_loop": "循环",
                "growth_path": "成长",
                "ending_flavor": "结局"
            },
            "constraints": [],
            "issues": []
        })
        .to_string();

        let output = validate_structured_output("story_macro", &raw).expect("story macro 应通过");
        let value: serde_json::Value = serde_json::from_str(&output).expect("输出应是 JSON");
        assert_eq!(value["expansion"]["mystery_box"], "谜一；谜二");
    }

    #[test]
    fn story_macro_coerces_common_type_deviations() {
        let raw = serde_json::json!({
            "expansion": {
                "expanded_premise": ["前提一", "前提二"],
                "protagonist_core": "主角",
                "conflict_engine": "冲突",
                "conflict_layers": "外部压力",
                "mystery_box": "谜一",
                "emotional_line": "成长",
                "setpiece_seeds": "场面",
                "tone_reference": "冷峻"
            },
            "decomposition": {
                "selling_point": ["卖点一", "卖点二"],
                "core_conflict": "冲突",
                "main_hook": "钩子",
                "progression_loop": "循环",
                "growth_path": "成长",
                "ending_flavor": "结局"
            },
            "constraints": [],
            "issues": []
        })
        .to_string();

        let output = validate_structured_output("story_macro", &raw).expect("story macro 应通过");
        let value: serde_json::Value = serde_json::from_str(&output).expect("输出应是 JSON");
        assert_eq!(value["expansion"]["expanded_premise"], "前提一；前提二");
        assert_eq!(
            value["expansion"]["conflict_layers"]["external"],
            "外部压力"
        );
        assert_eq!(value["expansion"]["setpiece_seeds"][0], "场面");
        assert_eq!(value["decomposition"]["selling_point"], "卖点一；卖点二");
    }

    #[test]
    fn story_macro_normalizes_camel_case_fields() {
        let raw = serde_json::json!({
            "expansion": {
                "expandedPremise": "前提",
                "protagonistCore": "主角",
                "conflictEngine": "冲突",
                "conflictLayers": { "external": "外部压力" },
                "mysteryBox": ["谜一", "谜二"],
                "emotionalLine": "成长",
                "setpieceSeeds": ["场面"],
                "toneReference": "冷峻"
            },
            "decomposition": {
                "sellingPoint": "卖点",
                "coreConflict": "冲突",
                "mainHook": "钩子",
                "progressionLoop": "循环",
                "growthPath": "成长",
                "endingFlavor": "结局"
            },
            "constraints": [],
            "issues": []
        })
        .to_string();

        let output = validate_structured_output("story_macro", &raw).expect("story macro 应通过");
        let value: serde_json::Value = serde_json::from_str(&output).expect("输出应是 JSON");
        assert_eq!(value["expansion"]["mystery_box"], "谜一；谜二");
        assert_eq!(value["expansion"]["expanded_premise"], "前提");
        assert_eq!(value["decomposition"]["selling_point"], "卖点");
    }

    fn story_macro_without_key_events() -> String {
        serde_json::json!({
            "expansion": {
                "expanded_premise": "前提", "protagonist_core": "主角",
                "conflict_engine": "冲突", "conflict_layers": {"external": "外部压力"},
                "mystery_box": "谜", "emotional_line": "成长",
                "setpiece_seeds": ["场面"], "tone_reference": "冷峻"
            },
            "decomposition": {
                "selling_point": "卖点", "core_conflict": "冲突", "main_hook": "钩子",
                "progression_loop": "循环", "growth_path": "成长", "ending_flavor": "结局"
            },
            "constraints": [],
            "issues": []
        })
        .to_string()
    }

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

    #[test]
    fn strict_story_macro_requires_key_events() {
        let raw = story_macro_without_key_events();
        assert!(validate_structured_output("story_macro", &raw).is_ok());
        let error = validate_structured_output_with("story_macro", &raw, true).unwrap_err();
        assert!(error.contains("缺少 keyEvents"), "{error}");
    }

    #[test]
    fn strict_story_macro_requires_a_hard_key_event() {
        let mut value: serde_json::Value =
            serde_json::from_str(&story_macro_without_key_events()).unwrap();
        value["keyEvents"] = serde_json::json!([{
            "chapterOrder": 1, "type": "continuity_constraint",
            "description": "保持雪夜氛围", "enforcement": "advisory"
        }]);
        let error =
            validate_structured_output_with("story_macro", &value.to_string(), true).unwrap_err();
        assert!(error.contains("至少需要一条 hard"), "{error}");
    }

    #[test]
    fn story_macro_key_events_are_normalized() {
        let raw = serde_json::json!({
            "expansion": {
                "expanded_premise": "前提",
                "protagonist_core": "主角",
                "conflict_engine": "冲突",
                "conflict_layers": { "external": "外部压力" },
                "mystery_box": "谜",
                "emotional_line": "成长",
                "setpiece_seeds": ["场面"],
                "tone_reference": "冷峻"
            },
            "decomposition": {
                "selling_point": "卖点",
                "core_conflict": "冲突",
                "main_hook": "钩子",
                "progression_loop": "循环",
                "growth_path": "成长",
                "ending_flavor": "结局"
            },
            "keyEvents": [{
                "chapterOrder": 1,
                "type": "requiredEvent",
                "description": "林动亲眼撞见老铁匠被灭口",
                "participants": ["林动", "老铁匠"]
            }],
            "constraints": [],
            "issues": []
        })
        .to_string();
        let output = validate_structured_output("story_macro", &raw).expect("story macro 应通过");
        let value: serde_json::Value = serde_json::from_str(&output).expect("JSON");
        assert_eq!(value["keyEvents"][0]["id"], "ch1-event-1");
        assert_eq!(value["keyEvents"][0]["type"], "required_event");
    }

    #[test]
    fn empty_story_macro_key_events_are_rejected() {
        let raw = serde_json::json!({
            "expansion": {
                "expanded_premise": "前提", "protagonist_core": "主角",
                "conflict_engine": "冲突", "conflict_layers": {"external": "外部压力"},
                "mystery_box": "谜", "emotional_line": "成长",
                "setpiece_seeds": ["场面"], "tone_reference": "冷峻"
            },
            "decomposition": {
                "selling_point": "卖点", "core_conflict": "冲突", "main_hook": "钩子",
                "progression_loop": "循环", "growth_path": "成长", "ending_flavor": "结局"
            },
            "keyEvents": [],
            "constraints": [],
            "issues": []
        })
        .to_string();
        let error = validate_structured_output("story_macro", &raw).unwrap_err();
        assert!(error.contains("不能为空"), "{error}");
    }

    #[test]
    fn chapter_plan_must_cover_hard_story_key_events() {
        let cast = serde_json::json!({"members": [{"name": "林动"}]});
        let volume = serde_json::json!({"volumes": [{"title": "卷一"}]});
        let macro_payload = serde_json::json!({
            "keyEvents": [{
                "id": "ch1-event-1",
                "chapterOrder": 1,
                "type": "required_event",
                "description": "林动亲眼撞见老铁匠被灭口",
                "participants": ["林动", "老铁匠"],
                "enforcement": "hard"
            }]
        });
        let covered = serde_json::json!({
            "chapters": [{
                "chapterOrder": 1, "title": "第一章", "volumeTitle": "卷一",
                "summary": "林动撞见灭口", "characterNames": ["林动"],
                "obligations": [{
                    "id": "ch1-req-1", "chapterOrder": 1, "type": "required_event",
                    "description": "林动亲眼撞见老铁匠被灭口", "enforcement": "hard",
                    "participants": ["林动", "老铁匠"],
                    "sourceEventId": "ch1-event-1"
                }]
            }]
        })
        .to_string();
        assert!(
            validate_chapter_plan_references(&covered, &cast, &volume, &macro_payload, true)
                .is_ok()
        );

        let missing = covered.replace(",\"sourceEventId\":\"ch1-event-1\"", "");
        let error =
            validate_chapter_plan_references(&missing, &cast, &volume, &macro_payload, true)
                .unwrap_err();
        assert!(error.to_string().contains("未覆盖"), "{error}");

        let unknown = covered.replace("ch1-event-1", "missing-event");
        let error =
            validate_chapter_plan_references(&unknown, &cast, &volume, &macro_payload, true)
                .unwrap_err();
        assert!(error.to_string().contains("不存在"), "{error}");
    }

    fn canonical_event_plan(
        description: &str,
        participants: serde_json::Value,
        enforcement: &str,
    ) -> String {
        serde_json::json!({
            "chapters": [{
                "chapterOrder": 1, "title": "第一章", "volumeTitle": "卷一",
                "summary": "林动撞见灭口", "characterNames": ["林动"],
                "obligations": [{
                    "id": "ch1-req-1", "chapterOrder": 1, "type": "required_event",
                    "description": description, "enforcement": enforcement,
                    "participants": participants, "sourceEventId": "ch1-event-1"
                }]
            }]
        })
        .to_string()
    }

    #[test]
    fn source_event_content_cannot_be_rewritten() {
        let cast = serde_json::json!({"members": [{"name": "林动"}, {"name": "老铁匠"}]});
        let volume = serde_json::json!({"volumes": [{"title": "卷一"}]});
        let macro_payload = serde_json::json!({
            "keyEvents": [{
                "id": "ch1-event-1", "chapterOrder": 1, "type": "required_event",
                "description": "林动亲眼撞见老铁匠被灭口",
                "participants": ["林动", "老铁匠"], "enforcement": "hard"
            }]
        });
        let good = canonical_event_plan(
            "林动亲眼撞见老铁匠被灭口",
            serde_json::json!(["林动", "老铁匠"]),
            "hard",
        );
        assert!(
            validate_chapter_plan_references(&good, &cast, &volume, &macro_payload, true).is_ok()
        );

        let opposite = canonical_event_plan(
            "林动到达时老铁匠已经死亡",
            serde_json::json!(["林动", "老铁匠"]),
            "hard",
        );
        let error =
            validate_chapter_plan_references(&opposite, &cast, &volume, &macro_payload, true)
                .unwrap_err();
        assert!(error.to_string().contains("描述被改写"), "{error}");

        let replaced = canonical_event_plan(
            "林动亲眼撞见老铁匠被灭口",
            serde_json::json!(["林动", "苏晚"]),
            "hard",
        );
        let error =
            validate_chapter_plan_references(&replaced, &cast, &volume, &macro_payload, true)
                .unwrap_err();
        assert!(error.to_string().contains("参与者被替换"), "{error}");

        let downgraded = canonical_event_plan(
            "林动亲眼撞见老铁匠被灭口",
            serde_json::json!(["林动", "老铁匠"]),
            "advisory",
        );
        let error =
            validate_chapter_plan_references(&downgraded, &cast, &volume, &macro_payload, true)
                .unwrap_err();
        assert!(error.to_string().contains("降级"), "{error}");
    }

    #[test]
    fn future_chapter_source_event_is_rejected() {
        let cast = serde_json::json!({"members": [{"name": "林动"}]});
        let volume = serde_json::json!({"volumes": [{"title": "卷一"}]});
        let macro_payload = serde_json::json!({
            "keyEvents": [{
                "id": "ch2-event-1", "chapterOrder": 2, "type": "required_event",
                "description": "第二章才发生的伏击",
                "participants": ["林动"], "enforcement": "hard"
            }]
        });
        let plan = serde_json::json!({
            "chapters": [{
                "chapterOrder": 1, "title": "第一章", "volumeTitle": "卷一",
                "summary": "第一章", "characterNames": ["林动"],
                "obligations": [{
                    "id": "ch1-req-1", "chapterOrder": 1, "type": "required_event",
                    "description": "第二章才发生的伏击", "enforcement": "hard",
                    "participants": ["林动"], "sourceEventId": "ch2-event-1"
                }]
            }]
        })
        .to_string();
        let error = validate_chapter_plan_references(&plan, &cast, &volume, &macro_payload, true)
            .unwrap_err();
        assert!(error.to_string().contains("其他章节"), "{error}");
    }

    #[test]
    fn chapter_list_normalizes_obligations_and_assigns_stable_ids() {
        let raw = serde_json::json!({
            "chapters": [{
                "chapterOrder": 1,
                "title": "第一章",
                "volumeTitle": "卷一",
                "summary": "林动撞见灭口",
                "characterNames": ["林动"],
                "obligations": [{
                    "type": "requiredEvent",
                    "description": "林动亲眼撞见老铁匠被杀",
                    "participants": ["林动", "老铁匠"],
                    "evidence": ["第1章计划"]
                }]
            }]
        })
        .to_string();
        let output = validate_structured_output("chapter_list", &raw).expect("章节计划应通过");
        let value: serde_json::Value = serde_json::from_str(&output).expect("输出应是 JSON");
        let obligation = &value["chapters"][0]["obligations"][0];
        assert_eq!(obligation["id"], "ch1-req-1");
        assert_eq!(obligation["chapterOrder"], 1);
        assert_eq!(obligation["type"], "required_event");
        assert_eq!(obligation["enforcement"], "hard");
        assert_eq!(obligation["participants"][0], "林动");
    }

    #[test]
    fn generated_chapter_list_without_obligations_is_rejected() {
        let raw = serde_json::json!({
            "chapters": [{
                "chapterOrder": 1,
                "title": "第一章",
                "volumeTitle": "卷一",
                "summary": "入山",
                "characterNames": ["林动"]
            }]
        })
        .to_string();
        let error = validate_structured_output("chapter_list", &raw).unwrap_err();
        assert!(
            error.contains("缺少 obligations"),
            "new plans must not silently fall back to the legacy contract: {error}"
        );
    }

    #[test]
    fn generated_chapter_list_with_invalid_obligations_is_rejected() {
        let wrong_type = serde_json::json!({
            "chapters": [{
                "chapterOrder": 1,
                "title": "第一章",
                "volumeTitle": "卷一",
                "summary": "入山",
                "characterNames": ["林动"],
                "obligations": "林动必须入山"
            }]
        })
        .to_string();
        assert!(validate_structured_output("chapter_list", &wrong_type).is_err());

        let empty = serde_json::json!({
            "chapters": [{
                "chapterOrder": 1,
                "title": "第一章",
                "volumeTitle": "卷一",
                "summary": "入山",
                "characterNames": ["林动"],
                "obligations": []
            }]
        })
        .to_string();
        let error = validate_structured_output("chapter_list", &empty).unwrap_err();
        assert!(
            error.contains("没有 hard 义务"),
            "an empty obligation list is not a valid v2 plan: {error}"
        );

        let all_advisory = serde_json::json!({
            "chapters": [{
                "chapterOrder": 1,
                "title": "第一章",
                "volumeTitle": "卷一",
                "summary": "入山",
                "characterNames": ["林动"],
                "obligations": [{
                    "type": "continuity_constraint",
                    "description": "保持文风一致",
                    "enforcement": "advisory"
                }]
            }]
        })
        .to_string();
        let error = validate_structured_output("chapter_list", &all_advisory).unwrap_err();
        assert!(error.contains("没有 hard 义务"), "{error}");
    }

    #[test]
    fn historical_plan_without_obligations_remains_readable() {
        let plan = serde_json::json!({
            "chapters": [{
                "chapterOrder": 1,
                "title": "第一章",
                "volumeTitle": "卷一",
                "summary": "入山",
                "characterNames": ["林动"]
            }]
        });
        assert!(crate::obligations::parse_obligations(&plan["chapters"][0]).is_empty());
    }
}
