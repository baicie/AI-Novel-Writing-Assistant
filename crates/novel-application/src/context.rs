//! Deterministic chapter context assembly for the native director.
//!
//! The assembly is intentionally explicit: every source is either selected with a
//! hash/token estimate or recorded as missing/stale/over-budget in the trace. No
//! database read failure is converted into an empty source.

use novel_domain::ArtifactStatus;
use novel_workflow::{ArtifactView, book_scope, chapter_scope};
use serde::{Deserialize, Serialize};

use crate::dto::ChapterSnapshot;
use crate::error::AppError;
use crate::hash::sha256_hex;
use crate::ports::NovelRepository;

pub const CONTEXT_ASSEMBLY_VERSION: &str = "director-context-v2";
pub const DEFAULT_CONTEXT_TOKEN_BUDGET: u64 = 24_000;
pub const DEFAULT_OUTPUT_RESERVE: u64 = 8_000;
/// 预览入口的硬上限：任何调用方都不能把整本书塞进一次上下文。
pub const MAX_CONTEXT_TOKEN_BUDGET: u64 = 64_000;

pub fn default_context_token_budget() -> u64 {
    DEFAULT_CONTEXT_TOKEN_BUDGET
}
const RECENT_CHAPTER_LIMIT: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextSourceTrace {
    pub source_type: String,
    pub source_id: String,
    pub source_version: Option<i64>,
    pub source_status: String,
    pub selected: bool,
    pub exclusion_reason: Option<String>,
    pub estimated_tokens: u64,
    pub content_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextTrace {
    pub assembly_version: String,
    pub target_chapter_id: String,
    /// 本次组装使用的 chapter_list 产物版本。
    pub chapter_plan_version: Option<i64>,
    /// 目标章节的义务 ID；旧计划为空。
    pub obligation_ids: Vec<String>,
    pub input_budget: u64,
    pub output_reserve: u64,
    pub estimated_tokens: u64,
    pub content_hash: String,
    pub sources: Vec<ContextSourceTrace>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextAssembly {
    pub prompt_text: String,
    pub trace: ContextTrace,
    /// 目标章节使用的 chapter_list 产物版本；旧计划没有章节义务时仍会填写。
    pub chapter_plan_version: Option<i64>,
    /// 目标章节的结构化义务 ID，供 Writer/Reviewer/Repair 追踪。
    pub obligation_ids: Vec<String>,
    /// 已渲染的硬约束文本；为空表示旧计划没有结构化义务。
    pub obligations_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextPreview {
    pub trace: ContextTrace,
    /// 必需来源缺失的原因；非空时对应生成能力必须被阻断。
    pub blocked_reasons: Vec<String>,
}

struct SourceDraft {
    source_type: &'static str,
    source_id: String,
    source_version: Option<i64>,
    source_status: String,
    text: String,
    mandatory: bool,
    priority: u8,
}

fn estimate_tokens(text: &str) -> u64 {
    // Conservative approximation, not a tokenizer. CJK characters are counted as
    // one token each; other characters are grouped in threes. This clearly
    // over-estimates English and is documented as an estimate, never as exact usage.
    let (cjk, other) = text.chars().fold((0u64, 0u64), |(cjk, other), ch| {
        if ch.is_ascii() {
            (cjk, other + 1)
        } else {
            (cjk + 1, other)
        }
    });
    cjk + other.div_ceil(3)
}

fn source_trace(draft: &SourceDraft, selected: bool, reason: Option<&str>) -> ContextSourceTrace {
    ContextSourceTrace {
        source_type: draft.source_type.to_string(),
        source_id: draft.source_id.clone(),
        source_version: draft.source_version,
        source_status: draft.source_status.clone(),
        selected,
        exclusion_reason: reason.map(str::to_string),
        estimated_tokens: estimate_tokens(&draft.text),
        content_hash: selected.then(|| sha256_hex(draft.text.as_bytes())),
    }
}

fn artifact_source<'a>(
    artifacts: &'a [ArtifactView],
    artifact_type: &'static str,
    scope: &str,
) -> (Option<&'a ArtifactView>, Option<&'static str>) {
    let mut candidates: Vec<&ArtifactView> = artifacts
        .iter()
        .filter(|artifact| artifact.artifact_type == artifact_type && artifact.scope == scope)
        .collect();
    if candidates.is_empty() {
        return (None, Some("missing"));
    }
    candidates.sort_by_key(|artifact| artifact.version);
    let latest = candidates.last().expect("non-empty");
    if latest.status == ArtifactStatus::Stale {
        (None, Some("stale_artifact"))
    } else {
        (Some(latest), None)
    }
}

fn payload(value: &str, artifact_type: &str) -> Result<serde_json::Value, AppError> {
    serde_json::from_str(value)
        .map_err(|_| AppError::integrity(format!("上下文来源 {artifact_type} 的产物格式损坏。")))
}

fn chapter_plan_for_target(
    payload: &serde_json::Value,
    target: &ChapterSnapshot,
) -> Option<serde_json::Value> {
    payload
        .get("chapters")
        .and_then(serde_json::Value::as_array)?
        .iter()
        .find(|chapter| {
            chapter
                .get("chapterOrder")
                .and_then(serde_json::Value::as_i64)
                .is_some_and(|order| order == i64::from(target.sort_order))
        })
        .cloned()
}

/// Resolve the frozen target chapter plan from the latest non-stale chapter_list
/// artifact. This is used by review/repair so they never fall back to the whole
/// book plan (which would leak future chapters into the current context).
pub(crate) fn target_plan_from_artifacts(
    artifacts: &[ArtifactView],
    target_order: i32,
    pinned_version: Option<i64>,
) -> Result<serde_json::Value, AppError> {
    let mut candidates: Vec<&ArtifactView> = artifacts
        .iter()
        .filter(|artifact| {
            artifact.artifact_type == "chapter_list"
                && artifact.scope == book_scope()
                && artifact.status != ArtifactStatus::Stale
        })
        .collect();
    if candidates.is_empty() {
        return Err(AppError::integrity(
            "缺少可用的章节计划，不能生成或审校没有计划约束的正文。",
        ));
    }
    candidates.sort_by_key(|artifact| artifact.version);
    let artifact = match pinned_version {
        Some(version) => candidates
            .iter()
            .find(|artifact| artifact.version == version)
            .copied()
            .ok_or_else(|| {
                AppError::integrity(format!(
                    "本次 Run 冻结的章节计划版本 {version} 不可用，不能改用其他版本审校。"
                ))
            })?,
        None => *candidates.last().expect("non-empty"),
    };
    let plan = payload(&artifact.payload_json, "chapter_list")?;
    plan.get("chapters")
        .and_then(serde_json::Value::as_array)
        .and_then(|chapters| {
            chapters.iter().find(|chapter| {
                chapter
                    .get("chapterOrder")
                    .and_then(serde_json::Value::as_i64)
                    == Some(i64::from(target_order))
            })
        })
        .cloned()
        .ok_or_else(|| {
            AppError::integrity(format!(
                "章节计划缺少目标章节 {target_order}，不能生成或审校没有计划约束的正文。"
            ))
        })
}

pub(crate) fn target_obligations_from_artifacts(
    artifacts: &[ArtifactView],
    target_order: i32,
    pinned_version: Option<i64>,
) -> Result<Vec<crate::obligations::ChapterObligation>, AppError> {
    let plan = target_plan_from_artifacts(artifacts, target_order, pinned_version)?;
    Ok(crate::obligations::parse_obligations(&plan))
}

/// The context artifact records the chapter_list version used to write the
/// chapter. Review must use that same version, not a newer plan published while
/// the run is in flight. Prefer the artifact belonging to this run; old context
/// artifacts without a runId remain readable as a compatibility fallback.
pub(crate) fn pinned_chapter_plan_version(
    artifacts: &[ArtifactView],
    target_chapter_id: &str,
    run_id: &str,
) -> Option<i64> {
    let scope = chapter_scope(target_chapter_id);
    let contexts: Vec<&ArtifactView> = artifacts
        .iter()
        .filter(|artifact| artifact.artifact_type == "context" && artifact.scope == scope)
        .collect();
    let parsed = |artifact: &ArtifactView| {
        serde_json::from_str::<serde_json::Value>(&artifact.payload_json).ok()
    };
    let matching = contexts
        .iter()
        .filter(|artifact| {
            parsed(artifact)
                .and_then(|payload| {
                    payload
                        .get("runId")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string)
                })
                .as_deref()
                == Some(run_id)
        })
        .max_by_key(|artifact| artifact.version)
        .copied();
    let legacy = contexts
        .iter()
        .filter(|artifact| {
            parsed(artifact)
                .and_then(|payload| payload.get("runId").cloned())
                .is_none()
        })
        .max_by_key(|artifact| artifact.version)
        .copied();
    matching.or(legacy).and_then(|artifact| {
        parsed(artifact).and_then(|payload| {
            payload
                .get("chapterPlanVersion")
                .and_then(serde_json::Value::as_i64)
        })
    })
}

/// Select the context artifact for this run (chapter-scoped) so a newer run for
/// the same chapter cannot silently replace the writer/repair context. Legacy
/// artifacts without runId are accepted as a fallback.
pub(crate) fn latest_run_context_payload(
    artifacts: &[ArtifactView],
    target_chapter_id: &str,
    run_id: &str,
) -> Result<serde_json::Value, AppError> {
    let scope = chapter_scope(target_chapter_id);
    let contexts: Vec<&ArtifactView> = artifacts
        .iter()
        .filter(|artifact| artifact.artifact_type == "context" && artifact.scope == scope)
        .collect();
    let parsed = |artifact: &ArtifactView| {
        serde_json::from_str::<serde_json::Value>(&artifact.payload_json).ok()
    };
    let matching = contexts
        .iter()
        .filter(|artifact| {
            parsed(artifact)
                .and_then(|payload| {
                    payload
                        .get("runId")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string)
                })
                .as_deref()
                == Some(run_id)
        })
        .max_by_key(|artifact| artifact.version)
        .copied();
    let legacy = contexts
        .iter()
        .filter(|artifact| {
            parsed(artifact)
                .and_then(|payload| payload.get("runId").cloned())
                .is_none()
        })
        .max_by_key(|artifact| artifact.version)
        .copied();
    let artifact = matching.or(legacy).ok_or_else(|| {
        AppError::integrity("本次 Run 缺少章节上下文产物，不能继续写正文或修复。")
    })?;
    parsed(artifact).ok_or_else(|| AppError::integrity("章节上下文产物格式损坏。"))
}

/// Assemble chapter context from the frozen run artifacts and the confirmed
/// SQLite novel/chapter/character/world snapshot.
pub async fn assemble_director_context(
    novels: &dyn NovelRepository,
    artifacts: &[ArtifactView],
    novel_id: &str,
    target_chapter_id: &str,
    input_budget: u64,
    output_reserve: u64,
) -> Result<ContextAssembly, AppError> {
    Ok(assemble_context(
        novels,
        artifacts,
        novel_id,
        target_chapter_id,
        input_budget,
        output_reserve,
        true,
    )
    .await?
    .assembly)
}

/// 只读预览：不发送模型请求。必要来源缺失时不报错，而是记进 trace 并返回阻断原因。
pub async fn preview_director_context(
    novels: &dyn NovelRepository,
    artifacts: &[ArtifactView],
    novel_id: &str,
    target_chapter_id: &str,
    input_budget: u64,
    output_reserve: u64,
) -> Result<ContextPreview, AppError> {
    let budget = input_budget.clamp(512, MAX_CONTEXT_TOKEN_BUDGET);
    let outcome = assemble_context(
        novels,
        artifacts,
        novel_id,
        target_chapter_id,
        budget,
        output_reserve,
        false,
    )
    .await?;
    Ok(ContextPreview {
        trace: outcome.assembly.trace,
        blocked_reasons: outcome.blocked_reasons,
    })
}

struct AssemblyOutcome {
    assembly: ContextAssembly,
    blocked_reasons: Vec<String>,
}

async fn assemble_context(
    novels: &dyn NovelRepository,
    artifacts: &[ArtifactView],
    novel_id: &str,
    target_chapter_id: &str,
    input_budget: u64,
    output_reserve: u64,
    strict: bool,
) -> Result<AssemblyOutcome, AppError> {
    if input_budget < 512 {
        return Err(AppError::validation("上下文预算过小，无法保证必要事实。"));
    }
    let novel = novels.get_novel(novel_id).await?;
    let target = novels.get_chapter(novel_id, target_chapter_id).await?;
    let mut drafts: Vec<SourceDraft> = Vec::new();
    let mut blocked_reasons: Vec<String> = Vec::new();
    let mut chapter_plan_version: Option<i64> = None;
    let mut obligation_ids: Vec<String> = Vec::new();
    let mut obligations_text = String::new();

    let seed = serde_json::json!({
        "title": novel.title,
        "description": novel.description,
        "targetAudience": novel.target_audience,
        "sellingPoint": novel.book_selling_point,
        "competingFeel": novel.competing_feel,
        "first30ChapterPromise": novel.first_30_chapter_promise,
        "writingMode": novel.writing_mode,
        "styleTone": novel.style_tone,
    });
    drafts.push(SourceDraft {
        source_type: "novel_seed",
        source_id: novel.id.clone(),
        source_version: Some(novel.updated_at),
        source_status: "confirmed".into(),
        text: serde_json::to_string(&seed)
            .map_err(|_| AppError::integrity("小说种子无法序列化"))?,
        mandatory: true,
        priority: 0,
    });

    for (artifact_type, mandatory, priority) in [
        ("story_macro", true, 1u8),
        ("character_cast", true, 2),
        ("volume_strategy", true, 3),
        ("chapter_list", true, 4),
    ] {
        let (artifact, missing) = artifact_source(artifacts, artifact_type, book_scope());
        let Some(artifact) = artifact else {
            if mandatory {
                let reason = missing.unwrap_or("missing");
                if strict {
                    return Err(AppError::integrity(format!(
                        "上下文必要来源 {artifact_type} 不可用（{reason}），不能伪装成空上下文继续生成。"
                    )));
                }
                blocked_reasons.push(format!("必要来源 {artifact_type} 不可用（{reason}）"));
            }
            drafts.push(SourceDraft {
                source_type: artifact_type,
                source_id: String::new(),
                source_version: None,
                source_status: missing.unwrap_or("missing").into(),
                text: String::new(),
                mandatory: false,
                priority,
            });
            continue;
        };
        let value = payload(&artifact.payload_json, artifact_type)?;
        let text = if artifact_type == "chapter_list" {
            match chapter_plan_for_target(&value, &target) {
                Some(target_plan) => {
                    let obligations = crate::obligations::parse_obligations(&target_plan);
                    chapter_plan_version = Some(artifact.version);
                    obligation_ids = crate::obligations::obligation_ids(&obligations);
                    obligations_text = crate::obligations::render_obligations(&obligations);
                    serde_json::to_string(&target_plan)
                        .map_err(|_| AppError::integrity("章节计划无法序列化"))?
                }
                None => {
                    let reason = format!(
                        "目标章节 {} 没有章节计划（chapterOrder 不匹配或计划缺失），不能生成没有计划约束的正文。",
                        target.sort_order
                    );
                    if strict {
                        return Err(AppError::integrity(reason));
                    }
                    blocked_reasons.push(reason);
                    drafts.push(SourceDraft {
                        source_type: artifact_type,
                        source_id: format!(
                            "{artifact_type}:{}:{}",
                            artifact.scope, artifact.version
                        ),
                        source_version: Some(artifact.version),
                        source_status: "target_chapter_missing".into(),
                        text: String::new(),
                        mandatory: false,
                        priority,
                    });
                    continue;
                }
            }
        } else {
            serde_json::to_string(&value)
                .map_err(|_| AppError::integrity("上下文来源无法序列化"))?
        };
        drafts.push(SourceDraft {
            source_type: artifact_type,
            source_id: format!("{artifact_type}:{}:{}", artifact.scope, artifact.version),
            source_version: Some(artifact.version),
            source_status: artifact.status.as_str().to_string(),
            text,
            mandatory,
            priority,
        });
        if artifact_type == "chapter_list" && !obligations_text.is_empty() {
            drafts.push(SourceDraft {
                source_type: "chapter_obligations",
                source_id: format!(
                    "chapter_obligations:{}:{}",
                    artifact.scope, artifact.version
                ),
                source_version: Some(artifact.version),
                source_status: artifact.status.as_str().to_string(),
                text: obligations_text.clone(),
                mandatory: true,
                priority,
            });
        }
    }

    let characters = novels.list_characters(novel_id).await?;
    if characters.is_empty() {
        drafts.push(SourceDraft {
            source_type: "character_facts",
            source_id: novel_id.to_string(),
            source_version: None,
            source_status: "missing".into(),
            text: String::new(),
            mandatory: false,
            priority: 5,
        });
    } else {
        let text = serde_json::to_string(&characters)
            .map_err(|_| AppError::integrity("角色事实无法序列化"))?;
        drafts.push(SourceDraft {
            source_type: "character_facts",
            source_id: novel_id.to_string(),
            source_version: None,
            source_status: "confirmed".into(),
            text,
            mandatory: false,
            priority: 5,
        });
    }

    let worlds = novels.list_worlds_for_context(novel_id).await?;
    if worlds.is_empty() {
        drafts.push(SourceDraft {
            source_type: "world_facts",
            source_id: novel_id.to_string(),
            source_version: None,
            source_status: "missing".into(),
            text: String::new(),
            mandatory: false,
            priority: 6,
        });
    } else {
        let text = serde_json::to_string(&worlds)
            .map_err(|_| AppError::integrity("世界设定无法序列化"))?;
        drafts.push(SourceDraft {
            source_type: "world_facts",
            source_id: novel_id.to_string(),
            source_version: None,
            source_status: "confirmed".into(),
            text,
            mandatory: false,
            priority: 6,
        });
    }

    let story_facts = novels.list_story_facts(novel_id, target.sort_order).await?;
    if story_facts.is_empty() {
        drafts.push(SourceDraft {
            source_type: "story_facts",
            source_id: novel_id.to_string(),
            source_version: None,
            source_status: "missing".into(),
            text: String::new(),
            mandatory: false,
            priority: 6,
        });
    } else {
        let text = serde_json::to_string(&story_facts)
            .map_err(|_| AppError::integrity("剧情事实无法序列化"))?;
        drafts.push(SourceDraft {
            source_type: "story_facts",
            source_id: novel_id.to_string(),
            source_version: None,
            source_status: "confirmed".into(),
            text,
            mandatory: false,
            priority: 6,
        });
    }

    let structured = novels
        .list_structured_facts_for_context(novel_id, i64::from(target.sort_order))
        .await?;
    if !structured.is_empty() {
        let text = serde_json::to_string(&structured)
            .map_err(|_| AppError::integrity("结构化人物事实无法序列化"))?;
        drafts.push(SourceDraft {
            source_type: "structured_story_facts",
            source_id: novel_id.to_string(),
            source_version: None,
            source_status: "confirmed".into(),
            text,
            mandatory: false,
            priority: 5,
        });
    }

    let chapters = novels.list_chapters(novel_id).await?;
    let mut recent: Vec<&ChapterSnapshot> = chapters
        .iter()
        .filter(|chapter| {
            chapter.id != target.id
                && chapter.sort_order < target.sort_order
                && !chapter.content.trim().is_empty()
        })
        .collect();
    recent.sort_by_key(|chapter| chapter.sort_order);
    for chapter in recent.iter().rev().take(RECENT_CHAPTER_LIMIT).rev() {
        drafts.push(SourceDraft {
            source_type: "confirmed_chapter",
            source_id: chapter.id.clone(),
            source_version: Some(chapter.official_version),
            source_status: format!("confirmed:{}", chapter.protection),
            text: format!(
                "第{}章 {}\n{}",
                chapter.sort_order, chapter.title, chapter.content
            ),
            mandatory: false,
            priority: 7,
        });
    }
    if recent.is_empty() {
        drafts.push(SourceDraft {
            source_type: "confirmed_chapter",
            source_id: target.id.clone(),
            source_version: None,
            source_status: "missing".into(),
            text: String::new(),
            mandatory: false,
            priority: 7,
        });
    }
    // Earlier confirmed chapters are represented by their frozen chapter-plan
    // summaries. This keeps long books within budget while retaining prior arcs.
    let recent_orders: std::collections::BTreeSet<i32> =
        recent.iter().map(|chapter| chapter.sort_order).collect();
    let earlier: Vec<&ChapterSnapshot> = chapters
        .iter()
        .filter(|chapter| {
            chapter.id != target.id
                && chapter.sort_order < target.sort_order
                && !recent_orders.contains(&chapter.sort_order)
        })
        .collect();
    if !earlier.is_empty()
        && let Some(plan) = latest_book_artifact(artifacts, "chapter_list")
    {
        let summaries = chapter_arc_summaries(&plan.payload_json, target.sort_order);
        if !summaries.is_empty() {
            let mut text = String::new();
            for line in summaries {
                if text.chars().count() + line.chars().count() > 6_000 {
                    break;
                }
                text.push_str(&line);
                text.push('\n');
            }
            drafts.push(SourceDraft {
                source_type: "chapter_arc_summary",
                source_id: novel_id.to_string(),
                source_version: Some(plan.version),
                source_status: "confirmed".into(),
                text,
                mandatory: false,
                priority: 7,
            });
        }
    }
    for (source_type, reason) in [
        ("recent_summary", "not_migrated"),
        ("user_supplement", "not_migrated"),
    ] {
        drafts.push(SourceDraft {
            source_type,
            source_id: novel_id.to_string(),
            source_version: None,
            source_status: reason.into(),
            text: String::new(),
            mandatory: false,
            priority: 8,
        });
    }

    drafts.sort_by(|left, right| {
        left.priority
            .cmp(&right.priority)
            .then_with(|| left.source_id.cmp(&right.source_id))
    });
    let mandatory_tokens: u64 = drafts
        .iter()
        .filter(|source| source.mandatory)
        .map(|source| estimate_tokens(&source.text))
        .sum();
    if mandatory_tokens > input_budget {
        return Err(AppError::validation(format!(
            "上下文预算不足：必要事实约 {mandatory_tokens} tokens，预算 {input_budget}。"
        )));
    }

    let mut traces = Vec::new();
    let mut prompt_text = String::new();
    let mut used_tokens = 0u64;
    for draft in &drafts {
        if draft.text.trim().is_empty() {
            traces.push(source_trace(
                draft,
                false,
                Some(draft.source_status.as_str()),
            ));
            continue;
        }
        let tokens = estimate_tokens(&draft.text);
        if !draft.mandatory && used_tokens + tokens > input_budget {
            traces.push(source_trace(draft, false, Some("over_budget")));
            continue;
        }
        used_tokens += tokens;
        prompt_text.push_str(&format!("\n## {}\n", draft.source_type));
        prompt_text.push_str(&draft.text);
        prompt_text.push('\n');
        traces.push(source_trace(draft, true, None));
    }
    if prompt_text.trim().is_empty() && strict {
        return Err(AppError::integrity("上下文组装为空，已停止生成。"));
    }
    let content_hash = sha256_hex(prompt_text.as_bytes());
    Ok(AssemblyOutcome {
        assembly: ContextAssembly {
            prompt_text,
            chapter_plan_version,
            obligation_ids: obligation_ids.clone(),
            obligations_text,
            trace: ContextTrace {
                assembly_version: CONTEXT_ASSEMBLY_VERSION.to_string(),
                target_chapter_id: target.id,
                chapter_plan_version,
                obligation_ids,
                input_budget,
                output_reserve,
                estimated_tokens: used_tokens,
                content_hash,
                sources: traces,
            },
        },
        blocked_reasons,
    })
}

fn latest_book_artifact<'a>(
    artifacts: &'a [novel_domain::ArtifactView],
    artifact_type: &str,
) -> Option<&'a novel_domain::ArtifactView> {
    artifacts
        .iter()
        .filter(|artifact| {
            artifact.artifact_type == artifact_type && artifact.scope == book_scope()
        })
        .max_by_key(|artifact| artifact.version)
}

fn chapter_arc_summaries(payload_json: &str, before_order: i32) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(payload_json) else {
        return Vec::new();
    };
    let Some(chapters) = value.get("chapters").and_then(serde_json::Value::as_array) else {
        return Vec::new();
    };
    let mut rows: Vec<(i32, String, String)> = chapters
        .iter()
        .filter_map(|chapter| {
            let order = chapter.get("chapterOrder")?.as_i64()? as i32;
            if order >= before_order {
                return None;
            }
            let title = chapter.get("title")?.as_str()?.to_string();
            let summary = chapter.get("summary")?.as_str()?.to_string();
            Some((order, title, summary))
        })
        .collect();
    rows.sort_by_key(|row| row.0);
    rows.into_iter()
        .map(|(order, title, summary)| format!("第{order}章 {title}：{summary}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan_artifact(version: i64, description: &str) -> ArtifactView {
        ArtifactView {
            artifact_type: "chapter_list".into(),
            scope: book_scope().into(),
            version,
            status: ArtifactStatus::Confirmed,
            protected_user_content: false,
            payload_json: serde_json::json!({
                "chapters": [{
                    "chapterOrder": 1,
                    "title": "第一章",
                    "volumeTitle": "卷一",
                    "summary": description,
                    "characterNames": ["林动"],
                    "obligations": [{
                        "id": format!("ch1-req-{version}"),
                        "chapterOrder": 1,
                        "type": "required_event",
                        "description": description,
                        "enforcement": "hard",
                        "participants": ["林动"],
                        "source": "chapter_plan"
                    }]
                }]
            })
            .to_string(),
        }
    }

    #[test]
    fn review_plan_can_be_pinned_to_the_context_version() {
        let artifacts = vec![plan_artifact(1, "旧计划"), plan_artifact(2, "新计划")];
        let pinned = target_plan_from_artifacts(&artifacts, 1, Some(1)).unwrap();
        assert_eq!(pinned["summary"], "旧计划");
        let latest = target_plan_from_artifacts(&artifacts, 1, None).unwrap();
        assert_eq!(latest["summary"], "新计划");
    }

    #[test]
    fn missing_pinned_version_does_not_fall_back_to_a_newer_plan() {
        let artifacts = vec![plan_artifact(2, "新计划")];
        let error = target_plan_from_artifacts(&artifacts, 1, Some(1)).unwrap_err();
        assert!(error.to_string().contains("冻结的章节计划版本 1"));
    }

    fn context_artifact(run_id: Option<&str>, plan_version: i64, version: i64) -> ArtifactView {
        let mut payload = serde_json::json!({
            "assemblyVersion": CONTEXT_ASSEMBLY_VERSION,
            "targetChapterId": "ch1",
            "scope": chapter_scope("ch1"),
            "chapterPlanVersion": plan_version,
            "obligationIds": [format!("ch1-req-{plan_version}")],
            "obligationsText": format!("义务 {plan_version}"),
            "promptText": format!("上下文 {plan_version}"),
        });
        if let Some(run_id) = run_id {
            payload["runId"] = serde_json::Value::String(run_id.into());
        }
        ArtifactView {
            artifact_type: "context".into(),
            scope: chapter_scope("ch1"),
            version,
            status: ArtifactStatus::Confirmed,
            protected_user_content: false,
            payload_json: payload.to_string(),
        }
    }

    #[test]
    fn review_pins_the_context_plan_version_per_run() {
        let artifacts = vec![
            context_artifact(Some("run-a"), 1, 1),
            context_artifact(Some("run-b"), 2, 2),
        ];
        assert_eq!(
            pinned_chapter_plan_version(&artifacts, "ch1", "run-a"),
            Some(1)
        );
        assert_eq!(
            pinned_chapter_plan_version(&artifacts, "ch1", "run-b"),
            Some(2)
        );
        let context = latest_run_context_payload(&artifacts, "ch1", "run-a").unwrap();
        assert_eq!(context["promptText"], "上下文 1");
    }
}
