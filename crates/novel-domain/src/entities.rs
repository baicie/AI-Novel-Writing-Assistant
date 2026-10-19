/// 核心领域实体定义
/// 
/// 这个模块定义了小说创作系统的核心实体，包括小说、章节、人物、世界设定等。
/// 所有实体都遵循领域驱动设计原则，包含业务规则和不变量检查。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 小说实体
/// 
/// 代表整个小说项目，包含基本元数据、状态和创作信息。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Novel {
    /// 唯一标识
    pub id: String,
    
    /// 小说标题
    pub title: String,
    
    /// 作者名称
    pub author: Option<String>,
    
    /// 小说简介
    pub description: Option<String>,
    
    /// 小说分类/类型
    pub genre: Option<String>,
    
    /// 创作模式
    pub mode: NovelMode,
    
    /// 当前状态
    pub status: NovelStatus,
    
    /// 创建时间戳（秒）
    pub created_at: i64,
    
    /// 更新时间戳（秒）
    pub updated_at: i64,
    
    /// 标签/标记
    pub tags: Vec<String>,
}

/// 小说模式
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NovelMode {
    /// 原创创作
    Original,
    /// 续写/参考创作
    Continuation,
    /// 参考创作
    Reference,
}

/// 小说状态
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NovelStatus {
    /// 已创建
    Created,
    /// 规划中
    Planning,
    /// 创作中
    InProgress,
    /// 已完成
    Completed,
    /// 已存档
    Archived,
}

impl Novel {
    /// 创建新小说
    pub fn new(id: String, title: String, mode: NovelMode, now: i64) -> Self {
        Self {
            id,
            title,
            author: None,
            description: None,
            genre: None,
            mode,
            status: NovelStatus::Created,
            created_at: now,
            updated_at: now,
            tags: Vec::new(),
        }
    }
    
    /// 验证标题非空
    pub fn validate(&self) -> Result<(), String> {
        if self.title.trim().is_empty() {
            return Err("Novel title cannot be empty".to_string());
        }
        Ok(())
    }
}

/// 章节实体
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Chapter {
    /// 唯一标识
    pub id: String,
    
    /// 所属小说 ID
    pub novel_id: String,
    
    /// 章节序号
    pub chapter_number: i32,
    
    /// 章节标题
    pub title: String,
    
    /// 章节内容
    pub content: Option<String>,
    
    /// 字数
    pub word_count: i32,
    
    /// 生成状态
    pub generation_state: Option<String>,
    
    /// 章节状态
    pub chapter_status: Option<String>,
    
    /// 创建时间戳（秒）
    pub created_at: i64,
    
    /// 更新时间戳（秒）
    pub updated_at: i64,
}

impl Chapter {
    /// 创建新章节
    pub fn new(
        id: String,
        novel_id: String,
        chapter_number: i32,
        title: String,
        now: i64,
    ) -> Self {
        Self {
            id,
            novel_id,
            chapter_number,
            title,
            content: None,
            word_count: 0,
            generation_state: None,
            chapter_status: None,
            created_at: now,
            updated_at: now,
        }
    }
    
    /// 验证章节
    pub fn validate(&self) -> Result<(), String> {
        if self.title.trim().is_empty() {
            return Err("Chapter title cannot be empty".to_string());
        }
        if self.chapter_number < 1 {
            return Err("Chapter number must be >= 1".to_string());
        }
        Ok(())
    }
    
    /// 计算字数
    pub fn update_word_count(&mut self) {
        self.word_count = self.content.as_ref()
            .map(|c| c.chars().count() as i32)
            .unwrap_or(0);
    }
}

/// 人物实体
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Character {
    /// 唯一标识
    pub id: String,
    
    /// 所属小说 ID
    pub novel_id: String,
    
    /// 人物名称
    pub name: String,
    
    /// 角色定位
    pub role: Option<String>,
    
    /// 性格描述
    pub personality: Option<String>,
    
    /// 人物关系（JSON 格式或其他人物 ID 列表）
    pub relationships: Option<Vec<String>>,
    
    /// 发展弧线/成长故事
    pub arc: Option<String>,
    
    /// 创建时间戳（秒）
    pub created_at: i64,
}

impl Character {
    /// 创建新人物
    pub fn new(
        id: String,
        novel_id: String,
        name: String,
        now: i64,
    ) -> Self {
        Self {
            id,
            novel_id,
            name,
            role: None,
            personality: None,
            relationships: None,
            arc: None,
            created_at: now,
        }
    }
    
    /// 验证人物
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("Character name cannot be empty".to_string());
        }
        Ok(())
    }
}

/// 世界设定实体
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorldSetting {
    /// 唯一标识
    pub id: String,
    
    /// 所属小说 ID
    pub novel_id: String,
    
    /// 设定名称
    pub name: String,
    
    /// 设定类型（如：地理、政治、魔法体系等）
    pub world_type: Option<String>,
    
    /// 详细描述
    pub description: Option<String>,
    
    /// 关键属性（JSON 格式）
    pub key_attributes: Option<HashMap<String, String>>,
    
    /// 创建时间戳（秒）
    pub created_at: i64,
}

impl WorldSetting {
    /// 创建新世界设定
    pub fn new(
        id: String,
        novel_id: String,
        name: String,
        now: i64,
    ) -> Self {
        Self {
            id,
            novel_id,
            name,
            world_type: None,
            description: None,
            key_attributes: None,
            created_at: now,
        }
    }
    
    /// 验证世界设定
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("World name cannot be empty".to_string());
        }
        Ok(())
    }
}

/// 工作流步骤类型
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowStepType {
    /// 大纲生成
    OutlineGeneration,
    /// 章节规划
    ChapterPlanning,
    /// 正文生成
    ContentGeneration,
    /// 审校评审
    Review,
    /// 修复调整
    Repair,
    /// 发布
    Publish,
}

/// 工作流步骤状态
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    /// 待执行
    Pending,
    /// 执行中
    Running,
    /// 已完成
    Completed,
    /// 已暂停
    Paused,
    /// 已失败
    Failed,
}

/// 工作流步骤
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    /// 唯一标识
    pub id: String,
    
    /// 所属运行 ID
    pub run_id: String,
    
    /// 步骤类型
    pub step_type: WorkflowStepType,
    
    /// 当前状态
    pub status: StepStatus,
    
    /// 步骤序号
    pub sequence: i32,
    
    /// 输入数据（JSON 格式）
    pub input: Option<serde_json::Value>,
    
    /// 输出数据（JSON 格式）
    pub output: Option<serde_json::Value>,
    
    /// 错误信息
    pub error: Option<String>,
    
    /// 创建时间戳（秒）
    pub created_at: i64,
    
    /// 开始时间戳（秒）
    pub started_at: Option<i64>,
    
    /// 完成时间戳（秒）
    pub completed_at: Option<i64>,
}

impl WorkflowStep {
    /// 创建新步骤
    pub fn new(
        id: String,
        run_id: String,
        step_type: WorkflowStepType,
        sequence: i32,
        now: i64,
    ) -> Self {
        Self {
            id,
            run_id,
            step_type,
            status: StepStatus::Pending,
            sequence,
            input: None,
            output: None,
            error: None,
            created_at: now,
            started_at: None,
            completed_at: None,
        }
    }
}

/// 运行合同（Run Contract）
/// 
/// 不可变的运行记录，表示一次完整的工作流执行
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunContract {
    /// 唯一标识
    pub id: String,
    
    /// 所属小说 ID
    pub novel_id: String,
    
    /// 运行状态（使用 novel_domain::RunStatus）
    pub status_str: String,  // 存储为字符串
    
    /// 开始章节号
    pub start_chapter: i32,
    
    /// 预计结束章节号
    pub end_chapter: Option<i32>,
    
    /// 已完成章节数
    pub completed_chapters: i32,
    
    /// 质量债记录
    pub quality_debt: Vec<QualityIssue>,
    
    /// 创建时间戳（秒）
    pub created_at: i64,
    
    /// 开始时间戳（秒）
    pub started_at: Option<i64>,
    
    /// 完成时间戳（秒）
    pub completed_at: Option<i64>,
}

impl RunContract {
    /// 创建新运行合同
    pub fn new(
        id: String,
        novel_id: String,
        start_chapter: i32,
        end_chapter: Option<i32>,
        now: i64,
    ) -> Self {
        Self {
            id,
            novel_id,
            status_str: "Queued".to_string(),
            start_chapter,
            end_chapter,
            completed_chapters: 0,
            quality_debt: Vec::new(),
            created_at: now,
            started_at: None,
            completed_at: None,
        }
    }
}

/// 质量问题
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QualityIssue {
    /// 问题 ID
    pub id: String,
    
    /// 问题类型
    pub issue_type: String,
    
    /// 问题描述
    pub description: String,
    
    /// 涉及章节
    pub chapter_id: Option<String>,
    
    /// 建议动作
    pub recommended_action: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_novel_creation() {
        let novel = Novel::new(
            "novel-1".to_string(),
            "Test Novel".to_string(),
            NovelMode::Original,
            1000,
        );
        assert_eq!(novel.title, "Test Novel");
        assert_eq!(novel.mode, NovelMode::Original);
        assert_eq!(novel.status, NovelStatus::Created);
    }
    
    #[test]
    fn test_novel_validation() {
        let mut novel = Novel::new(
            "novel-1".to_string(),
            "".to_string(),
            NovelMode::Original,
            1000,
        );
        assert!(novel.validate().is_err());
        
        novel.title = "Valid Title".to_string();
        assert!(novel.validate().is_ok());
    }
    
    #[test]
    fn test_chapter_creation() {
        let chapter = Chapter::new(
            "ch-1".to_string(),
            "novel-1".to_string(),
            1,
            "Chapter 1".to_string(),
            1000,
        );
        assert_eq!(chapter.chapter_number, 1);
        assert_eq!(chapter.title, "Chapter 1");
    }
    
    #[test]
    fn test_chapter_word_count() {
        let mut chapter = Chapter::new(
            "ch-1".to_string(),
            "novel-1".to_string(),
            1,
            "Chapter 1".to_string(),
            1000,
        );
        
        chapter.content = Some("这是一个测试。".to_string());
        chapter.update_word_count();
        // "这是一个测试。" 是 7 个 Unicode 字符
        assert_eq!(chapter.word_count, 7);
    }
    
    #[test]
    fn test_character_creation() {
        let character = Character::new(
            "char-1".to_string(),
            "novel-1".to_string(),
            "Hero".to_string(),
            1000,
        );
        assert_eq!(character.name, "Hero");
    }
    
    #[test]
    fn test_world_creation() {
        let world = WorldSetting::new(
            "world-1".to_string(),
            "novel-1".to_string(),
            "Fantasy Realm".to_string(),
            1000,
        );
        assert_eq!(world.name, "Fantasy Realm");
    }
}
