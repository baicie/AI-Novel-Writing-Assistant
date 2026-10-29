use async_trait::async_trait;
use novel_application::{NewWorld, PortError, UpdateWorldFields, WorldRepository};
use novel_domain::World;
use sqlx::Row;

use crate::db::{SqliteKernel, map_sql};
#[async_trait]
impl WorldRepository for SqliteKernel {
    async fn list_worlds(&self) -> Result<Vec<World>, PortError> {
        let rows = sqlx::query(
            "SELECT id, name, description, overview_summary, source_template_id, owner_novel_id, status, created_at, updated_at
             FROM world ORDER BY updated_at DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sql)?;
        rows.iter().map(world_from_row).collect()
    }

    async fn get_world(&self, id: &str) -> Result<Option<World>, PortError> {
        let row = sqlx::query(
            "SELECT id, name, description, overview_summary, source_template_id, owner_novel_id, status, created_at, updated_at
             FROM world WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sql)?;
        row.as_ref().map(world_from_row).transpose()
    }

    async fn create_world(&self, world: NewWorld) -> Result<World, PortError> {
        sqlx::query(
            "INSERT INTO world(id, name, description, overview_summary, source_template_id, owner_novel_id, status, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, 'draft', ?, ?)",
        )
        .bind(&world.id)
        .bind(&world.name)
        .bind(&world.description)
        .bind(&world.overview_summary)
        .bind(&world.source_template_id)
        .bind(&world.owner_novel_id)
        .bind(world.now_ms)
        .bind(world.now_ms)
        .execute(&self.pool)
        .await
        .map_err(map_sql)?;
        self.get_world(&world.id)
            .await?
            .ok_or_else(|| PortError::Storage("新建世界后回查失败。".into()))
    }

    async fn update_world(
        &self,
        id: &str,
        fields: &UpdateWorldFields,
        now_ms: i64,
    ) -> Result<World, PortError> {
        if let Some(value) = &fields.name {
            sqlx::query("UPDATE world SET name = ?, updated_at = ? WHERE id = ?")
                .bind(value)
                .bind(now_ms)
                .bind(id)
                .execute(&self.pool)
                .await
                .map_err(map_sql)?;
        }
        if let Some(value) = &fields.description {
            sqlx::query("UPDATE world SET description = ?, updated_at = ? WHERE id = ?")
                .bind(value.as_deref())
                .bind(now_ms)
                .bind(id)
                .execute(&self.pool)
                .await
                .map_err(map_sql)?;
        }
        if let Some(value) = &fields.overview_summary {
            sqlx::query("UPDATE world SET overview_summary = ?, updated_at = ? WHERE id = ?")
                .bind(value.as_deref())
                .bind(now_ms)
                .bind(id)
                .execute(&self.pool)
                .await
                .map_err(map_sql)?;
        }
        if let Some(value) = &fields.source_template_id {
            sqlx::query("UPDATE world SET source_template_id = ?, updated_at = ? WHERE id = ?")
                .bind(value.as_deref())
                .bind(now_ms)
                .bind(id)
                .execute(&self.pool)
                .await
                .map_err(map_sql)?;
        }
        if let Some(value) = &fields.owner_novel_id {
            sqlx::query("UPDATE world SET owner_novel_id = ?, updated_at = ? WHERE id = ?")
                .bind(value.as_deref())
                .bind(now_ms)
                .bind(id)
                .execute(&self.pool)
                .await
                .map_err(map_sql)?;
        }
        if let Some(value) = fields.status.as_deref() {
            sqlx::query("UPDATE world SET status = ?, updated_at = ? WHERE id = ?")
                .bind(value)
                .bind(now_ms)
                .bind(id)
                .execute(&self.pool)
                .await
                .map_err(map_sql)?;
        }
        self.get_world(id)
            .await?
            .ok_or_else(|| PortError::NotFound("没有找到这个世界观。".into()))
    }

    async fn delete_world(&self, id: &str) -> Result<(), PortError> {
        let result = sqlx::query("DELETE FROM world WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_sql)?;
        if result.rows_affected() == 0 {
            return Err(PortError::NotFound("没有找到这个世界观。".into()));
        }
        Ok(())
    }
}

fn world_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<World, PortError> {
    Ok(World {
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
}
