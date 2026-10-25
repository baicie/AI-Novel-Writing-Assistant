use novel_application::{
    AppError, ChapterRevisionSnapshot, ChapterSnapshot, CharacterSnapshot, NovelDetail,
    NovelRepository, NovelSummary, UpdateNovelFields, VolumeSnapshot,
};
use novel_domain::{CreationExperience, NarrativeForm, ProjectMode, WritingMode};
use serde::Deserialize;
use tauri::State;

use crate::state::AppState;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateNovelRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub target_audience: Option<Option<String>>,
    pub book_selling_point: Option<Option<String>>,
    pub competing_feel: Option<Option<String>>,
    pub first_30_chapter_promise: Option<Option<String>>,
    pub commercial_tags: Option<Option<Vec<String>>>,
    pub genre_id: Option<Option<String>>,
    pub primary_story_mode_id: Option<Option<String>>,
    pub secondary_story_mode_id: Option<Option<String>>,
    pub world_id: Option<Option<String>>,
    pub narrative_form: Option<String>,
    pub writing_mode: Option<String>,
    pub project_mode: Option<String>,
    pub narrative_pov: Option<Option<String>>,
    pub pace_preference: Option<Option<String>>,
    pub style_tone: Option<Option<String>>,
    pub emotion_intensity: Option<Option<String>>,
    pub ai_freedom: Option<Option<String>>,
    pub default_chapter_length: Option<Option<i32>>,
    pub estimated_chapter_count: Option<Option<i32>>,
    pub project_status: Option<String>,
    pub storyline_status: Option<String>,
    pub outline_status: Option<String>,
    pub resource_ready_score: Option<i32>,
    pub status: Option<String>,
    pub post_generation_style_review_enabled: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateNovelSetupRequest {
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub target_audience: Option<String>,
    pub book_selling_point: Option<String>,
    pub competing_feel: Option<String>,
    pub first_30_chapter_promise: Option<String>,
    pub commercial_tags: Option<Vec<String>>,
    pub genre_id: Option<String>,
    pub primary_story_mode_id: Option<String>,
    pub secondary_story_mode_id: Option<String>,
    pub world_id: Option<String>,
    pub writing_mode: Option<String>,
    pub project_mode: Option<String>,
    pub creation_experience: Option<String>,
    pub narrative_form: Option<String>,
    pub style_tone: Option<String>,
    pub default_chapter_length: Option<i32>,
    pub estimated_chapter_count: Option<i32>,
    pub post_generation_style_review_enabled: Option<bool>,
}

#[tauri::command]
pub async fn create_novel_with_setup(
    state: State<'_, AppState>,
    request: CreateNovelSetupRequest,
) -> Result<NovelDetail, AppError> {
    let narrative_form = request
        .narrative_form
        .as_deref()
        .map(|value| {
            NarrativeForm::parse(value).ok_or_else(|| AppError::validation("小说形态无法识别。"))
        })
        .transpose()?
        .unwrap_or(NarrativeForm::LongNovel);
    let writing_mode = request
        .writing_mode
        .as_deref()
        .map(|value| {
            WritingMode::parse(value).ok_or_else(|| AppError::validation("写作模式无法识别。"))
        })
        .transpose()?
        .unwrap_or(WritingMode::Original);
    let project_mode = request
        .project_mode
        .as_deref()
        .map(|value| {
            ProjectMode::parse(value).ok_or_else(|| AppError::validation("项目模式无法识别。"))
        })
        .transpose()?
        .unwrap_or(ProjectMode::AiLed);
    let creation_experience = request
        .creation_experience
        .as_deref()
        .map(|value| {
            CreationExperience::parse(value)
                .ok_or_else(|| AppError::validation("创作体验无法识别。"))
        })
        .transpose()?
        .unwrap_or(CreationExperience::Simple);
    state
        .runtime
        .director
        .create_novel_with_setup(
            &request.title,
            &request.description,
            narrative_form,
            writing_mode,
            project_mode,
            creation_experience,
            request.target_audience.as_deref().unwrap_or(""),
            request.book_selling_point.as_deref().unwrap_or(""),
            request.competing_feel.as_deref().unwrap_or(""),
            request.first_30_chapter_promise.as_deref().unwrap_or(""),
            request.commercial_tags.unwrap_or_default(),
            request.genre_id,
            request.primary_story_mode_id,
            request.secondary_story_mode_id,
            request.world_id,
            request.style_tone.as_deref().unwrap_or(""),
            request.default_chapter_length,
            request.estimated_chapter_count,
            request.post_generation_style_review_enabled.unwrap_or(true),
        )
        .await
}

#[tauri::command]
pub async fn list_novels(state: State<'_, AppState>) -> Result<Vec<NovelSummary>, AppError> {
    let novels = state.runtime.director.list_novels().await?;
    tracing::info!(count = novels.len(), "novel list served to webview");
    Ok(novels)
}

#[tauri::command]
pub async fn get_novel(
    state: State<'_, AppState>,
    novel_id: String,
) -> Result<NovelDetail, AppError> {
    let novel = state.runtime.director.get_novel(&novel_id).await?;
    tracing::info!(novel_id = %novel_id, "novel detail served to webview");
    Ok(novel)
}

#[tauri::command]
pub async fn create_novel(
    state: State<'_, AppState>,
    title: String,
    description: String,
) -> Result<NovelSummary, AppError> {
    state
        .runtime
        .director
        .create_novel(&title, &description)
        .await
}

#[tauri::command]
pub async fn update_novel(
    state: State<'_, AppState>,
    novel_id: String,
    request: UpdateNovelRequest,
) -> Result<NovelDetail, AppError> {
    state
        .runtime
        .director
        .update_novel(
            &novel_id,
            &UpdateNovelFields {
                title: request.title,
                description: request.description,
                target_audience: request.target_audience,
                book_selling_point: request.book_selling_point,
                competing_feel: request.competing_feel,
                first_30_chapter_promise: request.first_30_chapter_promise,
                commercial_tags: request.commercial_tags,
                genre_id: request.genre_id,
                primary_story_mode_id: request.primary_story_mode_id,
                secondary_story_mode_id: request.secondary_story_mode_id,
                world_id: request.world_id,
                narrative_form: request.narrative_form,
                writing_mode: request.writing_mode,
                project_mode: request.project_mode,
                narrative_pov: request.narrative_pov,
                pace_preference: request.pace_preference,
                style_tone: request.style_tone,
                emotion_intensity: request.emotion_intensity,
                ai_freedom: request.ai_freedom,
                default_chapter_length: request.default_chapter_length,
                estimated_chapter_count: request.estimated_chapter_count,
                project_status: request.project_status,
                storyline_status: request.storyline_status,
                outline_status: request.outline_status,
                resource_ready_score: request.resource_ready_score,
                status: request.status,
                post_generation_style_review_enabled: request.post_generation_style_review_enabled,
            },
        )
        .await
}

#[tauri::command]
pub async fn delete_novel(state: State<'_, AppState>, novel_id: String) -> Result<(), AppError> {
    state.runtime.director.delete_novel(&novel_id).await
}

#[tauri::command]
pub async fn list_chapters(
    state: State<'_, AppState>,
    novel_id: String,
) -> Result<Vec<ChapterSnapshot>, AppError> {
    state.runtime.director.list_chapters(&novel_id).await
}

#[tauri::command]
pub async fn list_characters(
    state: State<'_, AppState>,
    novel_id: String,
) -> Result<Vec<CharacterSnapshot>, AppError> {
    let characters = state.runtime.director.list_characters(&novel_id).await?;
    tracing::info!(novel_id = %novel_id, count = characters.len(), "characters served to webview");
    Ok(characters)
}

#[tauri::command]
pub async fn create_chapter(
    state: State<'_, AppState>,
    novel_id: String,
    title: String,
    sort_order: i32,
    volume_id: Option<String>,
) -> Result<ChapterSnapshot, AppError> {
    state
        .runtime
        .director
        .create_chapter_in_volume(&novel_id, &title, sort_order, volume_id)
        .await
}

#[tauri::command]
pub async fn get_chapter(
    state: State<'_, AppState>,
    novel_id: String,
    chapter_id: String,
) -> Result<ChapterSnapshot, AppError> {
    let chapter = state
        .runtime
        .director
        .get_chapter(&novel_id, &chapter_id)
        .await?;
    tracing::info!(novel_id = %novel_id, chapter_id = %chapter_id, "chapter served to webview");
    Ok(chapter)
}

#[tauri::command]
pub async fn save_chapter(
    state: State<'_, AppState>,
    novel_id: String,
    chapter_id: String,
    expected_version: i64,
    title: String,
    content: String,
) -> Result<ChapterSnapshot, AppError> {
    state
        .runtime
        .director
        .save_chapter(&novel_id, &chapter_id, expected_version, &title, &content)
        .await
}

#[tauri::command]
pub async fn delete_chapter(
    state: State<'_, AppState>,
    novel_id: String,
    chapter_id: String,
    expected_version: i64,
) -> Result<(), AppError> {
    state
        .runtime
        .director
        .delete_chapter(&novel_id, &chapter_id, expected_version)
        .await
}

#[tauri::command]
pub async fn export_novel(
    state: State<'_, AppState>,
    novel_id: String,
    format: String,
) -> Result<String, AppError> {
    let novel = state.runtime.director.get_novel(&novel_id).await?;
    let chapters = state.runtime.director.list_chapters(&novel_id).await?;
    let export = novel_platform::ExportNovel {
        title: novel.title,
        description: novel.description,
        chapters: chapters
            .into_iter()
            .map(|chapter| novel_platform::ExportChapter {
                title: chapter.title,
                content: chapter.content,
            })
            .collect(),
    };
    match format.as_str() {
        "json" => novel_platform::novel_json(&export)
            .map_err(|_| AppError::integrity("导出内容无法序列化。")),
        "markdown" | "md" => Ok(novel_platform::chapter_markdown(&export)),
        "txt" => {
            let mut output = format!("{}\n\n{}\n", export.title, export.description);
            for chapter in export.chapters {
                output.push_str(&format!("\n{}\n\n{}\n", chapter.title, chapter.content));
            }
            Ok(output)
        }
        _ => Err(AppError::validation(
            "导出格式只支持 markdown、json 或 txt。",
        )),
    }
}

#[tauri::command]
pub async fn update_chapter_meta(
    state: State<'_, AppState>,
    novel_id: String,
    chapter_id: String,
    expected_version: i64,
    title: String,
    sort_order: i32,
    volume_id: Option<String>,
) -> Result<ChapterSnapshot, AppError> {
    state
        .runtime
        .director
        .update_chapter_meta(
            &novel_id,
            &chapter_id,
            expected_version,
            &title,
            sort_order,
            volume_id,
        )
        .await
}

#[tauri::command]
pub async fn list_chapter_revisions(
    state: State<'_, AppState>,
    novel_id: String,
    chapter_id: String,
) -> Result<Vec<ChapterRevisionSnapshot>, AppError> {
    state
        .runtime
        .director
        .list_chapter_revisions(&novel_id, &chapter_id)
        .await
}

#[tauri::command]
pub async fn restore_chapter_revision(
    state: State<'_, AppState>,
    novel_id: String,
    chapter_id: String,
    revision_id: String,
    expected_version: i64,
) -> Result<ChapterSnapshot, AppError> {
    state
        .runtime
        .director
        .restore_chapter_revision(&novel_id, &chapter_id, &revision_id, expected_version)
        .await
}

#[tauri::command]
pub async fn list_volumes(
    state: State<'_, AppState>,
    novel_id: String,
) -> Result<Vec<VolumeSnapshot>, AppError> {
    state.runtime.director.list_volumes(&novel_id).await
}

#[tauri::command]
pub async fn create_volume(
    state: State<'_, AppState>,
    novel_id: String,
    title: String,
    sort_order: i32,
    summary: String,
) -> Result<VolumeSnapshot, AppError> {
    state
        .runtime
        .director
        .create_volume(&novel_id, &title, sort_order, &summary)
        .await
}

#[tauri::command]
pub async fn update_volume(
    state: State<'_, AppState>,
    novel_id: String,
    volume_id: String,
    title: String,
    sort_order: i32,
    summary: String,
) -> Result<VolumeSnapshot, AppError> {
    state
        .runtime
        .director
        .update_volume(&novel_id, &volume_id, &title, sort_order, &summary)
        .await
}

#[tauri::command]
pub async fn delete_volume(
    state: State<'_, AppState>,
    novel_id: String,
    volume_id: String,
) -> Result<(), AppError> {
    state
        .runtime
        .director
        .delete_volume(&novel_id, &volume_id)
        .await
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCharacterRequest {
    pub id: String,
    pub novel_id: String,
    pub name: String,
    pub role: String,
}

#[tauri::command]
pub async fn create_character(
    state: State<'_, AppState>,
    request: CreateCharacterRequest,
) -> Result<novel_application::CharacterSnapshot, AppError> {
    if request.name.trim().is_empty() {
        return Err(AppError::validation("人物名字不能为空。"));
    }
    state
        .kernel
        .create_character(novel_application::NewCharacter {
            id: request.id,
            novel_id: request.novel_id,
            name: request.name.trim().into(),
            role: request.role,
            now_ms: novel_platform::now_ms(),
        })
        .await
        .map_err(AppError::from)
}
