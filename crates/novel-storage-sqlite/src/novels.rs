use std::ops::DerefMut;

use async_trait::async_trait;
use novel_application::{
    CandidateEditDraftView, ChapterDraftView, ChapterRevisionSnapshot, ChapterSnapshot,
    CharacterSnapshot, CreateNovelInput, DeleteChapterRecord, ListNovelsQuery, ListNovelsResult,
    NewChapter, NewCharacter, NewNovel, NewVolume, NovelDetail, NovelRepository, NovelSummary,
    PortError, PromoteOutcome, SaveCandidateEditDraftRecord, SaveChapterDraftRecord,
    SaveChapterRecord, StoryFactView, UpdateChapterMetaRecord, UpdateNovelFields,
    UpdateVolumeRecord, VolumeSnapshot,
};
use novel_context::{Lexicon, fts_query, tokenize};
use novel_domain::{ChapterProtection, CreationExperience, ensure_can_promote};
use novel_workflow::chapter_scope;
use sqlx::Row;

use crate::db::{SqliteKernel, map_sql};

#[async_trait]
impl NovelRepository for SqliteKernel {
    async fn list_novels(&self) -> Result<Vec<NovelSummary>, PortError> {
        let rows = sqlx::query(
            "SELECT id, title, description, status, creation_experience, narrative_form, writing_mode, project_mode, updated_at
             FROM novel ORDER BY updated_at DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sql)?;
        rows.iter().map(novel_from_row).collect()
    }

    async fn list_novels_paginated(
        &self,
        query: &ListNovelsQuery,
    ) -> Result<ListNovelsResult, PortError> {
        let page = query.page.unwrap_or(1).max(1);
        let limit = query.limit.unwrap_or(20).clamp(1, 200);
        let offset = ((page - 1) * limit) as i64;
        let search = query.search.as_deref().unwrap_or("").trim();
        let like = if search.is_empty() {
            None
        } else {
            Some(format!("%{search}%"))
        };
        let status = query.status.as_deref().filter(|value| !value.is_empty());
        let narrative_form = query
            .narrative_form
            .as_deref()
            .filter(|value| !value.is_empty());
        let writing_mode = query
            .writing_mode
            .as_deref()
            .filter(|value| !value.is_empty());

        // 简单实现：用条件分支拼 SQL，避免引入动态 binding 框架。
        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM novel
             WHERE (?1 IS NULL OR title LIKE ?1 OR description LIKE ?1)
               AND (?2 IS NULL OR status = ?2)
               AND (?3 IS NULL OR narrative_form = ?3)
               AND (?4 IS NULL OR writing_mode = ?4)",
        )
        .bind(&like)
        .bind(status)
        .bind(narrative_form)
        .bind(writing_mode)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sql)?;

        let rows = sqlx::query(
            "SELECT id, title, description, status, creation_experience, narrative_form, writing_mode, project_mode, updated_at
             FROM novel
             WHERE (?1 IS NULL OR title LIKE ?1 OR description LIKE ?1)
               AND (?2 IS NULL OR status = ?2)
               AND (?3 IS NULL OR narrative_form = ?3)
               AND (?4 IS NULL OR writing_mode = ?4)
             ORDER BY updated_at DESC
             LIMIT ?5 OFFSET ?6",
        )
        .bind(&like)
        .bind(status)
        .bind(narrative_form)
        .bind(writing_mode)
        .bind(limit as i64)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sql)?;

        let items = rows
            .iter()
            .map(novel_from_row)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ListNovelsResult {
            items,
            page,
            limit,
            total,
        })
    }

    async fn create_novel(&self, novel: NewNovel) -> Result<NovelSummary, PortError> {
        sqlx::query(
            "INSERT INTO novel(id, title, description, status, post_generation_style_review_enabled, created_at, updated_at) VALUES (?, ?, ?, 'draft', 1, ?, ?)",
        )
        .bind(&novel.id)
        .bind(&novel.title)
        .bind(&novel.description)
        .bind(novel.now_ms)
        .bind(novel.now_ms)
        .execute(&self.pool)
        .await
        .map_err(map_sql)?;
        Ok(NovelSummary {
            id: novel.id,
            title: novel.title,
            description: novel.description,
            status: "draft".into(),
            creation_experience: "simple".into(),
            narrative_form: "long_novel".into(),
            writing_mode: "original".into(),
            project_mode: "ai_led".into(),
            updated_at: novel.now_ms,
        })
    }

    async fn create_novel_with_setup(
        &self,
        input: CreateNovelInput,
    ) -> Result<NovelDetail, PortError> {
        let commercial_tags_json =
            serde_json::to_string(&input.commercial_tags).unwrap_or_else(|_| "[]".to_string());
        sqlx::query(
            "INSERT INTO novel(
                id, title, description, status,
                narrative_form, target_audience, book_selling_point, competing_feel,
                first_30_chapter_promise, commercial_tags_json,
                genre_id, primary_story_mode_id, secondary_story_mode_id, world_id,
                writing_mode, project_mode, creation_experience,
                style_tone, default_chapter_length, estimated_chapter_count,
                post_generation_style_review_enabled, created_at, updated_at
             ) VALUES (
                ?, ?, ?, 'draft',
                ?, ?, ?, ?,
                ?, ?,
                ?, ?, ?, ?,
                ?, ?, ?,
                ?, ?, ?,
                ?, ?, ?
             )",
        )
        .bind(&input.id)
        .bind(&input.title)
        .bind(&input.description)
        .bind(input.narrative_form.as_str())
        .bind(&input.target_audience)
        .bind(&input.book_selling_point)
        .bind(&input.competing_feel)
        .bind(&input.first_30_chapter_promise)
        .bind(&commercial_tags_json)
        .bind(&input.genre_id)
        .bind(&input.primary_story_mode_id)
        .bind(&input.secondary_story_mode_id)
        .bind(&input.world_id)
        .bind(input.writing_mode.as_str())
        .bind(input.project_mode.as_str())
        .bind(input.creation_experience.as_str())
        .bind(&input.style_tone)
        .bind(input.default_chapter_length)
        .bind(input.estimated_chapter_count)
        .bind(i64::from(u8::from(
            input.post_generation_style_review_enabled,
        )))
        .bind(input.now_ms)
        .bind(input.now_ms)
        .execute(&self.pool)
        .await
        .map_err(map_sql)?;
        self.get_novel(&input.id).await
    }

    async fn get_novel(&self, id: &str) -> Result<NovelDetail, PortError> {
        let row = sqlx::query(
            "SELECT id, title, description, status,
                    narrative_form, target_audience, book_selling_point, competing_feel,
                    first_30_chapter_promise, commercial_tags_json,
                    genre_id, primary_story_mode_id, secondary_story_mode_id, world_id,
                    writing_mode, project_mode, creation_experience,
                    narrative_pov, pace_preference, style_tone, emotion_intensity, ai_freedom,
                    default_chapter_length, estimated_chapter_count,
                    project_status, storyline_status, outline_status, resource_ready_score,
                    post_generation_style_review_enabled, created_at, updated_at
             FROM novel WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sql)?
        .ok_or_else(|| PortError::NotFound("没有找到这本书。".into()))?;
        novel_detail_from_row(&row)
    }

    async fn update_novel(
        &self,
        id: &str,
        fields: &UpdateNovelFields,
        now_ms: i64,
    ) -> Result<NovelDetail, PortError> {
        // 字段是稀疏的；null vs unset 由外层/内层 Option 表示。所有列更新在一个事务内提交，
        // 避免保存到一半时留下一本字段互相矛盾的半成品小说。
        let mut tx = self.pool.begin().await.map_err(map_sql)?;
        if let Some(value) = &fields.title {
            sqlx::query("UPDATE novel SET title = ?, updated_at = ? WHERE id = ?")
                .bind(value)
                .bind(now_ms)
                .bind(id)
                .execute(tx.deref_mut())
                .await
                .map_err(map_sql)?;
        }
        if let Some(value) = &fields.description {
            sqlx::query("UPDATE novel SET description = ?, updated_at = ? WHERE id = ?")
                .bind(value)
                .bind(now_ms)
                .bind(id)
                .execute(tx.deref_mut())
                .await
                .map_err(map_sql)?;
        }
        if let Some(value) = &fields.target_audience {
            sqlx::query("UPDATE novel SET target_audience = ?, updated_at = ? WHERE id = ?")
                .bind(value.as_deref())
                .bind(now_ms)
                .bind(id)
                .execute(tx.deref_mut())
                .await
                .map_err(map_sql)?;
        }
        if let Some(value) = &fields.book_selling_point {
            sqlx::query("UPDATE novel SET book_selling_point = ?, updated_at = ? WHERE id = ?")
                .bind(value.as_deref())
                .bind(now_ms)
                .bind(id)
                .execute(tx.deref_mut())
                .await
                .map_err(map_sql)?;
        }
        if let Some(value) = &fields.competing_feel {
            sqlx::query("UPDATE novel SET competing_feel = ?, updated_at = ? WHERE id = ?")
                .bind(value.as_deref())
                .bind(now_ms)
                .bind(id)
                .execute(tx.deref_mut())
                .await
                .map_err(map_sql)?;
        }
        if let Some(value) = &fields.first_30_chapter_promise {
            sqlx::query(
                "UPDATE novel SET first_30_chapter_promise = ?, updated_at = ? WHERE id = ?",
            )
            .bind(value.as_deref())
            .bind(now_ms)
            .bind(id)
            .execute(tx.deref_mut())
            .await
            .map_err(map_sql)?;
        }
        if let Some(value) = &fields.commercial_tags {
            let json = match value {
                Some(tags) => serde_json::to_string(tags).unwrap_or_else(|_| "[]".to_string()),
                None => "[]".to_string(),
            };
            sqlx::query("UPDATE novel SET commercial_tags_json = ?, updated_at = ? WHERE id = ?")
                .bind(json)
                .bind(now_ms)
                .bind(id)
                .execute(tx.deref_mut())
                .await
                .map_err(map_sql)?;
        }
        if let Some(value) = &fields.genre_id {
            sqlx::query("UPDATE novel SET genre_id = ?, updated_at = ? WHERE id = ?")
                .bind(value.as_deref())
                .bind(now_ms)
                .bind(id)
                .execute(tx.deref_mut())
                .await
                .map_err(map_sql)?;
        }
        if let Some(value) = &fields.primary_story_mode_id {
            sqlx::query("UPDATE novel SET primary_story_mode_id = ?, updated_at = ? WHERE id = ?")
                .bind(value.as_deref())
                .bind(now_ms)
                .bind(id)
                .execute(tx.deref_mut())
                .await
                .map_err(map_sql)?;
        }
        if let Some(value) = &fields.secondary_story_mode_id {
            sqlx::query(
                "UPDATE novel SET secondary_story_mode_id = ?, updated_at = ? WHERE id = ?",
            )
            .bind(value.as_deref())
            .bind(now_ms)
            .bind(id)
            .execute(tx.deref_mut())
            .await
            .map_err(map_sql)?;
        }
        if let Some(value) = &fields.world_id {
            sqlx::query("UPDATE novel SET world_id = ?, updated_at = ? WHERE id = ?")
                .bind(value.as_deref())
                .bind(now_ms)
                .bind(id)
                .execute(tx.deref_mut())
                .await
                .map_err(map_sql)?;
        }
        if let Some(value) = fields.narrative_form.as_deref() {
            sqlx::query("UPDATE novel SET narrative_form = ?, updated_at = ? WHERE id = ?")
                .bind(value)
                .bind(now_ms)
                .bind(id)
                .execute(tx.deref_mut())
                .await
                .map_err(map_sql)?;
        }
        if let Some(value) = fields.writing_mode.as_deref() {
            sqlx::query("UPDATE novel SET writing_mode = ?, updated_at = ? WHERE id = ?")
                .bind(value)
                .bind(now_ms)
                .bind(id)
                .execute(tx.deref_mut())
                .await
                .map_err(map_sql)?;
        }
        if let Some(value) = fields.project_mode.as_deref() {
            sqlx::query("UPDATE novel SET project_mode = ?, updated_at = ? WHERE id = ?")
                .bind(value)
                .bind(now_ms)
                .bind(id)
                .execute(tx.deref_mut())
                .await
                .map_err(map_sql)?;
        }
        if let Some(value) = &fields.narrative_pov {
            sqlx::query("UPDATE novel SET narrative_pov = ?, updated_at = ? WHERE id = ?")
                .bind(value.as_deref())
                .bind(now_ms)
                .bind(id)
                .execute(tx.deref_mut())
                .await
                .map_err(map_sql)?;
        }
        if let Some(value) = &fields.pace_preference {
            sqlx::query("UPDATE novel SET pace_preference = ?, updated_at = ? WHERE id = ?")
                .bind(value.as_deref())
                .bind(now_ms)
                .bind(id)
                .execute(tx.deref_mut())
                .await
                .map_err(map_sql)?;
        }
        if let Some(value) = &fields.style_tone {
            sqlx::query("UPDATE novel SET style_tone = ?, updated_at = ? WHERE id = ?")
                .bind(value.as_deref())
                .bind(now_ms)
                .bind(id)
                .execute(tx.deref_mut())
                .await
                .map_err(map_sql)?;
        }
        if let Some(value) = &fields.emotion_intensity {
            sqlx::query("UPDATE novel SET emotion_intensity = ?, updated_at = ? WHERE id = ?")
                .bind(value.as_deref())
                .bind(now_ms)
                .bind(id)
                .execute(tx.deref_mut())
                .await
                .map_err(map_sql)?;
        }
        if let Some(value) = &fields.ai_freedom {
            sqlx::query("UPDATE novel SET ai_freedom = ?, updated_at = ? WHERE id = ?")
                .bind(value.as_deref())
                .bind(now_ms)
                .bind(id)
                .execute(tx.deref_mut())
                .await
                .map_err(map_sql)?;
        }
        if let Some(value) = &fields.default_chapter_length {
            sqlx::query("UPDATE novel SET default_chapter_length = ?, updated_at = ? WHERE id = ?")
                .bind(value)
                .bind(now_ms)
                .bind(id)
                .execute(tx.deref_mut())
                .await
                .map_err(map_sql)?;
        }
        if let Some(value) = &fields.estimated_chapter_count {
            sqlx::query(
                "UPDATE novel SET estimated_chapter_count = ?, updated_at = ? WHERE id = ?",
            )
            .bind(value)
            .bind(now_ms)
            .bind(id)
            .execute(tx.deref_mut())
            .await
            .map_err(map_sql)?;
        }
        if let Some(value) = fields.project_status.as_deref() {
            sqlx::query("UPDATE novel SET project_status = ?, updated_at = ? WHERE id = ?")
                .bind(value)
                .bind(now_ms)
                .bind(id)
                .execute(tx.deref_mut())
                .await
                .map_err(map_sql)?;
        }
        if let Some(value) = fields.storyline_status.as_deref() {
            sqlx::query("UPDATE novel SET storyline_status = ?, updated_at = ? WHERE id = ?")
                .bind(value)
                .bind(now_ms)
                .bind(id)
                .execute(tx.deref_mut())
                .await
                .map_err(map_sql)?;
        }
        if let Some(value) = fields.outline_status.as_deref() {
            sqlx::query("UPDATE novel SET outline_status = ?, updated_at = ? WHERE id = ?")
                .bind(value)
                .bind(now_ms)
                .bind(id)
                .execute(tx.deref_mut())
                .await
                .map_err(map_sql)?;
        }
        if let Some(value) = fields.resource_ready_score {
            sqlx::query("UPDATE novel SET resource_ready_score = ?, updated_at = ? WHERE id = ?")
                .bind(value)
                .bind(now_ms)
                .bind(id)
                .execute(tx.deref_mut())
                .await
                .map_err(map_sql)?;
        }
        if let Some(value) = fields.status.as_deref() {
            sqlx::query("UPDATE novel SET status = ?, updated_at = ? WHERE id = ?")
                .bind(value)
                .bind(now_ms)
                .bind(id)
                .execute(tx.deref_mut())
                .await
                .map_err(map_sql)?;
        }
        if let Some(value) = fields.post_generation_style_review_enabled {
            sqlx::query(
                "UPDATE novel SET post_generation_style_review_enabled = ?, updated_at = ? WHERE id = ?",
            )
            .bind(i64::from(u8::from(value)))
            .bind(now_ms)
            .bind(id)
            .execute(tx.deref_mut())
            .await
            .map_err(map_sql)?;
        }
        tx.commit().await.map_err(map_sql)?;
        self.get_novel(id).await
    }

    async fn delete_novel(&self, id: &str) -> Result<(), PortError> {
        let mut tx = self.pool.begin().await.map_err(map_sql)?;
        let chapters: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM chapter WHERE novel_id = ?")
            .bind(id)
            .fetch_one(tx.deref_mut())
            .await
            .map_err(map_sql)?;
        let volumes: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM volume WHERE novel_id = ?")
            .bind(id)
            .fetch_one(tx.deref_mut())
            .await
            .map_err(map_sql)?;
        if chapters > 0 || volumes > 0 {
            return Err(PortError::Conflict(
                "这本书里还有卷或章节，先导出并删除书稿内容再删除整本书。".into(),
            ));
        }
        let result = sqlx::query("DELETE FROM novel WHERE id = ?")
            .bind(id)
            .execute(tx.deref_mut())
            .await
            .map_err(map_sql)?;
        if result.rows_affected() == 0 {
            return Err(PortError::NotFound("没有找到这本书。".into()));
        }
        tx.commit().await.map_err(map_sql)?;
        Ok(())
    }

    async fn set_creation_experience(
        &self,
        id: &str,
        experience: CreationExperience,
        now_ms: i64,
    ) -> Result<NovelDetail, PortError> {
        sqlx::query("UPDATE novel SET creation_experience = ?, updated_at = ? WHERE id = ?")
            .bind(experience.as_str())
            .bind(now_ms)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_sql)?;
        self.get_novel(id).await
    }

    async fn create_chapter(&self, chapter: NewChapter) -> Result<ChapterSnapshot, PortError> {
        let mut tx = self.pool.begin().await.map_err(map_sql)?;
        if let Some(volume_id) = chapter.volume_id.as_deref() {
            let belongs: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM volume WHERE id = ? AND novel_id = ?")
                    .bind(volume_id)
                    .bind(&chapter.novel_id)
                    .fetch_one(tx.deref_mut())
                    .await
                    .map_err(map_sql)?;
            if belongs == 0 {
                return Err(PortError::NotFound("没有找到这一卷。".into()));
            }
        }
        sqlx::query("INSERT INTO chapter(id, novel_id, volume_id, title, sort_order, protection, created_at, updated_at) VALUES (?, ?, ?, ?, ?, 'none', ?, ?)")
            .bind(&chapter.id)
            .bind(&chapter.novel_id)
            .bind(chapter.volume_id.as_deref())
            .bind(&chapter.title)
            .bind(chapter.sort_order)
            .bind(chapter.now_ms)
            .bind(chapter.now_ms)
            .execute(tx.deref_mut())
            .await
            .map_err(map_sql)?;
        tx.commit().await.map_err(map_sql)?;
        Ok(ChapterSnapshot {
            id: chapter.id,
            novel_id: chapter.novel_id,
            title: chapter.title,
            sort_order: chapter.sort_order,
            protection: "none".into(),
            official_version: 0,
            content: String::new(),
            updated_at: chapter.now_ms,
        })
    }

    async fn list_chapters(&self, novel_id: &str) -> Result<Vec<ChapterSnapshot>, PortError> {
        let rows = sqlx::query(
            "SELECT c.id, c.novel_id, c.title, c.sort_order, c.protection, c.updated_at, r.version AS official_version, r.content AS content
             FROM chapter c
             LEFT JOIN chapter_revision r ON r.id = c.official_revision_id
             WHERE c.novel_id = ?
             ORDER BY c.sort_order",
        )
        .bind(novel_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sql)?;
        rows.iter().map(chapter_from_row).collect()
    }

    async fn create_character(
        &self,
        character: NewCharacter,
    ) -> Result<CharacterSnapshot, PortError> {
        sqlx::query(
            "INSERT INTO character(id, novel_id, name, role, profile_json, created_at, updated_at)
             VALUES (?, ?, ?, ?, '{}', ?, ?)",
        )
        .bind(&character.id)
        .bind(&character.novel_id)
        .bind(&character.name)
        .bind(&character.role)
        .bind(character.now_ms)
        .bind(character.now_ms)
        .execute(&self.pool)
        .await
        .map_err(map_sql)?;
        Ok(CharacterSnapshot {
            id: character.id,
            novel_id: character.novel_id,
            name: character.name,
            role: character.role,
            personality: None,
            arc: None,
            profile: serde_json::json!({}),
            created_at: character.now_ms,
            updated_at: character.now_ms,
        })
    }

    async fn list_characters(&self, novel_id: &str) -> Result<Vec<CharacterSnapshot>, PortError> {
        let rows = sqlx::query(
            "SELECT id, novel_id, name, role, profile_json, created_at, updated_at
             FROM character WHERE novel_id = ? ORDER BY created_at, id",
        )
        .bind(novel_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sql)?;
        rows.iter()
            .map(|row| {
                let profile = row
                    .try_get::<String, _>("profile_json")
                    .ok()
                    .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
                    .unwrap_or_else(|| serde_json::json!({}));
                Ok(CharacterSnapshot {
                    id: row.try_get("id").map_err(map_sql)?,
                    novel_id: row.try_get("novel_id").map_err(map_sql)?,
                    name: row.try_get("name").map_err(map_sql)?,
                    role: row.try_get("role").map_err(map_sql)?,
                    personality: profile
                        .get("personality")
                        .and_then(|v| v.as_str())
                        .map(str::to_string),
                    arc: profile
                        .get("arc")
                        .and_then(|v| v.as_str())
                        .map(str::to_string),
                    profile,
                    created_at: row.try_get("created_at").map_err(map_sql)?,
                    updated_at: row.try_get("updated_at").map_err(map_sql)?,
                })
            })
            .collect()
    }

    async fn get_chapter(
        &self,
        novel_id: &str,
        chapter_id: &str,
    ) -> Result<ChapterSnapshot, PortError> {
        let row = sqlx::query(
            "SELECT c.id, c.novel_id, c.title, c.sort_order, c.protection, c.updated_at, r.version AS official_version, r.content AS content
             FROM chapter c
             LEFT JOIN chapter_revision r ON r.id = c.official_revision_id
             WHERE c.novel_id = ? AND c.id = ?",
        )
        .bind(novel_id)
        .bind(chapter_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sql)?
        .ok_or_else(|| PortError::NotFound("没有找到这一章。".into()))?;
        chapter_from_row(&row)
    }

    async fn save_user_chapter(
        &self,
        input: SaveChapterRecord,
    ) -> Result<ChapterSnapshot, PortError> {
        let mut tx = self.pool.begin().await.map_err(map_sql)?;
        let current = sqlx::query(
            "SELECT c.protection, r.version AS official_version
             FROM chapter c
             LEFT JOIN chapter_revision r ON r.id = c.official_revision_id
             WHERE c.id = ? AND c.novel_id = ?",
        )
        .bind(&input.chapter_id)
        .bind(&input.novel_id)
        .fetch_optional(tx.deref_mut())
        .await
        .map_err(map_sql)?
        .ok_or_else(|| PortError::NotFound("没有找到这一章。".into()))?;
        let official_version: Option<i64> = current.try_get("official_version").map_err(map_sql)?;
        let version = official_version.unwrap_or(0);
        if version != input.expected_version {
            return Err(PortError::Conflict(
                "章节内容已经变化，请重新打开后再保存。".into(),
            ));
        }
        let next_version = version + 1;
        sqlx::query("INSERT INTO chapter_revision(id, chapter_id, version, origin, content, content_sha256, created_at) VALUES (?, ?, ?, 'user', ?, ?, ?)")
            .bind(&input.revision_id)
            .bind(&input.chapter_id)
            .bind(next_version)
            .bind(&input.content)
            .bind(&input.content_sha256)
            .bind(input.now_ms)
            .execute(tx.deref_mut())
            .await
            .map_err(map_sql)?;
        sqlx::query("UPDATE chapter SET title = ?, official_revision_id = ?, protection = 'user_edited', updated_at = ? WHERE id = ?")
            .bind(&input.title)
            .bind(&input.revision_id)
            .bind(input.now_ms)
            .bind(&input.chapter_id)
            .execute(tx.deref_mut())
            .await
            .map_err(map_sql)?;
        invalidate_story_facts(
            &mut tx,
            &input.chapter_id,
            &format!("{}@v{}", input.revision_id, next_version),
            input.now_ms,
        )
        .await?;
        insert_artifact(
            &mut tx,
            &input.artifact_id,
            &input.novel_id,
            None,
            "chapter_draft",
            &chapter_scope(&input.chapter_id),
            "user_edited",
            true,
            &serde_json::json!({ "revisionId": input.revision_id }).to_string(),
            &input.content_sha256,
            input.now_ms,
        )
        .await?;
        sync_chapter_knowledge(
            &mut tx,
            &input.novel_id,
            &input.chapter_id,
            &input.title,
            &input.content,
            &input.revision_id,
            next_version,
            input.now_ms,
        )
        .await?;
        tx.commit().await.map_err(map_sql)?;
        self.get_chapter(&input.novel_id, &input.chapter_id).await
    }

    async fn save_chapter_draft(
        &self,
        record: SaveChapterDraftRecord,
    ) -> Result<ChapterDraftView, PortError> {
        let row = if record.expected_revision == 0 {
            sqlx::query(
                "INSERT INTO chapter_local_draft
                    (novel_id, chapter_id, client_id, content, base_official_version, base_updated_at, local_revision, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, 1, ?)
                 ON CONFLICT(novel_id, chapter_id, client_id) DO NOTHING
                 RETURNING novel_id, chapter_id, client_id, content, base_official_version, base_updated_at, local_revision, updated_at",
            )
            .bind(&record.novel_id)
            .bind(&record.chapter_id)
            .bind(&record.client_id)
            .bind(&record.content)
            .bind(record.base_official_version)
            .bind(&record.base_updated_at)
            .bind(record.now_ms)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_sql)?
        } else {
            sqlx::query(
                "UPDATE chapter_local_draft
                 SET content = ?, base_official_version = ?, base_updated_at = ?,
                     local_revision = local_revision + 1, updated_at = ?
                 WHERE novel_id = ? AND chapter_id = ? AND client_id = ? AND local_revision = ?
                 RETURNING novel_id, chapter_id, client_id, content, base_official_version, base_updated_at, local_revision, updated_at",
            )
            .bind(&record.content)
            .bind(record.base_official_version)
            .bind(&record.base_updated_at)
            .bind(record.now_ms)
            .bind(&record.novel_id)
            .bind(&record.chapter_id)
            .bind(&record.client_id)
            .bind(record.expected_revision)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_sql)?
        };
        let row = row
            .ok_or_else(|| PortError::Conflict("本地草稿已经被更新，较旧的保存已拒绝。".into()))?;
        draft_from_row(&row)
    }

    async fn get_chapter_draft(
        &self,
        novel_id: &str,
        chapter_id: &str,
        client_id: &str,
    ) -> Result<Option<ChapterDraftView>, PortError> {
        let row = sqlx::query(
            "SELECT novel_id, chapter_id, client_id, content, base_official_version, base_updated_at, local_revision, updated_at
             FROM chapter_local_draft WHERE novel_id = ? AND chapter_id = ? AND client_id = ?",
        )
        .bind(novel_id)
        .bind(chapter_id)
        .bind(client_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sql)?;
        row.as_ref().map(draft_from_row).transpose()
    }

    async fn list_chapter_drafts(
        &self,
        novel_id: &str,
        chapter_id: &str,
    ) -> Result<Vec<ChapterDraftView>, PortError> {
        let rows = sqlx::query(
            "SELECT novel_id, chapter_id, client_id, content, base_official_version, base_updated_at, local_revision, updated_at
             FROM chapter_local_draft WHERE novel_id = ? AND chapter_id = ?
             ORDER BY updated_at DESC, local_revision DESC",
        )
        .bind(novel_id)
        .bind(chapter_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sql)?;
        rows.iter().map(draft_from_row).collect()
    }

    async fn clear_chapter_draft(
        &self,
        novel_id: &str,
        chapter_id: &str,
        client_id: &str,
        submitted_content: &str,
        local_revision: i64,
    ) -> Result<bool, PortError> {
        let result = sqlx::query(
            "DELETE FROM chapter_local_draft
             WHERE novel_id = ? AND chapter_id = ? AND client_id = ?
               AND content = ? AND local_revision <= ?",
        )
        .bind(novel_id)
        .bind(chapter_id)
        .bind(client_id)
        .bind(submitted_content)
        .bind(local_revision)
        .execute(&self.pool)
        .await
        .map_err(map_sql)?;
        Ok(result.rows_affected() > 0)
    }

    async fn save_candidate_edit_draft(
        &self,
        record: SaveCandidateEditDraftRecord,
    ) -> Result<CandidateEditDraftView, PortError> {
        let row = if record.expected_revision == 0 {
            sqlx::query(
                "INSERT INTO candidate_edit_draft
                    (novel_id, chapter_id, run_id, gate_id, client_id, base_candidate_id,
                     base_content_sha256, content, local_revision, control_version, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?)
                 ON CONFLICT(run_id, gate_id, client_id) DO NOTHING
                 RETURNING novel_id, chapter_id, run_id, gate_id, client_id, base_candidate_id,
                           base_content_sha256, content, local_revision, control_version, updated_at",
            )
            .bind(&record.novel_id).bind(&record.chapter_id).bind(&record.run_id)
            .bind(&record.gate_id).bind(&record.client_id).bind(&record.base_candidate_id)
            .bind(&record.base_content_sha256).bind(&record.content)
            .bind(record.control_version).bind(record.now_ms)
            .fetch_optional(&self.pool).await.map_err(map_sql)?
        } else {
            sqlx::query(
                "UPDATE candidate_edit_draft
                 SET content = ?, local_revision = local_revision + 1, control_version = ?, updated_at = ?
                 WHERE run_id = ? AND gate_id = ? AND client_id = ? AND local_revision = ?
                   AND base_candidate_id = ? AND base_content_sha256 = ?
                 RETURNING novel_id, chapter_id, run_id, gate_id, client_id, base_candidate_id,
                           base_content_sha256, content, local_revision, control_version, updated_at",
            )
            .bind(&record.content).bind(record.control_version).bind(record.now_ms)
            .bind(&record.run_id).bind(&record.gate_id).bind(&record.client_id)
            .bind(record.expected_revision).bind(&record.base_candidate_id)
            .bind(&record.base_content_sha256)
            .fetch_optional(&self.pool).await.map_err(map_sql)?
        };
        row.ok_or_else(|| PortError::Conflict("候选编辑草稿已被更新，较旧的保存已拒绝。".into()))
            .and_then(|row| candidate_edit_from_row(&row))
    }

    async fn get_candidate_edit_draft(
        &self,
        run_id: &str,
        gate_id: &str,
        client_id: &str,
    ) -> Result<Option<CandidateEditDraftView>, PortError> {
        let row = sqlx::query(
            "SELECT novel_id, chapter_id, run_id, gate_id, client_id, base_candidate_id,
                    base_content_sha256, content, local_revision, control_version, updated_at
             FROM candidate_edit_draft WHERE run_id = ? AND gate_id = ? AND client_id = ?",
        )
        .bind(run_id)
        .bind(gate_id)
        .bind(client_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sql)?;
        row.as_ref().map(candidate_edit_from_row).transpose()
    }

    async fn list_candidate_edit_drafts(
        &self,
        novel_id: &str,
        run_id: &str,
    ) -> Result<Vec<CandidateEditDraftView>, PortError> {
        let rows = sqlx::query(
            "SELECT novel_id, chapter_id, run_id, gate_id, client_id, base_candidate_id,
                    base_content_sha256, content, local_revision, control_version, updated_at
             FROM candidate_edit_draft WHERE novel_id = ? AND run_id = ?
             ORDER BY updated_at DESC, rowid DESC",
        )
        .bind(novel_id)
        .bind(run_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sql)?;
        rows.iter().map(candidate_edit_from_row).collect()
    }

    async fn clear_candidate_edit_draft(
        &self,
        run_id: &str,
        gate_id: &str,
        client_id: &str,
        content: &str,
        local_revision: i64,
    ) -> Result<bool, PortError> {
        let result = sqlx::query(
            "DELETE FROM candidate_edit_draft
             WHERE run_id = ? AND gate_id = ? AND client_id = ?
               AND content = ? AND local_revision <= ?",
        )
        .bind(run_id)
        .bind(gate_id)
        .bind(client_id)
        .bind(content)
        .bind(local_revision)
        .execute(&self.pool)
        .await
        .map_err(map_sql)?;
        Ok(result.rows_affected() > 0)
    }

    async fn delete_chapter(&self, input: DeleteChapterRecord) -> Result<(), PortError> {
        let mut tx = self.pool.begin().await.map_err(map_sql)?;
        let current = sqlx::query(
            "SELECT r.version AS official_version
             FROM chapter c
             LEFT JOIN chapter_revision r ON r.id = c.official_revision_id
             WHERE c.id = ? AND c.novel_id = ?",
        )
        .bind(&input.chapter_id)
        .bind(&input.novel_id)
        .fetch_optional(tx.deref_mut())
        .await
        .map_err(map_sql)?
        .ok_or_else(|| PortError::NotFound("没有找到这一章。".into()))?;
        let version: Option<i64> = current.try_get("official_version").map_err(map_sql)?;
        if version.unwrap_or(0) != input.expected_version {
            return Err(PortError::Conflict(
                "章节内容已经变化，请重新打开后再删除。".into(),
            ));
        }
        invalidate_story_facts(&mut tx, &input.chapter_id, "", novel_platform_now()).await?;
        delete_chapter_knowledge(&mut tx, &input.chapter_id).await?;
        sqlx::query("DELETE FROM chapter_candidate WHERE chapter_id = ?")
            .bind(&input.chapter_id)
            .execute(tx.deref_mut())
            .await
            .map_err(map_sql)?;
        sqlx::query("DELETE FROM chapter_revision WHERE chapter_id = ?")
            .bind(&input.chapter_id)
            .execute(tx.deref_mut())
            .await
            .map_err(map_sql)?;
        sqlx::query("DELETE FROM chapter WHERE id = ? AND novel_id = ?")
            .bind(&input.chapter_id)
            .bind(&input.novel_id)
            .execute(tx.deref_mut())
            .await
            .map_err(map_sql)?;
        tx.commit().await.map_err(map_sql)?;
        Ok(())
    }

    async fn update_chapter_meta(
        &self,
        input: UpdateChapterMetaRecord,
    ) -> Result<ChapterSnapshot, PortError> {
        let mut tx = self.pool.begin().await.map_err(map_sql)?;
        let current = sqlx::query(
            "SELECT r.version AS official_version
             FROM chapter c LEFT JOIN chapter_revision r ON r.id = c.official_revision_id
             WHERE c.id = ? AND c.novel_id = ?",
        )
        .bind(&input.chapter_id)
        .bind(&input.novel_id)
        .fetch_optional(tx.deref_mut())
        .await
        .map_err(map_sql)?
        .ok_or_else(|| PortError::NotFound("没有找到这一章。".into()))?;
        let version: Option<i64> = current.try_get("official_version").map_err(map_sql)?;
        if version.unwrap_or(0) != input.expected_version {
            return Err(PortError::Conflict(
                "章节内容已经变化，请重新打开后再保存。".into(),
            ));
        }
        if let Some(volume_id) = input.volume_id.as_deref() {
            let belongs: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM volume WHERE id = ? AND novel_id = ?")
                    .bind(volume_id)
                    .bind(&input.novel_id)
                    .fetch_one(tx.deref_mut())
                    .await
                    .map_err(map_sql)?;
            if belongs == 0 {
                return Err(PortError::NotFound("没有找到这一卷。".into()));
            }
        }
        sqlx::query(
            "UPDATE chapter SET title = ?, sort_order = ?, volume_id = ?, updated_at = ?
             WHERE id = ? AND novel_id = ?",
        )
        .bind(&input.title)
        .bind(input.sort_order)
        .bind(input.volume_id.as_deref())
        .bind(novel_platform_now())
        .bind(&input.chapter_id)
        .bind(&input.novel_id)
        .execute(tx.deref_mut())
        .await
        .map_err(map_sql)?;
        tx.commit().await.map_err(map_sql)?;
        self.get_chapter(&input.novel_id, &input.chapter_id).await
    }

    async fn list_chapter_revisions(
        &self,
        novel_id: &str,
        chapter_id: &str,
    ) -> Result<Vec<ChapterRevisionSnapshot>, PortError> {
        let rows = sqlx::query(
            "SELECT r.id, r.chapter_id, r.version, r.origin, r.content, r.content_sha256, r.created_at
             FROM chapter_revision r
             JOIN chapter c ON c.id = r.chapter_id
             WHERE c.novel_id = ? AND c.id = ?
             ORDER BY r.version DESC",
        )
        .bind(novel_id)
        .bind(chapter_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sql)?;
        if rows.is_empty() {
            let exists: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM chapter WHERE id = ? AND novel_id = ?")
                    .bind(chapter_id)
                    .bind(novel_id)
                    .fetch_one(&self.pool)
                    .await
                    .map_err(map_sql)?;
            if exists == 0 {
                return Err(PortError::NotFound("没有找到这一章。".into()));
            }
        }
        rows.iter()
            .map(|row| {
                Ok(ChapterRevisionSnapshot {
                    id: row.try_get("id").map_err(map_sql)?,
                    chapter_id: row.try_get("chapter_id").map_err(map_sql)?,
                    version: row.try_get("version").map_err(map_sql)?,
                    origin: row.try_get("origin").map_err(map_sql)?,
                    content: row.try_get("content").map_err(map_sql)?,
                    content_sha256: row.try_get("content_sha256").map_err(map_sql)?,
                    created_at: row.try_get("created_at").map_err(map_sql)?,
                })
            })
            .collect()
    }

    async fn list_worlds_for_context(
        &self,
        novel_id: &str,
    ) -> Result<Vec<novel_domain::World>, PortError> {
        let rows = sqlx::query(
            "SELECT w.id, w.name, w.description, w.overview_summary, w.source_template_id,
                    w.owner_novel_id, w.status, w.created_at, w.updated_at
             FROM world w
             WHERE w.owner_novel_id = ?
                OR w.id = (SELECT world_id FROM novel WHERE id = ?)
             ORDER BY w.updated_at DESC, w.id ASC",
        )
        .bind(novel_id)
        .bind(novel_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sql)?;
        rows.iter()
            .map(|row| {
                Ok(novel_domain::World {
                    id: row.try_get("id").map_err(map_sql)?,
                    name: row.try_get("name").map_err(map_sql)?,
                    description: row.try_get("description").map_err(map_sql)?,
                    overview_summary: row.try_get("overview_summary").map_err(map_sql)?,
                    source_template_id: row.try_get("source_template_id").map_err(map_sql)?,
                    owner_novel_id: row.try_get("owner_novel_id").map_err(map_sql)?,
                    status: row.try_get("status").map_err(map_sql)?,
                    created_at: row.try_get("created_at").map_err(map_sql)?,
                    updated_at: row.try_get("updated_at").map_err(map_sql)?,
                })
            })
            .collect()
    }

    async fn list_story_facts(
        &self,
        novel_id: &str,
        before_chapter_order: i32,
    ) -> Result<Vec<StoryFactView>, PortError> {
        let mut facts = Vec::new();
        let states = sqlx::query(
            "SELECT cs.id, cs.body, cs.chapter_order, cs.source_revision, cs.status
             FROM character_state cs JOIN character c ON c.id = cs.character_id
             WHERE c.novel_id = ? AND cs.status = 'confirmed' AND cs.chapter_order <= ?
               AND cs.rowid = (
                 SELECT cs2.rowid FROM character_state cs2
                 WHERE cs2.character_id = cs.character_id AND cs2.chapter_order <= ?
                 ORDER BY cs2.chapter_order DESC, cs2.updated_at DESC, cs2.rowid DESC LIMIT 1
               )
             ORDER BY cs.chapter_order",
        )
        .bind(novel_id)
        .bind(before_chapter_order)
        .bind(before_chapter_order)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sql)?;
        for row in &states {
            facts.push(StoryFactView {
                id: row.try_get("id").map_err(map_sql)?,
                fact_type: "character_state".into(),
                body: row.try_get("body").map_err(map_sql)?,
                chapter_order: row.try_get("chapter_order").map_err(map_sql)?,
                source_revision: row.try_get("source_revision").map_err(map_sql)?,
                status: row.try_get("status").map_err(map_sql)?,
            });
        }
        let events = sqlx::query(
            "SELECT id, body, chapter_order, source_revision, status
             FROM timeline_event
             WHERE novel_id = ? AND status = 'confirmed' AND chapter_order <= ?
             ORDER BY chapter_order, id",
        )
        .bind(novel_id)
        .bind(before_chapter_order)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sql)?;
        for row in &events {
            facts.push(StoryFactView {
                id: row.try_get("id").map_err(map_sql)?,
                fact_type: "timeline_event".into(),
                body: row.try_get("body").map_err(map_sql)?,
                chapter_order: row.try_get("chapter_order").map_err(map_sql)?,
                source_revision: row.try_get("source_revision").map_err(map_sql)?,
                status: row.try_get("status").map_err(map_sql)?,
            });
        }
        let threads = sqlx::query(
            "SELECT id, title || '：' || body AS body, opened_chapter_order AS chapter_order,
                    source_revision, status
             FROM plot_thread
             WHERE novel_id = ? AND opened_chapter_order <= ? AND status IN ('open','resolved')
             ORDER BY opened_chapter_order, id",
        )
        .bind(novel_id)
        .bind(before_chapter_order)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sql)?;
        for row in &threads {
            facts.push(StoryFactView {
                id: row.try_get("id").map_err(map_sql)?,
                fact_type: "plot_thread".into(),
                body: row.try_get("body").map_err(map_sql)?,
                chapter_order: row.try_get("chapter_order").map_err(map_sql)?,
                source_revision: row.try_get("source_revision").map_err(map_sql)?,
                status: row.try_get("status").map_err(map_sql)?,
            });
        }
        Ok(facts)
    }

    async fn upsert_character_state(
        &self,
        state: novel_application::NewCharacterState,
    ) -> Result<(), PortError> {
        sqlx::query(
            "INSERT INTO character_state
               (id, character_id, chapter_order, body, created_at, source_revision, status,
                updated_at, source_chapter_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
               chapter_order = excluded.chapter_order, body = excluded.body,
               source_revision = excluded.source_revision, status = excluded.status,
               updated_at = excluded.updated_at, source_chapter_id = excluded.source_chapter_id",
        )
        .bind(&state.id)
        .bind(&state.character_id)
        .bind(state.chapter_order)
        .bind(&state.body)
        .bind(state.now_ms)
        .bind(&state.source_revision)
        .bind(&state.status)
        .bind(state.now_ms)
        .bind(&state.source_chapter_id)
        .execute(&self.pool)
        .await
        .map_err(map_sql)?;
        Ok(())
    }

    async fn upsert_timeline_event(
        &self,
        event: novel_application::NewTimelineEvent,
    ) -> Result<(), PortError> {
        sqlx::query(
            "INSERT INTO timeline_event
               (id, novel_id, chapter_order, body, created_at, source_revision, status,
                updated_at, source_chapter_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
               chapter_order = excluded.chapter_order, body = excluded.body,
               source_revision = excluded.source_revision, status = excluded.status,
               updated_at = excluded.updated_at, source_chapter_id = excluded.source_chapter_id",
        )
        .bind(&event.id)
        .bind(&event.novel_id)
        .bind(event.chapter_order)
        .bind(&event.body)
        .bind(event.now_ms)
        .bind(&event.source_revision)
        .bind(&event.status)
        .bind(event.now_ms)
        .bind(&event.source_chapter_id)
        .execute(&self.pool)
        .await
        .map_err(map_sql)?;
        Ok(())
    }

    async fn upsert_plot_thread(
        &self,
        thread: novel_application::NewPlotThread,
    ) -> Result<(), PortError> {
        sqlx::query(
            "INSERT INTO plot_thread
               (id, novel_id, title, body, opened_chapter_order, resolved_chapter_order,
                status, source_revision, evidence_json, created_at, updated_at, source_chapter_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
               title = excluded.title, body = excluded.body,
               opened_chapter_order = excluded.opened_chapter_order,
               resolved_chapter_order = excluded.resolved_chapter_order,
               status = excluded.status, source_revision = excluded.source_revision,
               updated_at = excluded.updated_at, source_chapter_id = excluded.source_chapter_id",
        )
        .bind(&thread.id)
        .bind(&thread.novel_id)
        .bind(&thread.title)
        .bind(&thread.body)
        .bind(thread.opened_chapter_order)
        .bind(thread.resolved_chapter_order)
        .bind(&thread.status)
        .bind(&thread.source_revision)
        .bind(thread.now_ms)
        .bind(thread.now_ms)
        .bind(&thread.source_chapter_id)
        .execute(&self.pool)
        .await
        .map_err(map_sql)?;
        Ok(())
    }

    async fn create_volume(&self, volume: NewVolume) -> Result<VolumeSnapshot, PortError> {
        let mut tx = self.pool.begin().await.map_err(map_sql)?;
        ensure_novel_exists(&mut tx, &volume.novel_id).await?;
        sqlx::query(
            "INSERT INTO volume(id, novel_id, title, sort_order, summary) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&volume.id)
        .bind(&volume.novel_id)
        .bind(&volume.title)
        .bind(volume.sort_order)
        .bind(&volume.summary)
        .execute(tx.deref_mut())
        .await
        .map_err(map_volume_sql)?;
        tx.commit().await.map_err(map_sql)?;
        Ok(VolumeSnapshot {
            id: volume.id,
            novel_id: volume.novel_id,
            title: volume.title,
            sort_order: volume.sort_order,
            summary: volume.summary,
        })
    }

    async fn list_volumes(&self, novel_id: &str) -> Result<Vec<VolumeSnapshot>, PortError> {
        let rows = sqlx::query(
            "SELECT id, novel_id, title, sort_order, summary
             FROM volume WHERE novel_id = ? ORDER BY sort_order",
        )
        .bind(novel_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sql)?;
        rows.iter()
            .map(|row| {
                Ok(VolumeSnapshot {
                    id: row.try_get("id").map_err(map_sql)?,
                    novel_id: row.try_get("novel_id").map_err(map_sql)?,
                    title: row.try_get("title").map_err(map_sql)?,
                    sort_order: row.try_get("sort_order").map_err(map_sql)?,
                    summary: row.try_get("summary").map_err(map_sql)?,
                })
            })
            .collect()
    }

    async fn update_volume(&self, input: UpdateVolumeRecord) -> Result<VolumeSnapshot, PortError> {
        let mut tx = self.pool.begin().await.map_err(map_sql)?;
        let affected = sqlx::query(
            "UPDATE volume SET title = ?, sort_order = ?, summary = ?
             WHERE id = ? AND novel_id = ?",
        )
        .bind(&input.title)
        .bind(input.sort_order)
        .bind(&input.summary)
        .bind(&input.volume_id)
        .bind(&input.novel_id)
        .execute(tx.deref_mut())
        .await
        .map_err(map_volume_sql)?
        .rows_affected();
        if affected == 0 {
            return Err(PortError::NotFound("没有找到这一卷。".into()));
        }
        tx.commit().await.map_err(map_sql)?;
        Ok(VolumeSnapshot {
            id: input.volume_id,
            novel_id: input.novel_id,
            title: input.title,
            sort_order: input.sort_order,
            summary: input.summary,
        })
    }

    async fn delete_volume(&self, novel_id: &str, volume_id: &str) -> Result<(), PortError> {
        let mut tx = self.pool.begin().await.map_err(map_sql)?;
        let chapters: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM chapter WHERE volume_id = ? AND novel_id = ?")
                .bind(volume_id)
                .bind(novel_id)
                .fetch_one(tx.deref_mut())
                .await
                .map_err(map_sql)?;
        if chapters > 0 {
            return Err(PortError::Conflict(
                "这一卷里还有章节，先移动或删除章节再删卷。".into(),
            ));
        }
        let affected = sqlx::query("DELETE FROM volume WHERE id = ? AND novel_id = ?")
            .bind(volume_id)
            .bind(novel_id)
            .execute(tx.deref_mut())
            .await
            .map_err(map_sql)?
            .rows_affected();
        if affected == 0 {
            return Err(PortError::NotFound("没有找到这一卷。".into()));
        }
        tx.commit().await.map_err(map_sql)?;
        Ok(())
    }

    async fn promote_candidate(
        &self,
        chapter_id: &str,
        candidate_id: &str,
        revision_id: &str,
        now_ms: i64,
    ) -> Result<PromoteOutcome, PortError> {
        let mut tx = self.pool.begin().await.map_err(map_sql)?;
        let chapter = sqlx::query("SELECT novel_id, protection FROM chapter WHERE id = ?")
            .bind(chapter_id)
            .fetch_optional(tx.deref_mut())
            .await
            .map_err(map_sql)?
            .ok_or_else(|| PortError::NotFound("没有找到这一章。".into()))?;
        let protection = ChapterProtection::parse(
            chapter
                .try_get::<String, _>("protection")
                .map_err(map_sql)?
                .as_str(),
        )
        .map_err(|_| PortError::Integrity("章节保护状态无法识别。".into()))?;
        if ensure_can_promote(protection).is_err() {
            return Ok(PromoteOutcome::Blocked);
        }
        let candidate = sqlx::query(
            "SELECT chapter_id, content, content_sha256 FROM chapter_candidate WHERE id = ?",
        )
        .bind(candidate_id)
        .fetch_optional(tx.deref_mut())
        .await
        .map_err(map_sql)?
        .ok_or_else(|| PortError::NotFound("没有找到候选正文。".into()))?;
        let candidate_chapter: String = candidate.try_get("chapter_id").map_err(map_sql)?;
        if candidate_chapter != chapter_id {
            return Err(PortError::Integrity("候选正文不属于这一章。".into()));
        }
        let version: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(version), 0) + 1 FROM chapter_revision WHERE chapter_id = ?",
        )
        .bind(chapter_id)
        .fetch_one(tx.deref_mut())
        .await
        .map_err(map_sql)?;
        sqlx::query("INSERT INTO chapter_revision(id, chapter_id, version, origin, content, content_sha256, created_at) VALUES (?, ?, ?, 'promoted', ?, ?, ?)")
            .bind(revision_id)
            .bind(chapter_id)
            .bind(version)
            .bind(candidate.try_get::<String, _>("content").map_err(map_sql)?)
            .bind(candidate.try_get::<String, _>("content_sha256").map_err(map_sql)?)
            .bind(now_ms)
            .execute(tx.deref_mut())
            .await
            .map_err(map_sql)?;
        sqlx::query("UPDATE chapter SET official_revision_id = ?, updated_at = ? WHERE id = ? AND protection = 'none'")
            .bind(revision_id)
            .bind(now_ms)
            .bind(chapter_id)
            .execute(tx.deref_mut())
            .await
            .map_err(map_sql)?;
        invalidate_story_facts(
            &mut tx,
            chapter_id,
            &format!("{}@v{}", revision_id, version),
            now_ms,
        )
        .await?;
        let content: String = candidate.try_get("content").map_err(map_sql)?;
        let chapter_title: String = sqlx::query_scalar("SELECT title FROM chapter WHERE id = ?")
            .bind(chapter_id)
            .fetch_one(tx.deref_mut())
            .await
            .map_err(map_sql)?;
        let chapter_novel: String = sqlx::query_scalar("SELECT novel_id FROM chapter WHERE id = ?")
            .bind(chapter_id)
            .fetch_one(tx.deref_mut())
            .await
            .map_err(map_sql)?;
        sync_chapter_knowledge(
            &mut tx,
            &chapter_novel,
            chapter_id,
            &chapter_title,
            &content,
            revision_id,
            version,
            now_ms,
        )
        .await?;
        tx.commit().await.map_err(map_sql)?;
        Ok(PromoteOutcome::Promoted { version })
    }
    async fn promote_candidate_fenced(
        &self,
        fence: &novel_application::LeaseFence,
        run_id: &str,
        chapter_id: &str,
        candidate_id: &str,
        revision_id: &str,
        now_ms: i64,
    ) -> Result<PromoteOutcome, PortError> {
        let mut tx = self.pool.begin().await.map_err(map_sql)?;
        let row = sqlx::query("SELECT lease_owner, lease_until, version, status FROM workflow_control WHERE run_id = ?")
            .bind(run_id).fetch_optional(tx.deref_mut()).await.map_err(map_sql)?;
        let Some(row) = row else {
            return Err(PortError::Conflict(
                "创作任务已不存在，执行权已失效。".into(),
            ));
        };
        let owner: Option<String> = row.try_get("lease_owner").map_err(map_sql)?;
        let until: Option<i64> = row.try_get("lease_until").map_err(map_sql)?;
        let version: i64 = row.try_get("version").map_err(map_sql)?;
        let status: String = row.try_get("status").map_err(map_sql)?;
        if owner.as_deref() != Some(fence.owner.as_str())
            || version != fence.control_version
            || until.is_some_and(|value| value < now_ms)
            || !matches!(status.as_str(), "running" | "queued")
        {
            return Err(PortError::Conflict(
                "执行权已失效，旧 Worker 的提交已被拒绝。".into(),
            ));
        }
        let chapter = sqlx::query("SELECT novel_id, protection FROM chapter WHERE id = ?")
            .bind(chapter_id)
            .fetch_optional(tx.deref_mut())
            .await
            .map_err(map_sql)?
            .ok_or_else(|| PortError::NotFound("没有找到这一章。".into()))?;
        let protection = ChapterProtection::parse(
            chapter
                .try_get::<String, _>("protection")
                .map_err(map_sql)?
                .as_str(),
        )
        .map_err(|_| PortError::Integrity("章节保护状态无法识别。".into()))?;
        if ensure_can_promote(protection).is_err() {
            return Ok(PromoteOutcome::Blocked);
        }
        let candidate = sqlx::query(
            "SELECT chapter_id, content, content_sha256 FROM chapter_candidate WHERE id = ?",
        )
        .bind(candidate_id)
        .fetch_optional(tx.deref_mut())
        .await
        .map_err(map_sql)?
        .ok_or_else(|| PortError::NotFound("没有找到候选正文。".into()))?;
        if candidate
            .try_get::<String, _>("chapter_id")
            .map_err(map_sql)?
            != chapter_id
        {
            return Err(PortError::Integrity("候选正文不属于这一章。".into()));
        }
        let version: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(version), 0) + 1 FROM chapter_revision WHERE chapter_id = ?",
        )
        .bind(chapter_id)
        .fetch_one(tx.deref_mut())
        .await
        .map_err(map_sql)?;
        sqlx::query("INSERT INTO chapter_revision(id, chapter_id, version, origin, content, content_sha256, created_at) VALUES (?, ?, ?, 'promoted', ?, ?, ?)")
            .bind(revision_id).bind(chapter_id).bind(version)
            .bind(candidate.try_get::<String, _>("content").map_err(map_sql)?)
            .bind(candidate.try_get::<String, _>("content_sha256").map_err(map_sql)?)
            .bind(now_ms).execute(tx.deref_mut()).await.map_err(map_sql)?;
        sqlx::query("UPDATE chapter SET official_revision_id = ?, updated_at = ? WHERE id = ? AND protection = 'none'")
            .bind(revision_id).bind(now_ms).bind(chapter_id).execute(tx.deref_mut()).await.map_err(map_sql)?;
        tx.commit().await.map_err(map_sql)?;
        Ok(PromoteOutcome::Promoted { version })
    }
}

async fn invalidate_story_facts(
    tx: &mut crate::db::Tx<'_>,
    chapter_id: &str,
    new_revision: &str,
    now_ms: i64,
) -> Result<(), PortError> {
    sqlx::query(
        "UPDATE character_state SET status = 'superseded', updated_at = ?
         WHERE source_chapter_id = ? AND status = 'confirmed' AND source_revision != ?",
    )
    .bind(now_ms)
    .bind(chapter_id)
    .bind(new_revision)
    .execute(tx.deref_mut())
    .await
    .map_err(map_sql)?;
    sqlx::query(
        "UPDATE timeline_event SET status = 'superseded', updated_at = ?
         WHERE source_chapter_id = ? AND status = 'confirmed' AND source_revision != ?",
    )
    .bind(now_ms)
    .bind(chapter_id)
    .bind(new_revision)
    .execute(tx.deref_mut())
    .await
    .map_err(map_sql)?;
    sqlx::query(
        "UPDATE plot_thread SET status = 'superseded', updated_at = ?
         WHERE source_chapter_id = ? AND status IN ('open','resolved') AND source_revision != ?",
    )
    .bind(now_ms)
    .bind(chapter_id)
    .bind(new_revision)
    .execute(tx.deref_mut())
    .await
    .map_err(map_sql)?;
    Ok(())
}

async fn sync_chapter_knowledge(
    tx: &mut crate::db::Tx<'_>,
    novel_id: &str,
    chapter_id: &str,
    title: &str,
    content: &str,
    source_revision: &str,
    source_version: i64,
    now_ms: i64,
) -> Result<(), PortError> {
    use sha2::{Digest, Sha256};
    let document_id = format!("chapter:{chapter_id}");
    let source_revision = format!("{source_revision}@v{source_version}");
    let content_hash = hex::encode(Sha256::digest(content.as_bytes()));
    let terms = fts_query(&tokenize(content, &Lexicon::default()));
    let existing: Option<i64> =
        sqlx::query_scalar("SELECT version FROM knowledge_document WHERE id = ?")
            .bind(&document_id)
            .fetch_optional(tx.deref_mut())
            .await
            .map_err(map_sql)?;
    let next_version = existing.unwrap_or(0) + 1;
    sqlx::query(
        "INSERT INTO knowledge_document
           (id, novel_id, owner_type, title, body, version, index_status, created_at,
            source_revision, source_content_hash, index_version, indexed_at)
         VALUES (?, ?, 'chapter', ?, ?, ?, 'ready', ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
           novel_id = excluded.novel_id, owner_type = 'chapter', title = excluded.title,
           body = excluded.body, version = excluded.version, index_status = 'ready',
           source_revision = excluded.source_revision,
           source_content_hash = excluded.source_content_hash,
           index_version = excluded.index_version, indexed_at = excluded.indexed_at",
    )
    .bind(&document_id)
    .bind(novel_id)
    .bind(title)
    .bind(content)
    .bind(next_version)
    .bind(now_ms)
    .bind(&source_revision)
    .bind(&content_hash)
    .bind(next_version)
    .bind(now_ms)
    .execute(tx.deref_mut())
    .await
    .map_err(map_sql)?;
    let chunks: Vec<String> =
        sqlx::query_scalar("SELECT id FROM knowledge_chunk WHERE document_id = ?")
            .bind(&document_id)
            .fetch_all(tx.deref_mut())
            .await
            .map_err(map_sql)?;
    for chunk in &chunks {
        sqlx::query("DELETE FROM knowledge_fts WHERE chunk_id = ?")
            .bind(chunk)
            .execute(tx.deref_mut())
            .await
            .map_err(map_sql)?;
        sqlx::query("DELETE FROM embedding WHERE chunk_id = ?")
            .bind(chunk)
            .execute(tx.deref_mut())
            .await
            .map_err(map_sql)?;
    }
    sqlx::query("DELETE FROM knowledge_chunk WHERE document_id = ?")
        .bind(&document_id)
        .execute(tx.deref_mut())
        .await
        .map_err(map_sql)?;
    let chunk_id = format!("{document_id}:0");
    sqlx::query("INSERT INTO knowledge_chunk(id, document_id, ordinal, body, terms, source_revision, chunk_hash) VALUES (?, ?, 0, ?, ?, ?, ?)")
        .bind(&chunk_id).bind(&document_id).bind(content).bind(&terms)
        .bind(&source_revision).bind(&content_hash).execute(tx.deref_mut()).await.map_err(map_sql)?;
    sqlx::query("INSERT INTO knowledge_fts(chunk_id, terms) VALUES (?, ?)")
        .bind(&chunk_id)
        .bind(&terms)
        .execute(tx.deref_mut())
        .await
        .map_err(map_sql)?;
    Ok(())
}

async fn delete_chapter_knowledge(
    tx: &mut crate::db::Tx<'_>,
    chapter_id: &str,
) -> Result<(), PortError> {
    let document_id = format!("chapter:{chapter_id}");
    let chunks: Vec<String> =
        sqlx::query_scalar("SELECT id FROM knowledge_chunk WHERE document_id = ?")
            .bind(&document_id)
            .fetch_all(tx.deref_mut())
            .await
            .map_err(map_sql)?;
    for chunk in &chunks {
        sqlx::query("DELETE FROM knowledge_fts WHERE chunk_id = ?")
            .bind(chunk)
            .execute(tx.deref_mut())
            .await
            .map_err(map_sql)?;
        sqlx::query("DELETE FROM embedding WHERE chunk_id = ?")
            .bind(chunk)
            .execute(tx.deref_mut())
            .await
            .map_err(map_sql)?;
    }
    sqlx::query("DELETE FROM knowledge_chunk WHERE document_id = ?")
        .bind(&document_id)
        .execute(tx.deref_mut())
        .await
        .map_err(map_sql)?;
    sqlx::query("DELETE FROM knowledge_document WHERE id = ?")
        .bind(&document_id)
        .execute(tx.deref_mut())
        .await
        .map_err(map_sql)?;
    Ok(())
}

fn novel_platform_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

async fn ensure_novel_exists(tx: &mut crate::db::Tx<'_>, novel_id: &str) -> Result<(), PortError> {
    let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM novel WHERE id = ?")
        .bind(novel_id)
        .fetch_one(tx.deref_mut())
        .await
        .map_err(map_sql)?;
    if exists == 0 {
        return Err(PortError::NotFound("没有找到这本书。".into()));
    }
    Ok(())
}

fn map_volume_sql(error: sqlx::Error) -> PortError {
    let text = error.to_string();
    if text.contains("UNIQUE constraint failed: volume.novel_id, volume.sort_order") {
        PortError::Conflict("这个卷序已经被占用，请换一个顺序。".into())
    } else {
        map_sql(error)
    }
}

fn candidate_edit_from_row(
    row: &sqlx::sqlite::SqliteRow,
) -> Result<CandidateEditDraftView, PortError> {
    Ok(CandidateEditDraftView {
        novel_id: row.try_get("novel_id").map_err(map_sql)?,
        chapter_id: row.try_get("chapter_id").map_err(map_sql)?,
        run_id: row.try_get("run_id").map_err(map_sql)?,
        gate_id: row.try_get("gate_id").map_err(map_sql)?,
        client_id: row.try_get("client_id").map_err(map_sql)?,
        base_candidate_id: row.try_get("base_candidate_id").map_err(map_sql)?,
        base_content_sha256: row.try_get("base_content_sha256").map_err(map_sql)?,
        content: row.try_get("content").map_err(map_sql)?,
        local_revision: row.try_get("local_revision").map_err(map_sql)?,
        control_version: row.try_get("control_version").map_err(map_sql)?,
        updated_at: row.try_get("updated_at").map_err(map_sql)?,
    })
}

fn draft_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<ChapterDraftView, PortError> {
    Ok(ChapterDraftView {
        novel_id: row.try_get("novel_id").map_err(map_sql)?,
        chapter_id: row.try_get("chapter_id").map_err(map_sql)?,
        client_id: row.try_get("client_id").map_err(map_sql)?,
        content: row.try_get("content").map_err(map_sql)?,
        base_official_version: row.try_get("base_official_version").map_err(map_sql)?,
        base_updated_at: row.try_get("base_updated_at").map_err(map_sql)?,
        local_revision: row.try_get("local_revision").map_err(map_sql)?,
        updated_at: row.try_get("updated_at").map_err(map_sql)?,
    })
}

fn novel_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<NovelSummary, PortError> {
    Ok(NovelSummary {
        id: row.try_get("id").map_err(map_sql)?,
        title: row.try_get("title").map_err(map_sql)?,
        description: row.try_get("description").map_err(map_sql)?,
        status: row.try_get("status").map_err(map_sql)?,
        creation_experience: row.try_get("creation_experience").map_err(map_sql)?,
        narrative_form: row.try_get("narrative_form").map_err(map_sql)?,
        writing_mode: row.try_get("writing_mode").map_err(map_sql)?,
        project_mode: row.try_get("project_mode").map_err(map_sql)?,
        updated_at: row.try_get("updated_at").map_err(map_sql)?,
    })
}

fn novel_detail_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<NovelDetail, PortError> {
    let commercial_tags_json: String = row.try_get("commercial_tags_json").map_err(map_sql)?;
    let commercial_tags =
        serde_json::from_str::<Vec<String>>(&commercial_tags_json).unwrap_or_default();
    Ok(NovelDetail {
        id: row.try_get("id").map_err(map_sql)?,
        title: row.try_get("title").map_err(map_sql)?,
        description: row.try_get("description").map_err(map_sql)?,
        status: row.try_get("status").map_err(map_sql)?,
        creation_experience: row.try_get("creation_experience").map_err(map_sql)?,
        narrative_form: row.try_get("narrative_form").map_err(map_sql)?,
        target_audience: row.try_get("target_audience").map_err(map_sql)?,
        book_selling_point: row.try_get("book_selling_point").map_err(map_sql)?,
        competing_feel: row.try_get("competing_feel").map_err(map_sql)?,
        first_30_chapter_promise: row.try_get("first_30_chapter_promise").map_err(map_sql)?,
        commercial_tags,
        genre_id: row.try_get("genre_id").map_err(map_sql)?,
        primary_story_mode_id: row.try_get("primary_story_mode_id").map_err(map_sql)?,
        secondary_story_mode_id: row.try_get("secondary_story_mode_id").map_err(map_sql)?,
        world_id: row.try_get("world_id").map_err(map_sql)?,
        writing_mode: row.try_get("writing_mode").map_err(map_sql)?,
        project_mode: row.try_get("project_mode").map_err(map_sql)?,
        narrative_pov: row.try_get("narrative_pov").map_err(map_sql)?,
        pace_preference: row.try_get("pace_preference").map_err(map_sql)?,
        style_tone: row.try_get("style_tone").map_err(map_sql)?,
        emotion_intensity: row.try_get("emotion_intensity").map_err(map_sql)?,
        ai_freedom: row.try_get("ai_freedom").map_err(map_sql)?,
        default_chapter_length: row.try_get("default_chapter_length").map_err(map_sql)?,
        estimated_chapter_count: row.try_get("estimated_chapter_count").map_err(map_sql)?,
        project_status: row.try_get("project_status").map_err(map_sql)?,
        storyline_status: row.try_get("storyline_status").map_err(map_sql)?,
        outline_status: row.try_get("outline_status").map_err(map_sql)?,
        resource_ready_score: row.try_get("resource_ready_score").map_err(map_sql)?,
        post_generation_style_review_enabled: row
            .try_get::<i64, _>("post_generation_style_review_enabled")
            .map_err(map_sql)?
            != 0,
        created_at: row.try_get("created_at").map_err(map_sql)?,
        updated_at: row.try_get("updated_at").map_err(map_sql)?,
    })
}

fn chapter_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<ChapterSnapshot, PortError> {
    let version: Option<i64> = row.try_get("official_version").map_err(map_sql)?;
    let content: Option<String> = row.try_get("content").map_err(map_sql)?;
    Ok(ChapterSnapshot {
        id: row.try_get("id").map_err(map_sql)?,
        novel_id: row.try_get("novel_id").map_err(map_sql)?,
        title: row.try_get("title").map_err(map_sql)?,
        sort_order: row.try_get("sort_order").map_err(map_sql)?,
        protection: row.try_get("protection").map_err(map_sql)?,
        official_version: version.unwrap_or(0),
        content: content.unwrap_or_default(),
        updated_at: row.try_get("updated_at").map_err(map_sql)?,
    })
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_artifact(
    tx: &mut crate::db::Tx<'_>,
    id: &str,
    novel_id: &str,
    run_id: Option<&str>,
    artifact_type: &str,
    scope: &str,
    status: &str,
    protected: bool,
    payload: &str,
    hash: &str,
    created_at: i64,
) -> Result<(), PortError> {
    let version: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(version), 0) + 1 FROM workflow_artifact WHERE novel_id = ? AND type = ? AND scope = ?")
        .bind(novel_id)
        .bind(artifact_type)
        .bind(scope)
        .fetch_one(tx.deref_mut())
        .await
        .map_err(map_sql)?;
    sqlx::query(
        "INSERT INTO workflow_artifact(id, novel_id, run_id, type, scope, version, status, protected_user_content, payload_json, content_sha256, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(novel_id)
    .bind(run_id)
    .bind(artifact_type)
    .bind(scope)
    .bind(version)
    .bind(status)
    .bind(i64::from(u8::from(protected)))
    .bind(payload)
    .bind(hash)
    .bind(created_at)
    .execute(tx.deref_mut())
    .await
    .map_err(map_sql)?;
    Ok(())
}
