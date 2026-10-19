//! 阶段 2 引入的 setup 领域类型。
//!
//! 这里是纯数据形状，不引用 Tauri / SQLx / HTTP / 系统时间。
//! 调用方负责把原始字段填进来，验证在应用层做。

use serde::{Deserialize, Serialize};

/// 创作经验：新手走"简易"，老手走"专业"。这一项决定主工作台默认展示哪一档内容。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CreationExperience {
    Simple,
    Professional,
}

impl CreationExperience {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Simple => "simple",
            Self::Professional => "professional",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "simple" => Some(Self::Simple),
            "professional" => Some(Self::Professional),
            _ => None,
        }
    }
}

/// 写作模式：原创 vs 续写。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WritingMode {
    Original,
    Continuation,
}

impl WritingMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Original => "original",
            Self::Continuation => "continuation",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "original" => Some(Self::Original),
            "continuation" => Some(Self::Continuation),
            _ => None,
        }
    }
}

/// 项目驱动模式：AI 全权 / AI 协作 / 草稿 / 自动流水线。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectMode {
    AiLed,
    CoPilot,
    DraftMode,
    AutoPipeline,
}

impl ProjectMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AiLed => "ai_led",
            Self::CoPilot => "co_pilot",
            Self::DraftMode => "draft_mode",
            Self::AutoPipeline => "auto_pipeline",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "ai_led" => Some(Self::AiLed),
            "co_pilot" => Some(Self::CoPilot),
            "draft_mode" => Some(Self::DraftMode),
            "auto_pipeline" => Some(Self::AutoPipeline),
            _ => None,
        }
    }
}

/// 小说形态：长篇 / 短篇。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NarrativeForm {
    LongNovel,
    ShortStory,
}

impl NarrativeForm {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LongNovel => "long_novel",
            Self::ShortStory => "short_story",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "long_novel" => Some(Self::LongNovel),
            "short_story" => Some(Self::ShortStory),
            _ => None,
        }
    }
}

/// 世界观实体（一个世界观模板，可以被多本小说引用）。
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct World {
    pub id: String,
    pub name: String,
    pub description: String,
    pub overview_summary: String,
    pub source_template_id: Option<String>,
    pub owner_novel_id: Option<String>,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// 类型查表：体裁。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Genre {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub description: String,
    pub sort_order: i32,
}

/// 类型查表：故事模式。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryMode {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub description: String,
    pub sort_order: i32,
}

/// 第一本书的 onboarding 投影，供前端"快速上手"卡片使用。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FirstNovelOnboarding {
    pub current_milestone: String,
    pub attention_required: bool,
    pub milestones: Vec<FirstNovelMilestone>,
    pub latest_novel_id: Option<String>,
    pub latest_chapter_id: Option<String>,
    pub latest_chapter_title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FirstNovelMilestone {
    pub key: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub result_summary: Option<String>,
}

/// 创作环境快设状态：模型是否已经填好、是否可以开始写。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickSetupStatus {
    pub ready_for_creation: bool,
    pub model_configured: bool,
    pub api_key_present: bool,
    pub endpoint: Option<String>,
    pub model: Option<String>,
    pub allow_localhost_http: bool,
    pub next_step: String,
}
