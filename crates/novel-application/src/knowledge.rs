use novel_context::{ChunkRef, Lexicon, assemble, fts_query, tokenize};

use crate::error::AppError;
use crate::ports::{KnowledgeRepository, RetrievalTrace};

#[allow(clippy::too_many_arguments)]
pub async fn assemble_context(
    store: &dyn KnowledgeRepository,
    novel_id: &str,
    query: &str,
    lexicon: &Lexicon,
    owner_types: &[String],
    explicit_ids: &[String],
    vector: Option<(&str, &[f32], bool)>,
    allow_fallback: bool,
    max_chars: usize,
    trace_id: &str,
    now_ms: i64,
) -> Result<String, AppError> {
    let terms = tokenize(query, lexicon);
    let keyword_hits = if terms.is_empty() {
        Vec::new()
    } else {
        store
            .search_keyword(novel_id, &fts_query(&terms), 8)
            .await?
    };
    let vector_hits = if let Some((model, values, normalized)) = vector {
        store
            .search_vector(novel_id, model, values, normalized, 8)
            .await?
    } else {
        Vec::new()
    };
    let ranked = novel_context::rrf(
        &[
            keyword_hits
                .iter()
                .map(|hit| hit.chunk_id.clone())
                .collect(),
            vector_hits.iter().map(|hit| hit.chunk_id.clone()).collect(),
        ],
        60.0,
    );
    let mut chunks = Vec::new();
    for hit in keyword_hits.iter().chain(vector_hits.iter()) {
        if chunks
            .iter()
            .any(|chunk: &ChunkRef| chunk.id == hit.chunk_id)
        {
            continue;
        }
        chunks.push(ChunkRef {
            id: hit.chunk_id.clone(),
            owner_type: hit.owner_type.clone(),
            body: hit.body.clone(),
        });
    }
    let text = assemble(
        explicit_ids,
        owner_types,
        &ranked.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>(),
        &chunks,
        allow_fallback,
        max_chars,
    );
    let summary: String = novel_llm::redact_secrets(query).chars().take(80).collect();
    store
        .record_trace(RetrievalTrace {
            id: trace_id.to_string(),
            novel_id: novel_id.to_string(),
            query_summary: summary,
            chunk_ids: ranked.into_iter().map(|(id, _)| id).collect(),
            created_at: now_ms,
        })
        .await?;
    Ok(text)
}
