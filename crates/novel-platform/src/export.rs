use std::io::Write;
use std::path::Path;

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

/// Write UTF-8 text to a user-selected path using a sibling temp file.
/// The caller must only pass a path returned by the native save dialog.
pub fn write_text_atomically(target: &Path, content: &str) -> Result<(), String> {
    let parent = target
        .parent()
        .ok_or_else(|| "保存路径无效。".to_string())?;
    let file_name = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "保存文件名无效。".to_string())?;
    let temp = parent.join(format!(".{file_name}.tmp-{}", std::process::id()));
    {
        let mut file = std::fs::File::create(&temp)
            .map_err(|error| format!("无法创建导出临时文件：{error}"))?;
        file.write_all(content.as_bytes())
            .map_err(|error| format!("写入导出文件失败：{error}"))?;
        file.flush()
            .map_err(|error| format!("刷新导出文件失败：{error}"))?;
        file.sync_all()
            .map_err(|error| format!("同步导出文件失败：{error}"))?;
    }
    if let Err(error) = std::fs::rename(&temp, target) {
        let _ = std::fs::remove_file(&temp);
        return Err(format!("保存导出文件失败：{error}"));
    }
    Ok(())
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
    fn atomic_export_writes_utf8_and_replaces_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("小说.md");
        write_text_atomically(&target, "第一章：林动入山\n").unwrap();
        write_text_atomically(&target, "第一章：林动入山\n第二章：夜雨\n").unwrap();
        assert_eq!(
            std::fs::read_to_string(&target).unwrap(),
            "第一章：林动入山\n第二章：夜雨\n"
        );
        assert!(
            !dir.path()
                .join(format!(".小说.md.tmp-{}", std::process::id()))
                .exists()
        );
        assert!(write_text_atomically(&dir.path().join("missing/小说.md"), "x").is_err());
    }

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
