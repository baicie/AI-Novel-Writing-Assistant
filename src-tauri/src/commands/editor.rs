use novel_application::ChapterDraftView;
use novel_application::{AppError, ChapterSnapshot};
use serde::Serialize;
use tauri::State;

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
