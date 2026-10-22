use async_trait::async_trait;
use novel_application::{LookupRepository, PortError};
use novel_domain::{FirstNovelMilestone, FirstNovelOnboarding, Genre, StoryMode};
use sqlx::Row;

use crate::db::{SqliteKernel, map_sql};

#[async_trait]
impl LookupRepository for SqliteKernel {
    async fn list_genres(&self) -> Result<Vec<Genre>, PortError> {
        let rows = sqlx::query(
            "SELECT id, name, slug, description, sort_order FROM genre WHERE is_active = 1 ORDER BY sort_order, name",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sql)?;
        rows.iter().map(genre_from_row).collect()
    }

    async fn list_story_modes(&self) -> Result<Vec<StoryMode>, PortError> {
        let rows = sqlx::query(
            "SELECT id, name, slug, description, sort_order FROM story_mode WHERE is_active = 1 ORDER BY sort_order, name",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sql)?;
        rows.iter().map(story_mode_from_row).collect()
    }

    async fn get_first_novel_onboarding(&self) -> Result<FirstNovelOnboarding, PortError> {
        // 简单版 onboarding 投影：先看是否有模型密钥；没有就停在"创作环境"。
        // 拿到 API key 后看有没有第一本书，没有就停在"灵感与方向"。
        // 找到第一本书后看有没有第一章正文，没有就停在"首章成稿"。
        let novel_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM novel")
            .fetch_one(&self.pool)
            .await
            .map_err(map_sql)?;
        let first_chapter = sqlx::query(
            "SELECT c.id, c.title, c.novel_id FROM chapter c
             JOIN chapter_revision r ON r.id = c.official_revision_id
             WHERE r.content <> ''
             ORDER BY c.sort_order ASC LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sql)?;

        let (
            current_milestone,
            attention,
            latest_novel_id,
            latest_chapter_id,
            latest_chapter_title,
        ) = if novel_count == 0 {
            ("environment", true, None, None, None)
        } else {
            match first_chapter {
                Some(row) => {
                    let id: String = row.try_get("id").map_err(map_sql)?;
                    let title: String = row.try_get("title").map_err(map_sql)?;
                    let novel_id: String = row.try_get("novel_id").map_err(map_sql)?;
                    (
                        "first_chapter",
                        false,
                        Some(novel_id),
                        Some(id),
                        Some(title),
                    )
                }
                None => ("first_chapter", true, None, None, None),
            }
        };

        let milestones = build_milestones(current_milestone, attention);
        Ok(FirstNovelOnboarding {
            current_milestone: current_milestone.to_string(),
            attention_required: attention,
            milestones,
            latest_novel_id,
            latest_chapter_id,
            latest_chapter_title,
        })
    }
}

fn genre_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Genre, PortError> {
    Ok(Genre {
        id: row.try_get("id").map_err(map_sql)?,
        name: row.try_get("name").map_err(map_sql)?,
        slug: row.try_get("slug").map_err(map_sql)?,
        description: row.try_get("description").map_err(map_sql)?,
        sort_order: row.try_get("sort_order").map_err(map_sql)?,
    })
}

fn story_mode_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<StoryMode, PortError> {
    Ok(StoryMode {
        id: row.try_get("id").map_err(map_sql)?,
        name: row.try_get("name").map_err(map_sql)?,
        slug: row.try_get("slug").map_err(map_sql)?,
        description: row.try_get("description").map_err(map_sql)?,
        sort_order: row.try_get("sort_order").map_err(map_sql)?,
    })
}

fn build_milestones(current: &str, attention: bool) -> Vec<FirstNovelMilestone> {
    let definitions: &[(&str, &str, &str)] = &[
        (
            "environment",
            "创作环境",
            "配置一个能完成规划、正文和审校的文本模型。",
        ),
        (
            "idea_direction",
            "灵感与方向",
            "写下一句话灵感，并从 AI 给出的方向中选择一套。",
        ),
        (
            "preparation",
            "开书准备",
            "AI 准备故事、世界、角色、卷章规划和执行资源。",
        ),
        (
            "production_choice",
            "生产方式",
            "选择 AI 持续写完整本书，或进入专业工作台。",
        ),
        (
            "first_chapter",
            "首章成稿",
            "等待第一章完成后，打开正文开始阅读。",
        ),
    ];
    let current_index = definitions
        .iter()
        .position(|(key, _, _)| *key == current)
        .unwrap_or(0);
    definitions
        .iter()
        .enumerate()
        .map(|(index, (key, title, description))| FirstNovelMilestone {
            key: (*key).to_string(),
            title: (*title).to_string(),
            description: (*description).to_string(),
            status: if index < current_index {
                "completed".to_string()
            } else if index == current_index {
                if attention {
                    "attention".to_string()
                } else {
                    "current".to_string()
                }
            } else {
                "pending".to_string()
            },
            result_summary: None,
        })
        .collect()
}
