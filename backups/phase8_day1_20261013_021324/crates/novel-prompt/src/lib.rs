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
    &[PromptAsset {
        id: "chapter_draft",
        version: "chapter-draft-v1",
        family: "chapter",
        template: "根据下面的章节目标写正文，不要输出解释。\n{{brief}}",
    }]
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
        assert!(resolve("chapter_draft", "chapter-draft-v2").is_err());
        let asset = resolve("chapter_draft", "chapter-draft-v1").unwrap();
        let mut vars = BTreeMap::new();
        vars.insert("brief".into(), "林动入山".into());
        assert!(render(asset.template, &vars).unwrap().contains("林动入山"));
        assert!(render(asset.template, &BTreeMap::new()).is_err());
    }
}
