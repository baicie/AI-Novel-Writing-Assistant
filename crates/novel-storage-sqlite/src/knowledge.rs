use std::ops::DerefMut;

use async_trait::async_trait;
use novel_application::{
    KnowledgeDocumentSummary, KnowledgeRepository, NewDocument, PortError, RetrievalTrace,
    SearchHit,
};
use novel_context::{Lexicon, cosine, decode_f32, encode_f32, fts_query, tokenize};
use sha2::{Digest, Sha256};
use sqlx::Row;

use crate::db::{SqliteKernel, map_sql};

#[async_trait]
impl KnowledgeRepository for SqliteKernel {
    async fn index_document(&self, document: NewDocument) -> Result<(), PortError> {
        let source_hash = document
            .source_content_hash
            .clone()
            .unwrap_or_else(|| hex::encode(Sha256::digest(document.body.as_bytes())));
        // Phase 1: mark the document as rebuilding before touching chunks. Once
        // this commits, stale chunks are no longer returned to readers even if the
        // rebuild transaction fails below.
        let existing: Option<(
            i64,
            String,
            String,
            String,
            Option<String>,
            String,
            String,
            Option<String>,
        )> = sqlx::query_as(
            "SELECT version, title, body, owner_type, source_content_hash, index_status,
                        novel_id, source_revision
             FROM knowledge_document WHERE id = ?",
        )
        .bind(&document.id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sql)?;
        if let Some((_version, _title, _body, _owner, _hash, _status, existing_novel, _revision)) =
            &existing
            && existing_novel != &document.novel_id
        {
            return Err(PortError::Conflict(
                "knowledge document 已属于另一本小说，不能跨小说覆盖。".into(),
            ));
        }
        if let (Some((_, _, _, _, _, _, _, Some(existing_revision))), Some(incoming_revision)) =
            (&existing, document.source_revision.as_deref())
            && let (Some(existing_rank), Some(incoming_rank)) = (
                revision_rank(existing_revision),
                revision_rank(incoming_revision),
            )
            && incoming_rank < existing_rank
        {
            return Err(PortError::Conflict(
                "知识来源 revision 已过期，不能覆盖较新的索引。".into(),
            ));
        }
        if let Some((
            _version,
            title,
            body,
            owner,
            existing_hash,
            status,
            _novel,
            existing_revision,
        )) = &existing
            && title == &document.title
            && body == &document.body
            && owner == &document.owner_type
            && existing_hash.as_deref() == Some(source_hash.as_str())
            && status == "ready"
            && existing_revision.as_deref() == document.source_revision.as_deref()
        {
            return Ok(());
        }
        let next_version = existing.as_ref().map(|row| row.0 + 1).unwrap_or(1);
        let source_revision = document
            .source_revision
            .clone()
            .unwrap_or_else(|| format!("v{next_version}"));
        sqlx::query(
            "INSERT INTO knowledge_document
               (id, novel_id, owner_type, title, body, version, index_status, created_at,
                source_revision, source_content_hash, index_version, indexed_at)
             VALUES (?, ?, ?, ?, ?, ?, 'building', ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
               novel_id = excluded.novel_id,
               owner_type = excluded.owner_type,
               title = excluded.title,
               body = excluded.body,
               version = excluded.version,
               index_status = 'building',
               source_revision = excluded.source_revision,
               source_content_hash = excluded.source_content_hash,
               index_version = excluded.index_version,
               indexed_at = excluded.indexed_at",
        )
        .bind(&document.id)
        .bind(&document.novel_id)
        .bind(&document.owner_type)
        .bind(&document.title)
        .bind(&document.body)
        .bind(next_version)
        .bind(document.created_at)
        .bind(&source_revision)
        .bind(&source_hash)
        .bind(next_version)
        .bind(document.created_at)
        .execute(&self.pool)
        .await
        .map_err(map_sql)?;

        // Phase 2: replace all derived rows atomically.
        let old_chunks: Vec<String> =
            sqlx::query_scalar("SELECT id FROM knowledge_chunk WHERE document_id = ?")
                .bind(&document.id)
                .fetch_all(&self.pool)
                .await
                .map_err(map_sql)?;
        let mut tx = self.pool.begin().await.map_err(map_sql)?;
        for chunk_id in &old_chunks {
            sqlx::query("DELETE FROM knowledge_fts WHERE chunk_id = ?")
                .bind(chunk_id)
                .execute(tx.deref_mut())
                .await
                .map_err(map_sql)?;
            sqlx::query("DELETE FROM embedding WHERE chunk_id = ?")
                .bind(chunk_id)
                .execute(tx.deref_mut())
                .await
                .map_err(map_sql)?;
        }
        sqlx::query("DELETE FROM knowledge_chunk WHERE document_id = ?")
            .bind(&document.id)
            .execute(tx.deref_mut())
            .await
            .map_err(map_sql)?;
        let chunk_id = format!("{}:0", document.id);
        let chunk_hash = hex::encode(Sha256::digest(document.body.as_bytes()));
        sqlx::query("INSERT INTO knowledge_chunk(id, document_id, ordinal, body, terms, source_revision, chunk_hash) VALUES (?, ?, 0, ?, ?, ?, ?)")
            .bind(&chunk_id).bind(&document.id).bind(&document.body).bind(&document.terms)
            .bind(&source_revision).bind(&chunk_hash)
            .execute(tx.deref_mut()).await.map_err(map_sql)?;
        sqlx::query("INSERT INTO knowledge_fts(chunk_id, terms) VALUES (?, ?)")
            .bind(&chunk_id)
            .bind(&document.terms)
            .execute(tx.deref_mut())
            .await
            .map_err(map_sql)?;
        if let Some(embedding) = &document.embedding {
            sqlx::query("INSERT INTO embedding(chunk_id, model, dimensions, normalized, vector_blob) VALUES (?, ?, ?, ?, ?)")
                .bind(&chunk_id).bind(&embedding.model).bind(embedding.vector.len() as i64)
                .bind(i64::from(u8::from(embedding.normalized))).bind(encode_f32(&embedding.vector))
                .execute(tx.deref_mut()).await.map_err(map_sql)?;
        }
        sqlx::query(
            "UPDATE knowledge_document SET index_status = 'ready', indexed_at = ? WHERE id = ?",
        )
        .bind(document.created_at)
        .bind(&document.id)
        .execute(tx.deref_mut())
        .await
        .map_err(map_sql)?;
        tx.commit().await.map_err(map_sql)?;
        Ok(())
    }

    async fn delete_document(&self, document_id: &str) -> Result<bool, PortError> {
        let mut tx = self.pool.begin().await.map_err(map_sql)?;
        let exists: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM knowledge_document WHERE id = ?")
                .bind(document_id)
                .fetch_one(tx.deref_mut())
                .await
                .map_err(map_sql)?;
        if exists == 0 {
            return Ok(false);
        }
        let owner_type: String =
            sqlx::query_scalar("SELECT owner_type FROM knowledge_document WHERE id = ?")
                .bind(document_id)
                .fetch_one(tx.deref_mut())
                .await
                .map_err(map_sql)?;
        if owner_type == "chapter" || document_id.starts_with("chapter:") {
            return Err(PortError::Conflict(
                "系统派生的章节索引不能通过通用知识删除接口删除。".into(),
            ));
        }
        let chunks: Vec<String> =
            sqlx::query_scalar("SELECT id FROM knowledge_chunk WHERE document_id = ?")
                .bind(document_id)
                .fetch_all(tx.deref_mut())
                .await
                .map_err(map_sql)?;
        for chunk_id in &chunks {
            sqlx::query("DELETE FROM knowledge_fts WHERE chunk_id = ?")
                .bind(chunk_id)
                .execute(tx.deref_mut())
                .await
                .map_err(map_sql)?;
            sqlx::query("DELETE FROM embedding WHERE chunk_id = ?")
                .bind(chunk_id)
                .execute(tx.deref_mut())
                .await
                .map_err(map_sql)?;
        }
        sqlx::query("DELETE FROM knowledge_chunk WHERE document_id = ?")
            .bind(document_id)
            .execute(tx.deref_mut())
            .await
            .map_err(map_sql)?;
        sqlx::query("DELETE FROM knowledge_document WHERE id = ?")
            .bind(document_id)
            .execute(tx.deref_mut())
            .await
            .map_err(map_sql)?;
        tx.commit().await.map_err(map_sql)?;
        Ok(true)
    }

    async fn retry_index_document(&self, document_id: &str) -> Result<bool, PortError> {
        let row = sqlx::query(
            "SELECT novel_id, owner_type, title, body, source_revision, source_content_hash,
                    index_status
             FROM knowledge_document WHERE id = ?",
        )
        .bind(document_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sql)?;
        let Some(row) = row else {
            return Ok(false);
        };
        let status: String = row.try_get("index_status").map_err(map_sql)?;
        if status == "ready" {
            return Ok(false);
        }
        let source_revision: Option<String> = row.try_get("source_revision").map_err(map_sql)?;
        let terms = fts_query(&novel_context::tokenize(
            &row.try_get::<String, _>("body").map_err(map_sql)?,
            &novel_context::Lexicon::default(),
        ));
        self.index_document(NewDocument {
            id: document_id.to_string(),
            novel_id: row.try_get("novel_id").map_err(map_sql)?,
            owner_type: row.try_get("owner_type").map_err(map_sql)?,
            title: row.try_get("title").map_err(map_sql)?,
            body: row.try_get("body").map_err(map_sql)?,
            terms,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_millis() as i64)
                .unwrap_or(0),
            source_revision,
            source_content_hash: row.try_get("source_content_hash").map_err(map_sql)?,
            embedding: None,
        })
        .await?;
        Ok(true)
    }

    async fn get_document_body(
        &self,
        document_id: &str,
    ) -> Result<Option<novel_application::KnowledgeDocumentBody>, PortError> {
        let row = sqlx::query(
            "SELECT id, novel_id, owner_type, title, body, source_revision, index_status, index_version
             FROM knowledge_document WHERE id = ?",
        ).bind(document_id).fetch_optional(&self.pool).await.map_err(map_sql)?;
        row.as_ref()
            .map(|row| {
                Ok(novel_application::KnowledgeDocumentBody {
                    id: row.try_get("id").map_err(map_sql)?,
                    novel_id: row.try_get("novel_id").map_err(map_sql)?,
                    owner_type: row.try_get("owner_type").map_err(map_sql)?,
                    title: row.try_get("title").map_err(map_sql)?,
                    body: row.try_get("body").map_err(map_sql)?,
                    source_revision: row.try_get("source_revision").map_err(map_sql)?,
                    index_status: row.try_get("index_status").map_err(map_sql)?,
                    index_version: row.try_get("index_version").map_err(map_sql)?,
                })
            })
            .transpose()
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
            "SELECT knowledge_fts.chunk_id, d.owner_type, c.body, c.source_revision, c.chunk_hash, d.index_version
             FROM knowledge_fts
             JOIN knowledge_chunk c ON c.id = knowledge_fts.chunk_id
             JOIN knowledge_document d ON d.id = c.document_id
             WHERE d.novel_id = ? AND d.index_status = 'ready' AND knowledge_fts MATCH ?
             ORDER BY bm25(knowledge_fts), c.id
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
            "SELECT c.id AS chunk_id, d.owner_type, c.body, c.source_revision, c.chunk_hash,
                    d.index_version, e.vector_blob
             FROM embedding e
             JOIN knowledge_chunk c ON c.id = e.chunk_id
             JOIN knowledge_document d ON d.id = c.document_id
             WHERE d.novel_id = ? AND d.index_status = 'ready' AND e.model = ? AND e.dimensions = ? AND e.normalized = ?",
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
                .then_with(|| left.1.chunk_id.cmp(&right.1.chunk_id))
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
        source_revision: row.try_get("source_revision").map_err(map_sql)?,
        chunk_hash: row.try_get("chunk_hash").map_err(map_sql)?,
        index_version: row.try_get("index_version").map_err(map_sql)?,
    })
}

fn revision_rank(revision: &str) -> Option<i64> {
    revision
        .rsplit_once("@v")
        .and_then(|(_, value)| value.parse::<i64>().ok())
        .or_else(|| {
            revision
                .strip_prefix('v')
                .and_then(|value| value.parse::<i64>().ok())
        })
}
