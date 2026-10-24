use std::collections::BTreeMap;

use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PromptAsset {
    pub id: &'static str,
    pub version: &'static str,
    pub family: &'static str,
    pub template: &'static str,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PromptError {
    #[error("缺少提示词变量 {0}")]
    Missing(String),
    #[error("提示词模板没有闭合")]
    Unclosed,
    #[error("没有这个版本的提示词")]
    UnknownVersion,
}

pub fn builtin_prompts() -> &'static [PromptAsset] {
    &[
        PromptAsset {
            id: "chapter_draft",
            version: "chapter-draft-v1",
            family: "chapter",
            template: "根据下面的章节目标写正文，不要输出解释。\n{{brief}}",
        },
        PromptAsset {
            id: "chapter_draft",
            version: "chapter-draft-v2",
            family: "chapter",
            template: "STEP:chapter_draft\n你是小说作者。必须严格依据下面已经确认的上下文写本章正文，不要引入与上下文冲突的事实，不要输出解释。\n章节目标：{{brief}}\n已确认上下文：\n{{context}}",
        },
        PromptAsset {
            id: "story_macro",
            version: "story-macro-v1",
            family: "director",
            template: "STEP:story_macro\n你是资深小说作者与剧情策划。根据故事输入输出严格 JSON：expansion（expanded_premise、protagonist_core、conflict_engine、conflict_layers、mystery_box、emotional_line、setpiece_seeds、tone_reference）、decomposition（selling_point、core_conflict、main_hook、progression_loop、growth_path、major_payoffs、ending_flavor）、constraints、issues。不要输出 Markdown。\n故事输入：{{story_input}}",
        },
        PromptAsset {
            id: "character_cast",
            version: "character-cast-v1",
            family: "director",
            template: "STEP:character_cast\n依据 Story Macro 输出严格 JSON：members（至少 3 人，每人包含 name、role、castRole、storyFunction、personality、outerGoal、innerNeed、relations 或关系字段）和 relations（至少 2 条，sourceName、targetName、surfaceRelation、conflictSource）。人物必须服务于主线冲突。\n{{context}}",
        },
        PromptAsset {
            id: "volume_strategy",
            version: "volume-strategy-v1",
            family: "director",
            template: "STEP:volume_strategy\n依据 Story Macro 和人物阵容输出严格 JSON：volumes（按 volumeOrder 排序，每卷包含 title、roleLabel、coreReward、planningMode：hard 或 soft）。至少 1 卷，不得重复序号。\n{{context}}",
        },
        PromptAsset {
            id: "chapter_list",
            version: "chapter-list-v1",
            family: "director",
            template: "STEP:chapter_list\n依据卷策略输出严格 JSON：chapters（chapterOrder 从 1 递增且唯一，每章包含 title、volumeTitle、summary、characterNames）。章节必须属于已规划的卷，人物引用必须来自人物阵容。\n{{context}}",
        },
        PromptAsset {
            id: "chapter_repair",
            version: "chapter-repair-v1",
            family: "director",
            template: "STEP:chapter_repair\n你是小说修订编辑。根据审校问题和已确认上下文修订候选正文。只输出修订后的完整正文，不要解释，不要 Markdown。\n已确认上下文：\n{{context}}\n审校问题：\n{{issues}}\n待修订候选：\n{{candidate}}",
        },
        PromptAsset {
            id: "memory_extraction",
            version: "memory-extraction-v1",
            family: "memory",
            template: "STEP:memory_extraction\n你是小说连续性编辑。只依据下面已确认的章节正文抽取长期记忆候选，不要补充正文没有的事实。严格输出 JSON：{\"facts\":[{\"factType\":\"character_state|timeline_event|plot_thread\",\"truthfulness\":\"narrator_fact|character_claim|rumor|hypothesis|plan|author_asserted\",\"entityName\":\"人物名或伏笔名\",\"value\":\"事实正文\",\"evidenceQuote\":\"必须逐字来自正文的原文片段\",\"confidence\":0.0}]}。传闻/人物宣称/推测/未来计划不得标为 narrator_fact。不要输出 Markdown。\n已知人物：{{characters}}\n章节标题：{{title}}\n章节正文：\n{{content}}",
        },
        PromptAsset {
            id: "consistency_review",
            version: "consistency-review-v1",
            family: "director",
            template: "STEP:consistency_review\n审校章节正文与规划。只输出严格 JSON：issues（数组，每项含 type、severity、evidence、suggestion；无问题用空数组）。不要返回 Markdown。不要把调用失败当作没有问题。\n{{context}}",
        },
    ]
}

pub fn resolve(id: &str, version: &str) -> Result<&'static PromptAsset, PromptError> {
    builtin_prompts()
        .iter()
        .find(|asset| asset.id == id && asset.version == version)
        .ok_or(PromptError::UnknownVersion)
}

pub fn render(template: &str, vars: &BTreeMap<String, String>) -> Result<String, PromptError> {
    let mut rest = template;
    let mut output = String::new();
    while let Some(start) = rest.find("{{") {
        output.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find("}}") else {
            return Err(PromptError::Unclosed);
        };
        let key = after[..end].trim();
        let value = vars
            .get(key)
            .ok_or_else(|| PromptError::Missing(key.to_string()))?;
        output.push_str(value);
        rest = &after[end + 2..];
    }
    output.push_str(rest);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_must_match_and_missing_variables_fail() {
        assert!(resolve("chapter_draft", "chapter-draft-v3").is_err());
        let asset = resolve("chapter_draft", "chapter-draft-v1").unwrap();
        let mut vars = BTreeMap::new();
        vars.insert("brief".into(), "林动入山".into());
        assert!(render(asset.template, &vars).unwrap().contains("林动入山"));
        assert!(render(asset.template, &BTreeMap::new()).is_err());
    }
}
