use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportChapter {
    pub title: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportNovel {
    pub title: String,
    pub description: String,
    pub chapters: Vec<ExportChapter>,
}

pub fn novel_json(novel: &ExportNovel) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(novel)
}

pub fn chapter_markdown(novel: &ExportNovel) -> String {
    let mut output = format!("# {}\n\n{}\n", novel.title, novel.description);
    for chapter in &novel.chapters {
        output.push_str(&format!("\n## {}\n\n{}\n", chapter.title, chapter.content));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_keeps_chapter_text_and_omits_secrets() {
        let novel = ExportNovel {
            title: "山海".into(),
            description: "一个开始".into(),
            chapters: vec![ExportChapter {
                title: "第一章".into(),
                content: "林动入山".into(),
            }],
        };
        let json = novel_json(&novel).unwrap();
        assert!(json.contains("林动入山"));
        assert!(!json.contains("sk-"));
        assert!(chapter_markdown(&novel).contains("## 第一章"));
    }
}
