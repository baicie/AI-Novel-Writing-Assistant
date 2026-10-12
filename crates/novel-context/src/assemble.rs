use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkRef {
    pub id: String,
    pub owner_type: String,
    pub body: String,
}

pub fn rrf(rankings: &[Vec<String>], k: f32) -> Vec<(String, f32)> {
    let mut scores = BTreeMap::<String, f32>::new();
    for ranking in rankings {
        for (index, id) in ranking.iter().enumerate() {
            let rank = index as f32 + 1.0;
            *scores.entry(id.clone()).or_default() += 1.0 / (k + rank);
        }
    }
    let mut rows: Vec<_> = scores.into_iter().collect();
    rows.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    rows
}

pub fn decay(score: f32, distance: u32) -> f32 {
    score / (1.0 + distance as f32)
}

pub fn assemble(
    explicit_ids: &[String],
    owner_types: &[String],
    ranked_ids: &[String],
    chunks: &[ChunkRef],
    allow_unfiltered_fallback: bool,
    max_chars: usize,
) -> String {
    let lookup = |id: &str| chunks.iter().find(|chunk| chunk.id == id);
    let allowed = |chunk: &ChunkRef| {
        owner_types.is_empty() || owner_types.iter().any(|owner| owner == &chunk.owner_type)
    };
    let mut selected = Vec::new();
    for id in explicit_ids {
        if let Some(chunk) = lookup(id)
            && allowed(chunk)
        {
            selected.push(chunk);
        }
    }
    let filtered: Vec<_> = ranked_ids
        .iter()
        .filter_map(|id| lookup(id))
        .filter(|chunk| allowed(chunk))
        .collect();
    let pool = if filtered.is_empty() && allow_unfiltered_fallback {
        ranked_ids.iter().filter_map(|id| lookup(id)).collect()
    } else {
        filtered
    };
    for chunk in pool {
        if selected.iter().all(|existing| existing.id != chunk.id) {
            selected.push(chunk);
        }
    }
    let mut output = String::new();
    for chunk in selected {
        if output.chars().count() >= max_chars {
            break;
        }
        if !output.is_empty() {
            output.push('\n');
        }
        let remain = max_chars.saturating_sub(output.chars().count());
        output.extend(chunk.body.chars().take(remain));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_filter_is_hard_and_explicit_ids_come_first() {
        let chunks = vec![
            ChunkRef {
                id: "c".into(),
                owner_type: "character".into(),
                body: "角色甲".into(),
            },
            ChunkRef {
                id: "w".into(),
                owner_type: "world".into(),
                body: "世界乙".into(),
            },
        ];
        let text = assemble(
            &["c".into()],
            &["character".into()],
            &["w".into(), "c".into()],
            &chunks,
            false,
            100,
        );
        assert_eq!(text, "角色甲");
        let fallback = assemble(&[], &["missing".into()], &["w".into()], &chunks, true, 2);
        assert_eq!(fallback, "世界");
    }

    #[test]
    fn rrf_prefers_items_that_appear_in_both_lists() {
        let fused = rrf(
            &[vec!["a".into(), "b".into()], vec!["b".into(), "c".into()]],
            1.0,
        );
        assert_eq!(fused[0].0, "b");
        assert!(decay(1.0, 1) < 1.0);
    }
}
