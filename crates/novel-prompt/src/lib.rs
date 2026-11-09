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
            id: "chapter_draft",
            version: "chapter-draft-v3",
            family: "chapter",
            template: "STEP:chapter_draft\n你是小说作者。必须严格依据下面已经确认的上下文写本章正文，不要引入与上下文冲突的事实，不要输出解释。\n章节目标：{{brief}}\n已确认上下文：\n{{context}}\n章节义务：\n{{obligations}}\n硬约束必须遵守，不得改写关键事件、参与者、因果与顺序；创作建议可以合理取舍，但不能为了满足建议而破坏硬约束或已确认事实。",
        },
        PromptAsset {
            id: "story_macro",
            version: "story-macro-v1",
            family: "director",
            template: "STEP:story_macro\n你是资深小说作者与剧情策划。根据故事输入输出严格 JSON：expansion（expanded_premise、protagonist_core、conflict_engine、conflict_layers、mystery_box、emotional_line、setpiece_seeds、tone_reference）、decomposition（selling_point、core_conflict、main_hook、progression_loop、growth_path、major_payoffs、ending_flavor）、constraints、issues。不要输出 Markdown。\n故事输入：{{story_input}}",
        },
        PromptAsset {
            id: "story_macro",
            version: "story-macro-v2",
            family: "director",
            template: "STEP:story_macro\n你是资深小说作者与剧情策划。根据故事输入输出严格 JSON：expansion（expanded_premise、protagonist_core、conflict_engine、conflict_layers、mystery_box、emotional_line、setpiece_seeds、tone_reference）、decomposition（selling_point、core_conflict、main_hook、progression_loop、growth_path、major_payoffs、ending_flavor）、keyEvents、constraints、issues。keyEvents 是跨章节必须发生的关键事件数组，不能为空且至少一条 hard；每项包含 id、chapterOrder、type、description、participants、enforcement；type 只能是 required_event、forbidden_event、participant、causal_constraint、ordering_constraint、continuity_constraint；enforcement 只能是 hard 或 advisory；id 全书唯一。必须把 constraints 中明确指定章节的关键事件写成 chapterOrder 对应的 keyEvent，例如“第1章必须……”必须有一条 chapterOrder=1 的 required_event；不得把同一事件写到多个章节，不得写与 constraints 相反的事件。不要输出 Markdown。\n故事输入：{{story_input}}",
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
            id: "chapter_list",
            version: "chapter-list-v2",
            family: "director",
            template: "STEP:chapter_list\n依据 story_macro、卷策略与人物阵容输出严格 JSON：chapters（chapterOrder 从 1 递增且唯一，每章包含 title、volumeTitle、summary、characterNames、obligations）。obligations 是本章必须遵守的结构化义务数组，不能省略且每章至少一条 hard 义务；每项包含 id、type、description、enforcement、participants、source、evidence。type 只能是 required_event、forbidden_event、participant、causal_constraint、ordering_constraint、continuity_constraint；enforcement 只能是 hard 或 advisory。required_event 写本章必须发生的关键事件；forbidden_event 写不得提前或错误发生的事件；causal_constraint 写关键因果；ordering_constraint 写关键顺序；continuity_constraint 写已确认历史事实约束。id 使用 ch{chapterOrder}-{类型缩写}-{序号}，全书唯一。只把会改变剧情因果、参与者或时间顺序的要求写成 hard；普通文学描写写成 advisory 或不要写成义务。不得引用未来章节才揭示的事实。章节必须属于已规划的卷，人物引用必须来自人物阵容。\n{{context}}",
        },
        PromptAsset {
            id: "chapter_list",
            version: "chapter-list-v3",
            family: "director",
            template: "STEP:chapter_list\n依据 story_macro、卷策略与人物阵容输出严格 JSON：chapters（chapterOrder 从 1 递增且唯一，每章包含 title、volumeTitle、summary、characterNames、obligations）。obligations 是本章必须遵守的结构化义务数组，不能省略且每章至少一条 hard 义务；每项包含 id、type、description、enforcement、participants、source、evidence、sourceEventId。先读取 story_macro.keyEvents：对每一条 chapterOrder 等于本章的 hard 关键事件，必须生成一条同类型 hard 义务，sourceEventId 填该事件 id；不得省略、不得改写成相反含义（例如 macro 要求本章目击灭口，不得写成“尚未目击”或“到达时已死”）；不属于本章的 keyEvents 不得写入本章。义务的 description、participants、type 必须与 keyEvent 完全一致（可补充 evidence/source，但不得改写核心描述或替换参与者）。summary 必须与 obligations 一致，summary 中出现的关键事件必须在 required_event 中。如果 story_macro 没有 keyEvents，按本章摘要生成义务。type 只能是 required_event、forbidden_event、participant、causal_constraint、ordering_constraint、continuity_constraint；enforcement 只能是 hard 或 advisory。id 使用 ch{chapterOrder}-{类型缩写}-{序号}，全书唯一。只把会改变剧情因果、参与者或时间顺序的要求写成 hard；普通文学描写写成 advisory 或不要写成义务。不得引用未来章节才揭示的事实。章节必须属于已规划的卷，人物引用必须来自人物阵容。\n{{context}}",
        },
        PromptAsset {
            id: "structured_repair",
            version: "structured-repair-v1",
            family: "director",
            template: "STEP:structured_repair\n你是结构化输出修复器。下面包含原始任务要求、上一次模型输出和校验错误。请只输出修复后的完整 JSON 对象，不要 Markdown、不要解释、不要省略任何必填字段。如果原输出被截断，请补全缺失部分；如果字段类型不符，请按校验错误修正。\n本步骤硬性要求：{{step_requirement}}\n\n原始任务要求：\n{{original_prompt}}\n\n校验错误：{{error}}\n\n原始输出：\n{{raw}}",
        },
        PromptAsset {
            id: "chapter_repair",
            version: "chapter-repair-v1",
            family: "director",
            template: "STEP:chapter_repair\n你是小说修订编辑。根据审校问题和已确认上下文修订候选正文。只输出修订后的完整正文，不要解释，不要 Markdown。\n已确认上下文：\n{{context}}\n审校问题：\n{{issues}}\n待修订候选：\n{{candidate}}",
        },
        PromptAsset {
            id: "chapter_repair",
            version: "chapter-repair-v2",
            family: "director",
            template: "STEP:chapter_repair\n你是小说修订编辑。根据审校问题、已确认上下文和本章章节义务修订候选正文。只输出修订后的完整正文，不要解释，不要 Markdown。硬约束必须保留：不得改写硬约束中的关键事件、参与者、因果与顺序，也不得通过修改计划来迁就正文；创作建议可以合理取舍，但不得为满足建议而破坏硬约束或已确认事实。\n本章章节义务：\n{{obligations}}\n已确认上下文：\n{{context}}\n审校问题：\n{{issues}}\n待修订候选：\n{{candidate}}",
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
        PromptAsset {
            id: "consistency_review",
            version: "consistency-review-v2",
            family: "director",
            template: "STEP:consistency_review\n审校章节正文与规划。只输出严格 JSON：issues（数组，每项含 type、severity、evidence、suggestion；如违反结构化章节义务，必须填写 obligationId，且只能使用上下文中列出的义务 ID；必要时填写 violationType、enforcement、confidence、needsAuthorConfirmation；无问题用空数组）。evidence 必须逐字引用当前候选正文，不得引用历史正文。必须完整阅读候选正文后再判断 missing_element：局部引文只能证明引文存在，不能证明全文缺少要素；如果无法确认全文缺席，confidence 写 suspected_absence、needsAuthorConfirmation=true，并保留问题，不能返回 clean。如果全文明确出现与 hard 义务直接矛盾的事实，confidence 写 confirmed_violation；如果解释或证据不足以支持结论，confidence 写 insufficient_evidence、needsAuthorConfirmation=true。一般质量建议必须写 enforcement=advisory，不得触发 hard repair；hard 义务不能被模型自行降级。没有采用 advisory 建议不构成问题。不要把调用失败或无法判断当作没有问题；不得编造 obligationId。不要返回 Markdown。\n{{context}}",
        },
        PromptAsset {
            id: "chapter_selection_rewrite",
            version: "chapter-selection-rewrite-v1",
            family: "chapter",
            template: "STEP:chapter_selection_rewrite\n你是中文网络小说章节编辑器里的局部改写助手。围绕作者选中的一段正文，给出 2 到 3 个可直接比较的候选改写版本。\n\n任务边界：\n1. 只改写选中片段，不要重写整章，不要输出片段以外的正文。\n2. 改写必须贴合前后文语气、人物状态与本章目标。\n3. 不要解释过程，不要输出 Markdown，不要输出候选以外的额外文本。\n\n硬性约束：\n1. 不改剧情事实。\n2. 不改变人称与叙事视角。\n3. 不新增未授权设定。\n4. 保留原片段的核心信息与上下文承接。\n5. 不要写成模板化 AI 文风。\n\n候选要求：\n1. 返回 2 到 3 个候选，每个都是完整可替换的片段文本。\n2. label 要短，适合在编辑器里切换候选。\n3. summary 用一句话概括主要改动。\n4. rationale 用一句话说明这版的主要改法。\n5. riskNotes 列出 0 到 3 条需要作者注意的风险。\n6. semanticTags 保留 2 到 4 个高价值标签，例如“增强情绪”“压缩重复”。\n\n只输出严格 JSON：{\"candidates\":[{\"label\":\"\",\"content\":\"\",\"summary\":\"\",\"rationale\":\"\",\"riskNotes\":[],\"semanticTags\":[]}],\"macroAlignmentNote\":\"\"}\n\n本次改写意图：{{operation_label}}\n改写范围：{{scope_label}}\n作者补充要求：{{instruction}}\n章节标题：{{chapter_title}}\n前文：\n{{before_context}}\n待改写片段：\n{{selected_text}}\n后文：\n{{after_context}}",
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
        assert!(resolve("chapter_draft", "chapter-draft-v4").is_err());
        let asset = resolve("chapter_draft", "chapter-draft-v1").unwrap();
        let mut vars = BTreeMap::new();
        vars.insert("brief".into(), "林动入山".into());
        assert!(render(asset.template, &vars).unwrap().contains("林动入山"));
        assert!(render(asset.template, &BTreeMap::new()).is_err());
    }

    /// 选区改写的模板必须把「只改选中片段」和候选数量写进提示词本身：
    /// 这两条是正文安全边界，不能只靠调用方的参数约定。
    #[test]
    fn selection_rewrite_template_requires_every_context_slot() {
        let asset = resolve("chapter_selection_rewrite", "chapter-selection-rewrite-v1").unwrap();
        assert_eq!(asset.family, "chapter");
        assert!(asset.template.contains("只改写选中片段"));
        assert!(asset.template.contains("2 到 3 个"));

        let mut vars = BTreeMap::new();
        for key in [
            "operation_label",
            "scope_label",
            "instruction",
            "selected_text",
            "before_context",
            "after_context",
            "chapter_title",
        ] {
            vars.insert(key.to_string(), format!("<{key}>"));
        }
        let rendered = render(asset.template, &vars).unwrap();
        assert!(rendered.contains("<selected_text>"));
        assert!(rendered.contains("<before_context>"));
        assert!(rendered.contains("<after_context>"));

        // 少任何一个变量都必须报错，避免把 "{{selected_text}}" 原样发给模型。
        for key in ["selected_text", "before_context", "operation_label"] {
            let mut incomplete = vars.clone();
            incomplete.remove(key);
            assert!(
                render(asset.template, &incomplete).is_err(),
                "缺少 {key} 时必须拒绝渲染"
            );
        }
    }

    /// 已暂停的历史 Run 可能冻结在 v2：v2 模板不能被本轮修改改写。
    #[test]
    fn legacy_draft_v2_still_renders_without_an_obligations_slot() {
        let asset = resolve("chapter_draft", "chapter-draft-v2").unwrap();
        assert!(!asset.template.contains("{{obligations}}"));
        let mut vars = BTreeMap::new();
        vars.insert("brief".into(), "本章目标".into());
        vars.insert("context".into(), "已确认上下文".into());
        let rendered = render(asset.template, &vars).unwrap();
        assert!(rendered.contains("已确认上下文"));
    }

    #[test]
    fn planner_prompts_cover_story_key_events() {
        let story = resolve("story_macro", "story-macro-v2").unwrap();
        assert!(story.template.contains("keyEvents"));
        assert!(story.template.contains("chapterOrder"));

        let chapter = resolve("chapter_list", "chapter-list-v3").unwrap();
        assert!(chapter.template.contains("keyEvents"));
        assert!(chapter.template.contains("sourceEventId"));
        assert!(chapter.template.contains("不得改写成相反含义"));

        assert!(resolve("story_macro", "story-macro-v1").is_ok());
        assert!(resolve("chapter_list", "chapter-list-v2").is_ok());
    }

    #[test]
    fn free_draft_v1_remains_a_single_prompt_without_obligations() {
        let asset = resolve("chapter_draft", "chapter-draft-v1").unwrap();
        assert!(!asset.template.contains("{{context}}"));
        assert!(!asset.template.contains("obligations"));
        let mut vars = BTreeMap::new();
        vars.insert("brief".into(), "自由创作".into());
        let rendered = render(asset.template, &vars).unwrap();
        assert!(rendered.contains("自由创作"));
    }

    #[test]
    fn obligation_aware_prompts_are_versioned_and_require_the_slot() {
        assert!(resolve("chapter_list", "chapter-list-v1").is_ok());
        assert!(resolve("chapter_list", "chapter-list-v2").is_ok());
        assert!(resolve("consistency_review", "consistency-review-v2").is_ok());
        assert!(resolve("chapter_repair", "chapter-repair-v2").is_ok());

        let repair = resolve("chapter_repair", "chapter-repair-v2").unwrap();
        let mut vars = BTreeMap::new();
        vars.insert("context".into(), "上下文".into());
        vars.insert("issues".into(), "[]".into());
        vars.insert("candidate".into(), "候选".into());
        assert!(
            render(repair.template, &vars).is_err(),
            "missing obligations must not render with an empty slot"
        );
        vars.insert("obligations".into(), "[ch1-req-1] 硬约束".into());
        let rendered = render(repair.template, &vars).unwrap();
        assert!(rendered.contains("ch1-req-1"));
        assert!(rendered.contains("硬约束必须保留"));
        assert!(rendered.contains("创作建议可以合理取舍"));

        let review = resolve("consistency_review", "consistency-review-v2").unwrap();
        assert!(review.template.contains("advisory"));
        assert!(review.template.contains("不构成问题"));

        let draft = resolve("chapter_draft", "chapter-draft-v3").unwrap();
        let mut vars = BTreeMap::new();
        vars.insert("brief".into(), "本章目标".into());
        vars.insert("context".into(), "已确认上下文".into());
        assert!(
            render(draft.template, &vars).is_err(),
            "v3 must not silently render without the obligations slot"
        );
        vars.insert("obligations".into(), "【硬约束】ch1-req-1".into());
        let rendered = render(draft.template, &vars).unwrap();
        assert!(rendered.contains("ch1-req-1"));
        assert!(rendered.contains("创作建议可以合理取舍"));
    }
}
