use novel_application::ChapterDraftView;
use novel_application::{AppError, ChapterSnapshot};
use novel_llm::LlmRequest;
use novel_platform::SecretStore;
use serde::{Deserialize, Serialize};
use tauri::State;
use tokio_util::sync::CancellationToken;

use crate::state::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterMetaView {
    pub chapter_id: String,
    pub order: i32,
    pub title: String,
    pub word_count: u64,
    pub open_issue_count: u32,
    pub style_summary: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MacroContextView {
    pub chapter_role_in_volume: String,
    pub volume_title: String,
    pub volume_position_label: String,
    pub volume_phase_label: String,
    pub pace_directive: String,
    pub chapter_mission: String,
    pub previous_chapter_bridge: String,
    pub next_chapter_bridge: String,
    pub active_plot_threads: Vec<String>,
    pub character_state_summary: String,
    pub world_constraint_summary: String,
    pub must_keep_constraints: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterEditorWorkspaceView {
    pub chapter_meta: ChapterMetaView,
    pub macro_context: MacroContextView,
    pub diagnostic_cards: Vec<serde_json::Value>,
    pub recommended_task: Option<serde_json::Value>,
    pub refresh_reason: String,
}

#[tauri::command]
pub async fn get_chapter_editor_workspace(
    state: State<'_, AppState>,
    novel_id: String,
    chapter_id: String,
) -> Result<ChapterEditorWorkspaceView, AppError> {
    let chapter: ChapterSnapshot = state
        .runtime
        .director
        .get_chapter(&novel_id, &chapter_id)
        .await?;
    tracing::info!(novel_id = %novel_id, chapter_id = %chapter_id, "chapter editor workspace served to webview");
    Ok(workspace_from_chapter(chapter))
}

fn workspace_from_chapter(chapter: ChapterSnapshot) -> ChapterEditorWorkspaceView {
    ChapterEditorWorkspaceView {
        chapter_meta: ChapterMetaView {
            chapter_id: chapter.id,
            order: chapter.sort_order,
            title: chapter.title,
            word_count: chapter
                .content
                .chars()
                .filter(|character| !character.is_whitespace())
                .count() as u64,
            open_issue_count: 0,
            style_summary: None,
            updated_at: chapter.updated_at.to_string(),
        },
        macro_context: MacroContextView {
            chapter_role_in_volume: String::new(),
            volume_title: String::new(),
            volume_position_label: String::new(),
            volume_phase_label: String::new(),
            pace_directive: String::new(),
            chapter_mission: String::new(),
            previous_chapter_bridge: String::new(),
            next_chapter_bridge: String::new(),
            active_plot_threads: Vec::new(),
            character_state_summary: String::new(),
            world_constraint_summary: String::new(),
            must_keep_constraints: Vec::new(),
        },
        diagnostic_cards: Vec::new(),
        recommended_task: None,
        refresh_reason: "local-kernel".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_projection_keeps_chapter_text_metadata_without_exposing_body() {
        let workspace = workspace_from_chapter(ChapterSnapshot {
            id: "c1".into(),
            novel_id: "n1".into(),
            title: "第一章".into(),
            sort_order: 3,
            volume_id: None,
            protection: "user_edited".into(),
            official_version: 7,
            content: "林动入山\n风雪扑面".into(),
            updated_at: 42,
        });
        assert_eq!(workspace.chapter_meta.chapter_id, "c1");
        assert_eq!(workspace.chapter_meta.order, 3);
        assert_eq!(workspace.chapter_meta.word_count, 8);
        assert_eq!(workspace.chapter_meta.updated_at, "42");
        assert!(workspace.diagnostic_cards.is_empty());
        assert!(workspace.recommended_task.is_none());
    }
}

/// 选区改写请求：坐标与原文片段都由调用方给出，内核负责校验它们仍然对得上。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterSelectionRewriteRequest {
    pub request_id: String,
    pub novel_id: String,
    pub chapter_id: String,
    /// 选区在「当前编辑器正文」里的 UTF-16 偏移。
    pub selection_from: u32,
    pub selection_to: u32,
    /// 选区原文。必须与 content_snapshot 对应区间逐字一致。
    pub selected_text: String,
    /// 发起改写时编辑器里的完整正文，用来确认坐标没有漂移。
    pub content_snapshot: String,
    pub operation_label: String,
    pub scope_label: String,
    pub instruction: Option<String>,
    pub before_context: String,
    pub after_context: String,
    pub chapter_title: String,
    pub model: Option<String>,
}

/// 内核只返回模型给出的候选文本；候选 id 与 diff 由前端用同一套共享实现生成，
/// 避免两套 diff 产生无法互相校验的分片。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ChapterSelectionRewriteCandidate {
    pub label: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
    #[serde(default)]
    pub risk_notes: Vec<String>,
    #[serde(default)]
    pub semantic_tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ChapterSelectionRewriteView {
    pub candidates: Vec<ChapterSelectionRewriteCandidate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub macro_alignment_note: Option<String>,
}

/// 模型输出结构。
///
/// 这里**不**用 `deny_unknown_fields`：模型偶尔会多带一两个说明字段，
/// 为此让作者的整次改写失败不值得。真正的底线放在必填项上 ——
/// `content` 没有 `default`，缺正文的候选在反序列化阶段就会被拒。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RewriteModelOutput {
    candidates: Vec<RewriteModelCandidate>,
    #[serde(default)]
    macro_alignment_note: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RewriteModelCandidate {
    #[serde(default)]
    label: Option<String>,
    content: String,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    rationale: Option<String>,
    #[serde(default)]
    risk_notes: Vec<String>,
    #[serde(default)]
    semantic_tags: Vec<String>,
}

/// UTF-16 偏移取子串：编辑器坐标是 UTF-16，Rust 字符串是 UTF-8，
/// 中文正文下两者不可混用，否则选区会错位。
fn slice_utf16(text: &str, from: u32, to: u32) -> Option<String> {
    let units: Vec<u16> = text.encode_utf16().collect();
    let from = from as usize;
    let to = to as usize;
    if from > to || to > units.len() {
        return None;
    }
    String::from_utf16(&units[from..to]).ok()
}

/// 把模型输出收敛成可用候选与对齐说明：去掉空白候选与完全重复的文本，保留顺序。
fn normalize_output(output: RewriteModelOutput) -> ChapterSelectionRewriteView {
    let macro_alignment_note = output
        .macro_alignment_note
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string);
    ChapterSelectionRewriteView {
        candidates: normalize_candidates(output),
        macro_alignment_note,
    }
}

/// 把模型输出收敛成可用候选：去掉空白候选与完全重复的文本，保留顺序。
fn normalize_candidates(output: RewriteModelOutput) -> Vec<ChapterSelectionRewriteCandidate> {
    let mut seen: Vec<String> = Vec::new();
    let mut candidates = Vec::new();
    for (index, candidate) in output.candidates.into_iter().enumerate() {
        let content = candidate.content.trim().to_string();
        if content.is_empty() || seen.contains(&content) {
            continue;
        }
        seen.push(content.clone());
        let clean = |value: Option<String>| {
            value
                .map(|text| text.trim().to_string())
                .filter(|text| !text.is_empty())
        };
        candidates.push(ChapterSelectionRewriteCandidate {
            label: clean(candidate.label).unwrap_or_else(|| format!("方案 {}", index + 1)),
            content,
            summary: clean(candidate.summary),
            rationale: clean(candidate.rationale),
            risk_notes: candidate
                .risk_notes
                .into_iter()
                .map(|note| note.trim().to_string())
                .filter(|note| !note.is_empty())
                .take(3)
                .collect(),
            semantic_tags: candidate
                .semantic_tags
                .into_iter()
                .map(|tag| tag.trim().to_string())
                .filter(|tag| !tag.is_empty())
                .take(4)
                .collect(),
        });
        if candidates.len() == 3 {
            break;
        }
    }
    candidates
}

/// 章节选区改写：桌面端的局部改写走这里，不经过整章导演流水线。
///
/// 和 `open_run` 的区别是它不建 Run、不写候选表、不动正文：只做一次受控模型调用，
/// 把候选文本交回编辑器，由作者在 Diff 里决定要不要应用。
#[tauri::command]
pub async fn rewrite_chapter_selection(
    state: State<'_, AppState>,
    request: ChapterSelectionRewriteRequest,
) -> Result<ChapterSelectionRewriteView, AppError> {
    let request_id = request.request_id.trim().to_string();
    if request_id.is_empty() {
        return Err(AppError::validation("这次改写缺少请求编号。"));
    }
    if request.selected_text.trim().is_empty() {
        return Err(AppError::validation("请先选中要改写的正文。"));
    }

    // 选区必须仍然指向作者看到的那段文字：坐标漂移时拒绝改写，
    // 否则候选会被应用到错误的位置。
    let actual = slice_utf16(
        &request.content_snapshot,
        request.selection_from,
        request.selection_to,
    )
    .ok_or_else(|| AppError::validation("选区超出正文范围，请重新选中后再试。"))?;
    if actual != request.selected_text {
        return Err(AppError::conflict(
            "正文片段已变化，请重新选中内容后再生成改写。",
        ));
    }

    // 章节必须属于这本书：否则就能借改写读到别的书的正文。
    let chapter = state
        .runtime
        .director
        .get_chapter(&request.novel_id, &request.chapter_id)
        .await?;

    let asset = novel_prompt::resolve("chapter_selection_rewrite", "chapter-selection-rewrite-v1")
        .map_err(|_| AppError::integrity("缺少选区改写的提示词版本。"))?;
    let mut vars = std::collections::BTreeMap::new();
    vars.insert(
        "operation_label".to_string(),
        request.operation_label.trim().to_string(),
    );
    vars.insert(
        "scope_label".to_string(),
        request.scope_label.trim().to_string(),
    );
    vars.insert(
        "instruction".to_string(),
        request
            .instruction
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("无")
            .to_string(),
    );
    vars.insert("selected_text".to_string(), request.selected_text.clone());
    vars.insert("before_context".to_string(), request.before_context.clone());
    vars.insert("after_context".to_string(), request.after_context.clone());
    vars.insert(
        "chapter_title".to_string(),
        if request.chapter_title.trim().is_empty() {
            chapter.title.clone()
        } else {
            request.chapter_title.trim().to_string()
        },
    );
    let prompt = novel_prompt::render(asset.template, &vars)
        .map_err(|_| AppError::integrity("选区改写的提示词渲染失败。"))?;

    // 走和其它生成一致的受控路由：任务类型 writer，拿不到绑定时回退到历史模型设置。
    let route = crate::commands::provider::service(&state)
        .resolve_route("writer")
        .await
        .map_err(crate::commands::provider::to_app_error)?;
    let legacy = state
        .kernel
        .load_model_setup()
        .await
        .map_err(AppError::from)?
        .map(|setup| novel_application::LegacyModelTarget {
            endpoint: setup.endpoint,
            model: setup.model,
            allow_localhost_http: setup.allow_localhost_http,
            secret_ref: setup.secret_ref,
        });
    let target =
        novel_application::resolve_generation_target(route, legacy, request.model.as_deref())
            .map_err(crate::commands::provider::to_app_error)?;
    let api_key = state
        .secrets
        .get(&target.credential_ref)
        .await
        .map_err(|_| {
            AppError::from(novel_application::PortError::Storage(
                "系统凭据暂时不可用".into(),
            ))
        })?
        .unwrap_or_default();
    if api_key.trim().is_empty() {
        return Err(AppError::validation("还没有填写模型密钥。"));
    }

    let (sender, mut receiver) = tokio::sync::mpsc::channel::<String>(64);
    // 选区改写不流式回传，但 provider 需要一个接收端；这里直接丢弃增量。
    let drain = tokio::spawn(async move { while receiver.recv().await.is_some() {} });
    let response = state
        .llm
        .generate_with_protocol(
            LlmRequest {
                request_id: request_id.clone(),
                model: target.model_id.clone(),
                prompt,
                endpoint: target.endpoint.clone(),
                api_key,
                allow_localhost_http: target.allow_localhost_http,
                max_output_tokens: None,
            },
            &target.protocol,
            target.reasoning_effort.as_deref(),
            sender,
            CancellationToken::new(),
        )
        .await;
    let _ = drain.await;
    let response = response.map_err(|error| AppError::validation(error.to_string()))?;

    let output: RewriteModelOutput = novel_llm::parse_structured_json(&response.text)
        .map_err(|error| AppError::validation(error.to_string()))?;
    let view = normalize_output(output);
    if view.candidates.is_empty() {
        return Err(AppError::validation("模型没有给出可用的改写候选，请重试。"));
    }
    tracing::info!(
        novel_id = %request.novel_id,
        chapter_id = %request.chapter_id,
        candidates = view.candidates.len(),
        "chapter selection rewrite served from local kernel"
    );
    Ok(view)
}

#[cfg(test)]
mod rewrite_tests {
    use super::*;

    #[test]
    fn utf16_slice_matches_editor_offsets_for_chinese_text() {
        // 编辑器给的是 UTF-16 偏移：中文下按字节切会错位。
        let text = "林动入山，风雪扑面";
        assert_eq!(slice_utf16(text, 0, 4).unwrap(), "林动入山");
        assert_eq!(slice_utf16(text, 5, 9).unwrap(), "风雪扑面");
        assert!(slice_utf16(text, 5, 99).is_none(), "越界必须拒绝");
        assert!(slice_utf16(text, 6, 2).is_none(), "反向区间必须拒绝");
    }

    #[test]
    fn normalize_drops_blank_and_duplicate_candidates_and_caps_at_three() {
        let output = RewriteModelOutput {
            macro_alignment_note: Some("  ".into()),
            candidates: vec![
                RewriteModelCandidate {
                    label: Some("  ".into()),
                    content: "  改写一  ".into(),
                    summary: Some(" 摘要 ".into()),
                    rationale: None,
                    risk_notes: vec!["  ".into(), "风险一".into()],
                    semantic_tags: vec!["增强情绪".into()],
                },
                // 与上一条去掉空白后完全相同：重复候选对作者没有意义。
                RewriteModelCandidate {
                    label: Some("方案 B".into()),
                    content: "改写一".into(),
                    summary: None,
                    rationale: None,
                    risk_notes: vec![],
                    semantic_tags: vec![],
                },
                RewriteModelCandidate {
                    label: Some("方案 B".into()),
                    content: "改写二".into(),
                    summary: None,
                    rationale: None,
                    risk_notes: vec![],
                    semantic_tags: vec![],
                },
                RewriteModelCandidate {
                    label: None,
                    content: "   ".into(),
                    summary: None,
                    rationale: None,
                    risk_notes: vec![],
                    semantic_tags: vec![],
                },
                RewriteModelCandidate {
                    label: None,
                    content: "改写三".into(),
                    summary: None,
                    rationale: None,
                    risk_notes: vec![],
                    semantic_tags: vec![],
                },
                RewriteModelCandidate {
                    label: None,
                    content: "改写四".into(),
                    summary: None,
                    rationale: None,
                    risk_notes: vec![],
                    semantic_tags: vec![],
                },
            ],
        };
        let view = normalize_output(output);
        let candidates = &view.candidates;
        assert_eq!(
            candidates
                .iter()
                .map(|candidate| candidate.content.as_str())
                .collect::<Vec<_>>(),
            vec!["改写一", "改写二", "改写三"],
            "重复与空白候选必须被丢掉，且最多保留三个"
        );
        assert_eq!(candidates[0].label, "方案 1", "空 label 要有稳定回退");
        assert_eq!(candidates[0].summary.as_deref(), Some("摘要"));
        assert_eq!(candidates[0].risk_notes, vec!["风险一"], "空风险项要去掉");
        assert!(
            view.macro_alignment_note.is_none(),
            "纯空白的对齐说明不能当成有内容返回"
        );
    }

    #[test]
    fn macro_alignment_note_is_trimmed_and_passed_through() {
        let view = normalize_output(RewriteModelOutput {
            macro_alignment_note: Some("  本章要把矛盾推到卷中  ".into()),
            candidates: vec![RewriteModelCandidate {
                label: None,
                content: "改写".into(),
                summary: None,
                rationale: None,
                risk_notes: vec![],
                semantic_tags: vec![],
            }],
        });
        assert_eq!(
            view.macro_alignment_note.as_deref(),
            Some("本章要把矛盾推到卷中")
        );
    }

    #[test]
    fn model_output_without_candidates_is_rejected_instead_of_returning_empty() {
        let view = normalize_output(RewriteModelOutput {
            macro_alignment_note: None,
            candidates: vec![],
        });
        assert!(view.candidates.is_empty(), "没有候选时不能编造候选");
    }

    /// 解析边界：模型多塞字段可以容忍（只丢我们不读的部分），
    /// 但缺少 content 这种「没有正文可用」的输出必须失败，不能当成空候选继续。
    #[test]
    fn parsing_tolerates_extra_fields_but_requires_candidate_content() {
        let parsed: RewriteModelOutput = novel_llm::parse_structured_json(
            "{\"candidates\":[{\"content\":\"改写\"}],\"surprise\":1}",
        )
        .expect("多余字段不应让整次改写失败");
        assert_eq!(parsed.candidates.len(), 1);
        assert_eq!(parsed.candidates[0].content, "改写");

        assert!(
            novel_llm::parse_structured_json::<RewriteModelOutput>(
                "{\"candidates\":[{\"label\":\"方案\"}]}"
            )
            .is_err(),
            "候选缺少正文时必须报错，而不是当成空候选"
        );
        assert!(
            novel_llm::parse_structured_json::<RewriteModelOutput>("{\"macroAlignmentNote\":\"x\"}")
                .is_err(),
            "没有 candidates 字段必须报错"
        );
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveChapterDraftRequest {
    pub novel_id: String,
    pub chapter_id: String,
    pub client_id: String,
    pub content: String,
    pub base_official_version: i64,
    pub base_updated_at: String,
    pub expected_revision: i64,
}

#[tauri::command]
pub async fn get_chapter_draft(
    state: tauri::State<'_, crate::state::AppState>,
    novel_id: String,
    chapter_id: String,
    client_id: String,
) -> Result<Option<ChapterDraftView>, novel_application::AppError> {
    state
        .runtime
        .director
        .get_chapter_draft(&novel_id, &chapter_id, &client_id)
        .await
}

#[tauri::command]
pub async fn list_chapter_drafts(
    state: tauri::State<'_, crate::state::AppState>,
    novel_id: String,
    chapter_id: String,
) -> Result<Vec<ChapterDraftView>, novel_application::AppError> {
    state
        .runtime
        .director
        .list_chapter_drafts(&novel_id, &chapter_id)
        .await
}

#[tauri::command]
pub async fn save_chapter_draft(
    state: tauri::State<'_, crate::state::AppState>,
    request: SaveChapterDraftRequest,
) -> Result<ChapterDraftView, novel_application::AppError> {
    state
        .runtime
        .director
        .save_chapter_draft(
            &request.novel_id,
            &request.chapter_id,
            &request.client_id,
            &request.content,
            request.base_official_version,
            &request.base_updated_at,
            request.expected_revision,
        )
        .await
}

#[tauri::command]
pub async fn clear_chapter_draft(
    state: tauri::State<'_, crate::state::AppState>,
    novel_id: String,
    chapter_id: String,
    client_id: String,
    submitted_content: String,
    local_revision: i64,
) -> Result<bool, novel_application::AppError> {
    state
        .runtime
        .director
        .clear_chapter_draft(
            &novel_id,
            &chapter_id,
            &client_id,
            &submitted_content,
            local_revision,
        )
        .await
}
