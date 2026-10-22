use std::ops::DerefMut;

use async_trait::async_trait;
use novel_application::{
    KnowledgeDocumentSummary, KnowledgeRepository, NewDocument, PortError, RetrievalTrace,
    SearchHit,
};
use novel_context::{cosine, decode_f32, encode_f32};
use sqlx::Row;

use crate::db::{SqliteKernel, map_sql};

#[async_trait]
impl KnowledgeRepository for SqliteKernel {
    async fn index_document(&self, document: NewDocument) -> Result<(), PortError> {
        let mut tx = self.pool.begin().await.map_err(map_sql)?;
        sqlx::query(
            "INSERT INTO knowledge_document(id, novel_id, owner_type, title, body, version, index_status, created_at)
             VALUES (?, ?, ?, ?, ?, 1, 'ready', ?)",
        )
        .bind(&document.id)
        .bind(&document.novel_id)
        .bind(&document.owner_type)
        .bind(&document.title)
        .bind(&document.body)
        .bind(document.created_at)
        .execute(tx.deref_mut())
        .await
        .map_err(map_sql)?;
        let chunk_id = format!("{}:0", document.id);
        sqlx::query("INSERT INTO knowledge_chunk(id, document_id, ordinal, body, terms) VALUES (?, ?, 0, ?, ?)")
            .bind(&chunk_id)
            .bind(&document.id)
            .bind(&document.body)
            .bind(&document.terms)
            .execute(tx.deref_mut())
            .await
            .map_err(map_sql)?;
        sqlx::query("INSERT INTO knowledge_fts(chunk_id, terms) VALUES (?, ?)")
            .bind(&chunk_id)
            .bind(&document.terms)
            .execute(tx.deref_mut())
            .await
            .map_err(map_sql)?;
        if let Some(embedding) = &document.embedding {
            sqlx::query("INSERT INTO embedding(chunk_id, model, dimensions, normalized, vector_blob) VALUES (?, ?, ?, ?, ?)")
                .bind(&chunk_id)
                .bind(&embedding.model)
                .bind(embedding.vector.len() as i64)
                .bind(i64::from(u8::from(embedding.normalized)))
                .bind(encode_f32(&embedding.vector))
                .execute(tx.deref_mut())
                .await
                .map_err(map_sql)?;
        }
        tx.commit().await.map_err(map_sql)?;
        Ok(())
    }

    async fn list_documents(&self) -> Result<Vec<KnowledgeDocumentSummary>, PortError> {
        let rows = sqlx::query(
            "SELECT id, novel_id, owner_type, title, version, index_status, created_at
             FROM knowledge_document ORDER BY created_at DESC LIMIT 500",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sql)?;
        rows.iter()
            .map(|row| {
                Ok(KnowledgeDocumentSummary {
                    id: row.try_get("id").map_err(map_sql)?,
                    novel_id: row.try_get("novel_id").map_err(map_sql)?,
                    owner_type: row.try_get("owner_type").map_err(map_sql)?,
                    title: row.try_get("title").map_err(map_sql)?,
                    version: row.try_get("version").map_err(map_sql)?,
                    index_status: row.try_get("index_status").map_err(map_sql)?,
                    created_at: row.try_get("created_at").map_err(map_sql)?,
                })
            })
            .collect()
    }

    async fn search_keyword(
        &self,
        novel_id: &str,
        query: &str,
        limit: i64,
    ) -> Result<Vec<SearchHit>, PortError> {
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }
        let rows = sqlx::query(
            "SELECT knowledge_fts.chunk_id, d.owner_type, c.body
             FROM knowledge_fts
             JOIN knowledge_chunk c ON c.id = knowledge_fts.chunk_id
             JOIN knowledge_document d ON d.id = c.document_id
             WHERE d.novel_id = ? AND knowledge_fts MATCH ?
             LIMIT ?",
        )
        .bind(novel_id)
        .bind(query)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sql)?;
        rows.iter().map(hit_from_row).collect()
    }

    async fn search_vector(
        &self,
        novel_id: &str,
        model: &str,
        vector: &[f32],
        normalized: bool,
        limit: i64,
    ) -> Result<Vec<SearchHit>, PortError> {
        let rows = sqlx::query(
            "SELECT c.id AS chunk_id, d.owner_type, c.body, e.vector_blob
             FROM embedding e
             JOIN knowledge_chunk c ON c.id = e.chunk_id
             JOIN knowledge_document d ON d.id = c.document_id
             WHERE d.novel_id = ? AND e.model = ? AND e.dimensions = ? AND e.normalized = ?",
        )
        .bind(novel_id)
        .bind(model)
        .bind(vector.len() as i64)
        .bind(i64::from(u8::from(normalized)))
        .fetch_all(&self.pool)
        .await
        .map_err(map_sql)?;
        let mut scored = Vec::new();
        for row in rows {
            let blob: Vec<u8> = row.try_get("vector_blob").map_err(map_sql)?;
            let stored =
                decode_f32(&blob).map_err(|_| PortError::Integrity("向量维度无法读取。".into()))?;
            let score = cosine(vector, &stored)
                .map_err(|_| PortError::Integrity("向量维度不一致，不能混在一起检索。".into()))?;
            scored.push((score, hit_from_row(&row)?));
        }
        scored.sort_by(|left, right| {
            right
                .0
                .partial_cmp(&left.0)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        scored.truncate(usize::try_from(limit).unwrap_or(0));
        Ok(scored.into_iter().map(|(_, hit)| hit).collect())
    }

    async fn record_trace(&self, trace: RetrievalTrace) -> Result<(), PortError> {
        let ids = serde_json::to_string(&trace.chunk_ids)
            .map_err(|_| PortError::Integrity("检索记录无法保存。".into()))?;
        sqlx::query("INSERT INTO retrieval_trace(id, novel_id, query_summary, chunk_ids_json, created_at) VALUES (?, ?, ?, ?, ?)")
            .bind(&trace.id)
            .bind(&trace.novel_id)
            .bind(&trace.query_summary)
            .bind(ids)
            .bind(trace.created_at)
            .execute(&self.pool)
            .await
            .map_err(map_sql)?;
        Ok(())
    }

    async fn latest_trace(&self, novel_id: &str) -> Result<Option<RetrievalTrace>, PortError> {
        let row = sqlx::query("SELECT id, novel_id, query_summary, chunk_ids_json, created_at FROM retrieval_trace WHERE novel_id = ? ORDER BY created_at DESC LIMIT 1")
            .bind(novel_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_sql)?;
        let Some(row) = row else { return Ok(None) };
        let ids: String = row.try_get("chunk_ids_json").map_err(map_sql)?;
        Ok(Some(RetrievalTrace {
            id: row.try_get("id").map_err(map_sql)?,
            novel_id: row.try_get("novel_id").map_err(map_sql)?,
            query_summary: row.try_get("query_summary").map_err(map_sql)?,
            chunk_ids: serde_json::from_str(&ids)
                .map_err(|_| PortError::Integrity("检索记录无法读取。".into()))?,
            created_at: row.try_get("created_at").map_err(map_sql)?,
        }))
    }
}

fn hit_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<SearchHit, PortError> {
    Ok(SearchHit {
        chunk_id: row.try_get("chunk_id").map_err(map_sql)?,
        owner_type: row.try_get("owner_type").map_err(map_sql)?,
        body: row.try_get("body").map_err(map_sql)?,
    })
}
