use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::error::LlmError;

pub fn parse_structured_json<T: DeserializeOwned>(text: &str) -> Result<T, LlmError> {
    let trimmed = text.trim();
    let json = strip_fence(trimmed);
    let value: Value = serde_json::from_str(json)
        .map_err(|_| LlmError::StructuredOutput("模型没有返回可读取的结构".into()))?;
    serde_json::from_value(value)
        .map_err(|_| LlmError::StructuredOutput("模型结构与当前步骤不一致".into()))
}

fn strip_fence(text: &str) -> &str {
    let Some(rest) = text.strip_prefix("```") else {
        return text;
    };
    let rest = rest.strip_prefix("json").unwrap_or(rest).trim_start();
    rest.strip_suffix("```").unwrap_or(rest).trim()
}

/// `failures` 是已经失败的次数。最多再修一次，不能无限重试。
pub fn may_repair(failures: u8, max_repairs: u8) -> bool {
    failures > 0 && failures <= max_repairs
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize, PartialEq)]
    #[serde(deny_unknown_fields, rename_all = "camelCase")]
    struct DraftPlan {
        title: String,
    }

    #[test]
    fn accepts_fenced_json_and_stops_after_one_repair() {
        let parsed: DraftPlan =
            parse_structured_json("```json\n{\"title\":\"开端\"}\n```").unwrap();
        assert_eq!(parsed.title, "开端");
        assert!(parse_structured_json::<DraftPlan>("{\"title\":\"开端\",\"extra\":1}").is_err());
        assert!(may_repair(1, 1));
        assert!(!may_repair(2, 1));
        assert!(!may_repair(0, 1));
    }
}
