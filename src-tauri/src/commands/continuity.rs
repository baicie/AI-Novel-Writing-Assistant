use novel_application::{
    AppError, ChapterContinuityProjection, ContextPreview, ContextTrace,
    DEFAULT_CONTEXT_TOKEN_BUDGET, DEFAULT_OUTPUT_RESERVE, WorkflowRepository,
    preview_director_context,
};
use tauri::State;

use crate::state::AppState;

/// 作者查看第 N 章时，只读取截至该章已确认的事实；未来的事实不会被当成现在。
#[tauri::command]
pub async fn get_chapter_continuity(
    state: State<'_, AppState>,
    novel_id: String,
    chapter_id: String,
) -> Result<ChapterContinuityProjection, AppError> {
    state
        .chapter_continuity
        .project(&novel_id, &chapter_id)
        .await
}

/// 下一次创作会引用哪些上下文：只返回可审计的 trace，不返回 Prompt 正文，也不调用模型。
#[tauri::command]
pub async fn preview_chapter_context(
    state: State<'_, AppState>,
    novel_id: String,
    chapter_id: String,
    input_budget: Option<u64>,
    output_reserve: Option<u64>,
) -> Result<ContextPreview, AppError> {
    let artifacts = state
        .kernel
        .list_artifacts(&novel_id)
        .await
        .map_err(AppError::from)?;
    preview_director_context(
        state.kernel.as_ref(),
        &artifacts,
        &novel_id,
        &chapter_id,
        input_budget.unwrap_or(DEFAULT_CONTEXT_TOKEN_BUDGET),
        output_reserve.unwrap_or(DEFAULT_OUTPUT_RESERVE),
    )
    .await
}

/// 当前 Run 冻结的上下文 trace：历史运行只能读它自己保存的版本。
#[tauri::command]
pub async fn get_run_context_trace(
    state: State<'_, AppState>,
    novel_id: String,
    run_id: String,
) -> Result<Option<ContextTrace>, AppError> {
    let detail = state
        .runtime
        .director
        .get_run_detail(&novel_id, &run_id)
        .await?;
    let Some(artifact) = detail
        .artifacts
        .iter()
        .find(|artifact| artifact.artifact_type == "context")
    else {
        return Ok(None);
    };
    let payload: serde_json::Value = serde_json::from_str(&artifact.payload_json)
        .map_err(|_| AppError::integrity("运行上下文记录格式损坏。"))?;
    let trace = payload
        .get("trace")
        .cloned()
        .ok_or_else(|| AppError::integrity("运行上下文记录缺少 trace。"))?;
    serde_json::from_value(trace)
        .map(Some)
        .map_err(|_| AppError::integrity("运行上下文 trace 无法解析。"))
}
