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
        return Err(with_sent_state(
            AppError::validation("这次改写缺少请求编号。"),
            RewriteSentState::NeverSent,
        ));
    }
    if request.selected_text.trim().is_empty() {
        return Err(with_sent_state(
            AppError::validation("请先选中要改写的正文。"),
            RewriteSentState::NeverSent,
        ));
    }

    // 选区必须仍然指向作者看到的那段文字：坐标漂移时拒绝改写，
    // 否则候选会被应用到错误的位置。
    let actual = slice_utf16(
        &request.content_snapshot,
        request.selection_from,
        request.selection_to,
    )
    .ok_or_else(|| {
        with_sent_state(
            AppError::validation("选区超出正文范围，请重新选中后再试。"),
            RewriteSentState::NeverSent,
        )
    })?;
    if actual != request.selected_text {
        return Err(with_sent_state(
            AppError::conflict("正文片段已变化，请重新选中内容后再生成改写。"),
            RewriteSentState::NeverSent,
        ));
    }

    // 章节必须属于这本书：否则就能借改写读到别的书的正文。
    let chapter = state
        .runtime
        .director
        .get_chapter(&request.novel_id, &request.chapter_id)
        .await
        .map_err(|error| with_sent_state(error, RewriteSentState::NeverSent))?;

    let asset = novel_prompt::resolve("chapter_selection_rewrite", "chapter-selection-rewrite-v1")
        .map_err(|_| {
            with_sent_state(
                AppError::integrity("缺少选区改写的提示词版本。"),
                RewriteSentState::NeverSent,
            )
        })?;
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
    let prompt = novel_prompt::render(asset.template, &vars).map_err(|_| {
        with_sent_state(
            AppError::integrity("选区改写的提示词渲染失败。"),
            RewriteSentState::NeverSent,
        )
    })?;

    // 走和其它生成一致的受控路由：任务类型 writer，拿不到绑定时回退到历史模型设置。
    let route = crate::commands::provider::service(&state)
        .resolve_route("writer")
        .await
        .map_err(crate::commands::provider::to_app_error)
        .map_err(|error| with_sent_state(error, RewriteSentState::NeverSent))?;
    let legacy = state
        .kernel
        .load_model_setup()
        .await
        .map_err(AppError::from)
        .map_err(|error| with_sent_state(error, RewriteSentState::NeverSent))?
        .map(|setup| novel_application::LegacyModelTarget {
            endpoint: setup.endpoint,
            model: setup.model,
            allow_localhost_http: setup.allow_localhost_http,
            secret_ref: setup.secret_ref,
        });
    let target =
        novel_application::resolve_generation_target(route, legacy, request.model.as_deref())
            .map_err(crate::commands::provider::to_app_error)
            .map_err(|error| with_sent_state(error, RewriteSentState::NeverSent))?;
    let api_key = read_credential(state.secrets.as_ref(), &target.credential_ref)
        .await
        .map_err(|error| with_sent_state(error, RewriteSentState::NeverSent))?;

    // 先登记再发送：进程中途退出时记录会停在 sending，
    // 下次同一条请求编号回来就能看出「可能已经计费」，而不是当成从未发生。
    let started_at = novel_platform::now_ms();
    state
        .kernel
        .begin_selection_rewrite(
            &request_id,
            &request.novel_id,
            &request.chapter_id,
            started_at,
        )
        .await
        .map_err(|error| {
            with_sent_state(
                AppError::conflict(match error {
                    novel_application::PortError::Conflict(message) => message,
                    other => other.to_string(),
                }),
                // 这次请求在登记阶段就被拒，没有发出；上一次的未知记录另有提示。
                RewriteSentState::NeverSent,
            )
        })?;

    let view = match execute_rewrite(
        state.llm.as_ref(),
        &target,
        api_key,
        prompt,
        request_id.clone(),
    )
    .await
    {
        Ok(view) => {
            let _ = state
                .kernel
                .mark_selection_rewrite_succeeded(&request_id, novel_platform::now_ms())
                .await;
            view
        }
        Err(failure) => {
            let now = novel_platform::now_ms();
            // 发送状态来自执行段本身，不看错误文案：
            // 只有确定没发出去才允许免确认重试，其余都按「可能已计费」记录。
            let recorded = match failure.sent_state {
                RewriteSentState::NeverSent => {
                    state
                        .kernel
                        .mark_selection_rewrite_not_sent(&request_id, &failure.error.message, now)
                        .await
                }
                RewriteSentState::ResultUnknown => {
                    state
                        .kernel
                        .mark_selection_rewrite_unknown(&request_id, &failure.error.message, now)
                        .await
                }
                RewriteSentState::SentButUnusable => {
                    state
                        .kernel
                        .mark_selection_rewrite_failed(&request_id, &failure.error.message, now)
                        .await
                }
            };
            if recorded.is_err() {
                tracing::warn!(
                    request_id = %request_id,
                    "改写尝试的结果没能写入本地记录，下次重试会继续按未知计费处理"
                );
            }
            return Err(failure.error);
        }
    };
    tracing::info!(
        novel_id = %request.novel_id,
        chapter_id = %request.chapter_id,
        candidates = view.candidates.len(),
        "chapter selection rewrite served from local kernel"
    );
    Ok(view)
}

/// 执行段的发送状态：决定这次失败能不能免确认重试。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RewriteSentState {
    /// 请求没有发出：校验、凭据或路由在发送前就拒绝了。
    NeverSent,
    /// 已发出且模型返回，但结果不可用：可能已计费，结果本身不是未知。
    SentButUnusable,
    /// 可能已到达模型服务，但没有可确认的结果。
    ResultUnknown,
}

impl RewriteSentState {
    /// 跨进程给界面读的发送状态：不依赖任何中文文案。
    fn as_str(self) -> &'static str {
        match self {
            Self::NeverSent => "NOT_SENT",
            Self::SentButUnusable => "SENT_BUT_UNUSABLE",
            Self::ResultUnknown => "RESULT_UNKNOWN",
        }
    }
}

/// 把发送状态写进错误的机器可读字段；界面优先读它，而不是猜错误码或文案。
fn with_sent_state(mut error: AppError, state: RewriteSentState) -> AppError {
    error.details = Some(serde_json::json!({ "sentState": state.as_str() }));
    error
}

#[derive(Debug)]
struct RewriteFailure {
    error: AppError,
    sent_state: RewriteSentState,
}

impl RewriteFailure {
    fn never_sent(error: AppError) -> Self {
        Self {
            sent_state: RewriteSentState::NeverSent,
            error: with_sent_state(error, RewriteSentState::NeverSent),
        }
    }

    /// 请求已经发出、且拿到了模型返回，但结果不可用：这次调用已经产生费用。
    fn sent_but_unusable(message: impl Into<String>) -> Self {
        Self {
            sent_state: RewriteSentState::SentButUnusable,
            error: with_sent_state(
                AppError::model_result_unusable(message),
                RewriteSentState::SentButUnusable,
            ),
        }
    }

    fn from_llm_error(error: novel_llm::LlmError) -> Self {
        let sent_state = match &error {
            // 超时与「已计费但连接中断」都无法确认模型服务是否真的算出结果。
            novel_llm::LlmError::Timeout => RewriteSentState::ResultUnknown,
            novel_llm::LlmError::Unavailable { billed: true, .. } => {
                RewriteSentState::ResultUnknown
            }
            // 本地预算在发送前就拒绝：确定没有发出。
            novel_llm::LlmError::BudgetExceeded(_) => RewriteSentState::NeverSent,
            // 其余发生在请求已到达服务端之后（鉴权、限流、端点拒绝、响应解析）。
            // 拿不到确认的计费结果时按「可能已计费」处理，不能允许无提示重试。
            _ => RewriteSentState::SentButUnusable,
        };
        let app_error = match sent_state {
            RewriteSentState::ResultUnknown => AppError::model_result_unknown(format!(
                "{error}。这次改写可能已经发送到模型服务，结果无法确认；请先检查后再决定是否重新生成。"
            )),
            RewriteSentState::SentButUnusable => AppError::model_result_unusable(format!(
                "{error}。这次改写已经发送到模型服务，可能已经产生费用；重新生成会再次调用模型。"
            )),
            RewriteSentState::NeverSent => AppError::validation(error.to_string()),
        };
        Self {
            error: with_sent_state(app_error, sent_state),
            sent_state,
        }
    }
}

/// 凭据读取：密钥库不可用时按存储错误处理，不把它伪装成「没填密钥」。
async fn read_credential(
    secrets: &dyn SecretStore,
    credential_ref: &str,
) -> Result<String, AppError> {
    secrets
        .get(credential_ref)
        .await
        .map_err(|_| {
            AppError::from(novel_application::PortError::Storage(
                "系统凭据暂时不可用".into(),
            ))
        })
        .map(|value| value.unwrap_or_default())
}

/// 从读取凭据到解析模型输出的执行段：命令入口只做参数校验与上下文拼装。
///
/// 无密钥时的放行判定必须与生成链路共用同一套 P16.6 规则：
/// 只有本机 Ollama 可以无密钥调用，远程来源缺密钥在发出请求之前就被挡住。
/// 单独成函数是为了能用真实 mock 服务与探针 Provider 验证「零请求」。
async fn execute_rewrite(
    llm: &dyn novel_llm::LlmProvider,
    target: &novel_application::GenerationTarget,
    api_key: String,
    prompt: String,
    request_id: String,
) -> Result<ChapterSelectionRewriteView, RewriteFailure> {
    novel_application::ensure_credential(&target.protocol, &target.endpoint, &api_key)
        .map_err(|error| RewriteFailure::never_sent(AppError::validation(error.message.clone())))?;

    let (sender, mut receiver) = tokio::sync::mpsc::channel::<String>(64);
    // 选区改写不流式回传，但 provider 需要一个接收端；这里直接丢弃增量。
    let drain = tokio::spawn(async move { while receiver.recv().await.is_some() {} });
    let response = llm
        .generate_with_protocol(
            LlmRequest {
                request_id,
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
    let response = response.map_err(RewriteFailure::from_llm_error)?;

    // 到这里模型已经返回正文：后续解析失败属于明确失败，不能再说「结果未知」，
    // 但这次调用已经产生费用，提示里要说清楚重新生成会再次调用模型。
    let output: RewriteModelOutput =
        novel_llm::parse_structured_json(&response.text).map_err(|_| {
            RewriteFailure::sent_but_unusable(
                "模型返回的改写结果无法解析。这次调用已经产生费用；重新生成会再次调用模型。",
            )
        })?;
    let view = normalize_output(output);
    if view.candidates.is_empty() {
        return Err(RewriteFailure::sent_but_unusable(
            "模型没有给出可用的改写候选。这次调用已经产生费用；重新生成会再次调用模型。",
        ));
    }
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
            novel_llm::parse_structured_json::<RewriteModelOutput>(
                "{\"macroAlignmentNote\":\"x\"}"
            )
            .is_err(),
            "没有 candidates 字段必须报错"
        );
    }

    // ---- P17.2 P1.1：选区改写的无密钥策略与「零请求」保证 ----

    fn ollama_target(endpoint: String) -> novel_application::GenerationTarget {
        novel_application::GenerationTarget {
            provider_id: "local-ollama".into(),
            model_id: "qwen-local".into(),
            protocol: "ollama".into(),
            endpoint,
            allow_localhost_http: true,
            credential_ref: "local-ollama#1".into(),
            source: "binding".into(),
            reasoning_effort: None,
        }
    }

    /// 记录调用次数与最后一次请求的探针 Provider：用来证明被拒绝的路径没有发出任何请求。
    #[derive(Default)]
    struct ProbeProvider {
        calls: std::sync::atomic::AtomicUsize,
    }

    impl ProbeProvider {
        fn calls(&self) -> usize {
            self.calls.load(std::sync::atomic::Ordering::SeqCst)
        }
    }

    #[async_trait::async_trait]
    impl novel_llm::LlmProvider for ProbeProvider {
        fn capabilities(&self) -> novel_llm::ProviderCapabilities {
            novel_llm::ProviderCapabilities {
                streaming: false,
                structured_output: false,
                tool_call: false,
                reasoning: false,
                context_limit: 0,
            }
        }

        async fn generate(
            &self,
            _request: LlmRequest,
            _tokens: tokio::sync::mpsc::Sender<String>,
            _cancel: CancellationToken,
        ) -> Result<novel_llm::LlmResponse, novel_llm::LlmError> {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(novel_llm::LlmResponse {
                text: "{\"candidates\":[{\"content\":\"探针改\"}]}".into(),
                usage: novel_llm::TokenUsage::unknown(),
            })
        }
    }

    /// 启动一个只应答一次的真实 HTTP 服务：返回端口与收到的请求路径。
    async fn mock_http_server(body: String) -> (u16, tokio::task::JoinHandle<Option<String>>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("绑定本机 mock 服务");
        let port = listener.local_addr().unwrap().port();
        let handle = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.ok()?;
            let mut buffer = vec![0u8; 16 * 1024];
            let read = socket.read(&mut buffer).await.ok()?;
            let request = String::from_utf8_lossy(&buffer[..read]).to_string();
            let path = request
                .lines()
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
                .map(str::to_string);
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = socket.write_all(response.as_bytes()).await;
            path
        });
        (port, handle)
    }

    #[tokio::test]
    async fn keyless_loopback_ollama_reaches_the_real_ollama_endpoint() {
        // 本机 Ollama 无密钥必须真的发出去，并且走 /api/chat —— 不是被空密钥提前挡回。
        let (port, server) = mock_http_server(
            "{\"message\":{\"content\":\"{\\\"candidates\\\":[{\\\"label\\\":\\\"方案 A\\\",\\\"content\\\":\\\"林动踏入山门。\\\"}]}\"}}"
                .to_string(),
        )
        .await;
        let provider = novel_llm::ProtocolRouterProvider::new();
        let target = ollama_target(format!("http://127.0.0.1:{port}"));
        let view = execute_rewrite(
            &provider,
            &target,
            String::new(),
            "改写请求".into(),
            "req-keyless".into(),
        )
        .await
        .expect("本机 Ollama 无密钥应当完成改写");
        assert_eq!(view.candidates[0].content, "林动踏入山门。");
        let path = server.await.unwrap();
        assert_eq!(
            path.as_deref(),
            Some("/api/chat"),
            "无密钥的本机 Ollama 必须真正到达 /api/chat"
        );
    }

    #[tokio::test]
    async fn keyless_loopback_ollama_openai_mode_reaches_v1_chat_completions() {
        // baseURL 写成 /v1 时，无密钥同样要真正发出，并落在 Ollama 的兼容路径上。
        let (port, server) = mock_http_server(
            "{\"message\":{\"content\":\"{\\\"candidates\\\":[{\\\"content\\\":\\\"兼容模式改写\\\"}]}\"}}"
                .to_string(),
        )
        .await;
        let provider = novel_llm::ProtocolRouterProvider::new();
        let target = ollama_target(format!("http://127.0.0.1:{port}/v1"));
        let view = execute_rewrite(
            &provider,
            &target,
            String::new(),
            "改写请求".into(),
            "req-keyless-v1".into(),
        )
        .await
        .expect("本机 Ollama 兼容模式无密钥应当完成改写");
        assert_eq!(view.candidates[0].content, "兼容模式改写");
        let path = server.await.unwrap();
        assert_eq!(path.as_deref(), Some("/v1/chat/completions"));
    }

    #[tokio::test]
    async fn remote_ollama_without_key_is_rejected_before_any_request() {
        let probe = ProbeProvider::default();
        // 远程地址缺密钥：必须在发出请求之前拒绝，且给出可执行的修复提示。
        let target = novel_application::GenerationTarget {
            allow_localhost_http: false,
            ..ollama_target("https://ollama.example.com".into())
        };
        let error = execute_rewrite(
            &probe,
            &target,
            String::new(),
            "改写请求".into(),
            "req-remote".into(),
        )
        .await
        .expect_err("远程 Ollama 缺密钥必须被拒绝");
        assert_eq!(probe.calls(), 0, "被拒绝的请求不能发到 Provider");
        assert_eq!(
            error.error.code,
            novel_application::AppErrorCode::Validation
        );
        assert!(
            error.error.message.contains("密钥"),
            "提示要指向补密钥：{}",
            error.error.message
        );
    }

    #[tokio::test]
    async fn openai_compatible_without_key_is_rejected_before_any_request() {
        let probe = ProbeProvider::default();
        let target = novel_application::GenerationTarget {
            protocol: "openai-compatible".into(),
            endpoint: "http://127.0.0.1:11434/v1".into(),
            ..ollama_target("http://127.0.0.1:11434/v1".into())
        };
        let error = execute_rewrite(
            &probe,
            &target,
            "   ".into(),
            "改写请求".into(),
            "req-openai".into(),
        )
        .await
        .expect_err("OpenAI 兼容协议缺密钥必须被拒绝，即使地址在本机");
        assert_eq!(probe.calls(), 0, "被拒绝的请求不能发到 Provider");
        assert!(
            error.error.message.contains("密钥"),
            "{}",
            error.error.message
        );
    }

    #[tokio::test]
    async fn anthropic_protocol_without_key_is_rejected_before_any_request() {
        let probe = ProbeProvider::default();
        let target = novel_application::GenerationTarget {
            protocol: "anthropic-messages".into(),
            endpoint: "https://api.anthropic.com".into(),
            allow_localhost_http: false,
            ..ollama_target("https://api.anthropic.com".into())
        };
        let error = execute_rewrite(
            &probe,
            &target,
            String::new(),
            "改写请求".into(),
            "req-anthropic".into(),
        )
        .await
        .expect_err("只读协议缺密钥必须被拒绝");
        assert_eq!(probe.calls(), 0, "被拒绝的请求不能发到 Provider");
        assert!(
            error.error.message.contains("密钥"),
            "{}",
            error.error.message
        );
    }

    /// 有密钥时策略不能误伤：同一个远程 Ollama 带上密钥就应当放行到 Provider。
    #[tokio::test]
    async fn remote_ollama_with_key_passes_the_credential_gate() {
        let probe = ProbeProvider::default();
        let target = novel_application::GenerationTarget {
            allow_localhost_http: false,
            ..ollama_target("https://ollama.example.com".into())
        };
        // 用探针 Provider 证明「策略层放行」；真实网络行为由 Provider 自己的测试覆盖。
        let view = execute_rewrite(
            &probe,
            &target,
            "sk-remote".into(),
            "改写请求".into(),
            "req-remote-key".into(),
        )
        .await
        .expect("有密钥的远程来源应当通过密钥门");
        assert_eq!(probe.calls(), 1);
        assert_eq!(view.candidates[0].content, "探针改");
    }

    // ---- P17.2 P1.3：计费状态未知的边界 ----

    /// 按指定错误失败的探针 Provider：用来验证不同失败会被归到不同的发送状态。
    struct FailingProvider {
        error: novel_llm::LlmError,
    }

    #[async_trait::async_trait]
    impl novel_llm::LlmProvider for FailingProvider {
        fn capabilities(&self) -> novel_llm::ProviderCapabilities {
            novel_llm::ProviderCapabilities {
                streaming: false,
                structured_output: false,
                tool_call: false,
                reasoning: false,
                context_limit: 0,
            }
        }

        async fn generate(
            &self,
            _request: LlmRequest,
            _tokens: tokio::sync::mpsc::Sender<String>,
            _cancel: CancellationToken,
        ) -> Result<novel_llm::LlmResponse, novel_llm::LlmError> {
            Err(self.error.clone())
        }
    }

    /// 读错误的机器可读发送状态：与前端使用的是同一字段。
    fn sent_state_of(error: &AppError) -> Option<&str> {
        error.details.as_ref()?.get("sentState")?.as_str()
    }

    async fn rewrite_failure_for(error: novel_llm::LlmError) -> RewriteFailure {
        let provider = FailingProvider { error };
        let target = novel_application::GenerationTarget {
            allow_localhost_http: false,
            ..ollama_target("https://ollama.example.com".into())
        };
        execute_rewrite(
            &provider,
            &target,
            "sk-remote".into(),
            "改写请求".into(),
            "req-fault".into(),
        )
        .await
        .expect_err("故障注入必须失败")
    }

    /// 超时可能已经到达模型服务：必须报「结果未确认」，不能被当成免费的失败。
    #[tokio::test]
    async fn timeout_is_reported_as_unknown_billing_not_a_plain_failure() {
        let failure = rewrite_failure_for(novel_llm::LlmError::Timeout).await;
        assert_eq!(failure.sent_state, RewriteSentState::ResultUnknown);
        assert_eq!(
            failure.error.code,
            novel_application::AppErrorCode::ModelResultUnknown
        );
        assert!(!failure.error.retryable, "未知计费不能标记为可自动重试");
        assert!(
            failure.error.message.contains("无法确认"),
            "提示要说明结果未知：{}",
            failure.error.message
        );
        assert!(
            failure.error.message.contains("可能已经发送"),
            "提示要说明可能已经发出：{}",
            failure.error.message
        );
    }

    /// 连接中断但服务端可能已经处理：同样属于结果未知。
    #[tokio::test]
    async fn billed_connection_drop_is_reported_as_unknown_billing() {
        let failure = rewrite_failure_for(novel_llm::LlmError::Unavailable {
            billed: true,
            reason: "模型服务连接中断，无法确认是否已计费".into(),
        })
        .await;
        assert_eq!(failure.sent_state, RewriteSentState::ResultUnknown);
        assert_eq!(
            failure.error.code,
            novel_application::AppErrorCode::ModelResultUnknown
        );
    }

    /// 明确失败（鉴权、限流、端点拒绝）不是未知计费，界面可以给出确定的失败说明。
    #[tokio::test]
    async fn explicit_failures_are_not_reported_as_unknown_billing() {
        for error in [
            novel_llm::LlmError::Unauthorized,
            novel_llm::LlmError::RateLimit,
            novel_llm::LlmError::EndpointRejected("端点被拒绝".into()),
            novel_llm::LlmError::Unavailable {
                billed: false,
                reason: "模型服务暂时不可用".into(),
            },
        ] {
            let label = error.to_string();
            let failure = rewrite_failure_for(error).await;
            assert_eq!(
                failure.sent_state,
                RewriteSentState::SentButUnusable,
                "{label} 发生在请求到达服务端之后，不能算从未发送"
            );
            assert_ne!(
                failure.error.code,
                novel_application::AppErrorCode::ModelResultUnknown,
                "{label} 不能被算成未知计费"
            );
        }
    }

    /// 密钥门在发送之前拒绝：确定没发出，可以免确认重试。
    #[tokio::test]
    async fn credential_gate_is_recorded_as_never_sent() {
        let target = novel_application::GenerationTarget {
            allow_localhost_http: false,
            ..ollama_target("https://ollama.example.com".into())
        };
        let probe = ProbeProvider::default();
        let failure = execute_rewrite(
            &probe,
            &target,
            String::new(),
            "改写请求".into(),
            "req-gate".into(),
        )
        .await
        .expect_err("远程缺密钥必须被拒绝");
        assert_eq!(failure.sent_state, RewriteSentState::NeverSent);
        assert_eq!(probe.calls(), 0, "被拒绝的请求不能发出");
    }

    /// 模型返回了正文但格式不对：请求已经发出去，不能声称「没扣费」。
    #[tokio::test]
    async fn unparsable_model_output_is_not_reported_as_free() {
        struct GarbledProvider;

        #[async_trait::async_trait]
        impl novel_llm::LlmProvider for GarbledProvider {
            fn capabilities(&self) -> novel_llm::ProviderCapabilities {
                novel_llm::ProviderCapabilities {
                    streaming: false,
                    structured_output: false,
                    tool_call: false,
                    reasoning: false,
                    context_limit: 0,
                }
            }

            async fn generate(
                &self,
                _request: LlmRequest,
                _tokens: tokio::sync::mpsc::Sender<String>,
                _cancel: CancellationToken,
            ) -> Result<novel_llm::LlmResponse, novel_llm::LlmError> {
                Ok(novel_llm::LlmResponse {
                    text: "这不是 JSON".into(),
                    usage: novel_llm::TokenUsage::unknown(),
                })
            }
        }

        let target = novel_application::GenerationTarget {
            allow_localhost_http: false,
            ..ollama_target("https://ollama.example.com".into())
        };
        let failure = execute_rewrite(
            &GarbledProvider,
            &target,
            "sk-remote".into(),
            "改写请求".into(),
            "req-garbled".into(),
        )
        .await
        .expect_err("格式错误必须失败");
        // 模型已经返回并计费，这次请求不属于「从未发出」。
        assert_ne!(
            failure.sent_state,
            RewriteSentState::NeverSent,
            "模型已返回时不能标记为从未发出"
        );
        assert!(
            failure.error.message.contains("已经产生费用"),
            "要说明这次调用已经产生费用：{}",
            failure.error.message
        );
    }

    /// 模型已经返回但输出不可用：错误必须是机器可读的 `MODEL_RESULT_UNUSABLE`，
    /// 不能再复用发送前校验的 `VALIDATION`，否则界面会当成「没发出去、可以免费重试」。
    #[tokio::test]
    async fn unusable_model_output_reports_sent_but_unusable_contract() {
        /// 固定返回一段文本的 Provider：用来覆盖「已发送、但结果不可用」的解析边界。
        struct FixedTextProvider(&'static str);

        #[async_trait::async_trait]
        impl novel_llm::LlmProvider for FixedTextProvider {
            fn capabilities(&self) -> novel_llm::ProviderCapabilities {
                novel_llm::ProviderCapabilities {
                    streaming: false,
                    structured_output: false,
                    tool_call: false,
                    reasoning: false,
                    context_limit: 0,
                }
            }

            async fn generate(
                &self,
                _request: LlmRequest,
                _tokens: tokio::sync::mpsc::Sender<String>,
                _cancel: CancellationToken,
            ) -> Result<novel_llm::LlmResponse, novel_llm::LlmError> {
                Ok(novel_llm::LlmResponse {
                    text: self.0.to_string(),
                    usage: novel_llm::TokenUsage::unknown(),
                })
            }
        }

        let target = novel_application::GenerationTarget {
            allow_localhost_http: false,
            ..ollama_target("https://ollama.example.com".into())
        };
        for (label, text) in [
            ("非法 JSON", "这不是 JSON"),
            ("缺少 candidates", "{\"macroAlignmentNote\":\"x\"}"),
            ("空候选", "{\"candidates\":[]}"),
            ("只有空白候选", "{\"candidates\":[{\"content\":\"   \"}]}"),
        ] {
            let failure = execute_rewrite(
                &FixedTextProvider(text),
                &target,
                "sk-remote".into(),
                "改写请求".into(),
                "req-unusable".into(),
            )
            .await
            .expect_err("模型输出不可用时必须失败");
            assert_eq!(
                failure.sent_state,
                RewriteSentState::SentButUnusable,
                "{label} 发生在模型返回之后，不能算从未发送"
            );
            assert_eq!(
                failure.error.code,
                novel_application::AppErrorCode::ModelResultUnusable,
                "{label} 必须使用独立的机器可读错误码"
            );
            assert_eq!(
                sent_state_of(&failure.error),
                Some("SENT_BUT_UNUSABLE"),
                "{label} 的发送状态要能跨进程传到界面"
            );
            assert!(!failure.error.retryable, "{label} 不能被标记为可自动重试");
        }
    }

    /// 跨 Rust / 前端的错误契约：三个发送状态都有稳定的机器可读表示。
    #[test]
    fn rewrite_error_contract_carries_machine_readable_sent_state() {
        let cases = [
            (
                RewriteFailure::never_sent(AppError::validation("还没有填写模型密钥。")),
                "VALIDATION",
                "NOT_SENT",
            ),
            (
                RewriteFailure::sent_but_unusable("模型没有给出可用的改写候选。"),
                "MODEL_RESULT_UNUSABLE",
                "SENT_BUT_UNUSABLE",
            ),
            (
                RewriteFailure::from_llm_error(novel_llm::LlmError::Timeout),
                "MODEL_RESULT_UNKNOWN",
                "RESULT_UNKNOWN",
            ),
        ];
        for (failure, expected_code, expected_sent_state) in cases {
            let json = serde_json::to_value(&failure.error).expect("错误必须能序列化给前端");
            assert_eq!(json["code"], expected_code);
            assert_eq!(json["details"]["sentState"], expected_sent_state);
        }
    }

    /// 凭据库不可用与「没填密钥」是两种状态，不能互相冒充。
    #[tokio::test]
    async fn unavailable_secret_store_is_reported_as_storage_not_missing_key() {
        struct BrokenSecrets;

        #[async_trait::async_trait]
        impl SecretStore for BrokenSecrets {
            async fn put(
                &self,
                _id: &str,
                _secret: &str,
            ) -> Result<(), novel_platform::SecretError> {
                Err(novel_platform::SecretError::Unavailable)
            }
            async fn get(&self, _id: &str) -> Result<Option<String>, novel_platform::SecretError> {
                Err(novel_platform::SecretError::Unavailable)
            }
            async fn delete(&self, _id: &str) -> Result<(), novel_platform::SecretError> {
                Ok(())
            }
        }

        let error = read_credential(&BrokenSecrets, "slot")
            .await
            .expect_err("读取失败不能当成空密钥");
        assert_eq!(error.code, novel_application::AppErrorCode::StorageCorrupt);
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

/// 本章仍处于「可能已计费、结果未确认」的选区改写。
///
/// 重新生成之前，界面要先让作者知道上一请求可能已经产生费用；
/// 这里只返回状态与时间，不返回正文或提示词。
#[tauri::command]
pub async fn list_pending_selection_rewrites(
    state: tauri::State<'_, crate::state::AppState>,
    novel_id: String,
    chapter_id: String,
) -> Result<Vec<novel_application::SelectionRewriteAttemptView>, novel_application::AppError> {
    state
        .kernel
        .list_unknown_selection_rewrites(&novel_id, &chapter_id)
        .await
        .map_err(novel_application::AppError::from)
}
