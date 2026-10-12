use std::ops::DerefMut;

use async_trait::async_trait;
use novel_application::{
    ChapterSnapshot, NewChapter, NewNovel, NovelRepository, NovelSummary, PortError,
    PromoteOutcome, SaveChapterRecord,
};
use novel_domain::{ChapterProtection, ensure_can_promote};
use novel_workflow::chapter_scope;
use sqlx::Row;

use crate::db::{SqliteKernel, map_sql};

#[async_trait]
impl NovelRepository for SqliteKernel {
    async fn list_novels(&self) -> Result<Vec<NovelSummary>, PortError> {
        let rows = sqlx::query(
            "SELECT id, title, description, status, updated_at FROM novel ORDER BY updated_at DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sql)?;
        rows.iter().map(novel_from_row).collect()
    }

    async fn create_novel(&self, novel: NewNovel) -> Result<NovelSummary, PortError> {
        sqlx::query("INSERT INTO novel(id, title, description, status, created_at, updated_at) VALUES (?, ?, ?, 'draft', ?, ?)")
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
            updated_at: novel.now_ms,
        })
    }

    async fn create_chapter(&self, chapter: NewChapter) -> Result<ChapterSnapshot, PortError> {
        sqlx::query("INSERT INTO chapter(id, novel_id, title, sort_order, protection, created_at, updated_at) VALUES (?, ?, ?, ?, 'none', ?, ?)")
            .bind(&chapter.id)
            .bind(&chapter.novel_id)
            .bind(&chapter.title)
            .bind(chapter.sort_order)
            .bind(chapter.now_ms)
            .bind(chapter.now_ms)
            .execute(&self.pool)
            .await
            .map_err(map_sql)?;
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
        tx.commit().await.map_err(map_sql)?;
        self.get_chapter(&input.novel_id, &input.chapter_id).await
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
        tx.commit().await.map_err(map_sql)?;
        Ok(PromoteOutcome::Promoted { version })
    }
}

fn novel_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<NovelSummary, PortError> {
    Ok(NovelSummary {
        id: row.try_get("id").map_err(map_sql)?,
        title: row.try_get("title").map_err(map_sql)?,
        description: row.try_get("description").map_err(map_sql)?,
        status: row.try_get("status").map_err(map_sql)?,
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
