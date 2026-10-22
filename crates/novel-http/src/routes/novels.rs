use axum::Json;
use axum::extract::{Path, Query, State};
use novel_domain::{
    CreationExperience, NarrativeForm, ProjectMode, WritingMode, ensure_can_promote,
};
use serde::Deserialize;
use serde_json::{Value, json};

use novel_application::{ListNovelsQuery, UpdateNovelFields};

use crate::error::{HttpError, HttpResult};
use crate::state::KernelState;

pub async fn list(
    State(state): State<KernelState>,
    Query(params): Query<ListParams>,
) -> HttpResult<Json<Value>> {
    let query = ListNovelsQuery {
        page: params.page,
        limit: params.limit,
        search: params.search,
        status: params.status,
        narrative_form: params.narrative_form,
        writing_mode: params.writing_mode,
    };
    let result = state
        .runtime
        .director
        .list_novels_paginated(&query)
        .await
        .map_err(HttpError::from)?;
    Ok(Json(json!({
        "success": true,
        "data": result,
    })))
}

pub async fn create(
    State(state): State<KernelState>,
    Json(payload): Json<CreateNovelBody>,
) -> HttpResult<Json<Value>> {
    // Phase 2 阶段：接受完整 setup 字段。
    let narrative_form = payload
        .narrative_form
        .as_deref()
        .and_then(novel_domain::NarrativeForm::parse)
        .unwrap_or(NarrativeForm::LongNovel);
    let writing_mode = payload
        .writing_mode
        .as_deref()
        .and_then(novel_domain::WritingMode::parse)
        .unwrap_or(WritingMode::Original);
    let project_mode = payload
        .project_mode
        .as_deref()
        .and_then(novel_domain::ProjectMode::parse)
        .unwrap_or(ProjectMode::AiLed);
    let creation_experience = payload
        .creation_experience
        .as_deref()
        .and_then(novel_domain::CreationExperience::parse)
        .unwrap_or(CreationExperience::Simple);
    let detail = state
        .runtime
        .director
        .create_novel_with_setup(
            &payload.title,
            payload.description.as_deref().unwrap_or(""),
            narrative_form,
            writing_mode,
            project_mode,
            creation_experience,
            payload.target_audience.as_deref().unwrap_or(""),
            payload.book_selling_point.as_deref().unwrap_or(""),
            payload.competing_feel.as_deref().unwrap_or(""),
            payload.first_30_chapter_promise.as_deref().unwrap_or(""),
            payload.commercial_tags.unwrap_or_default(),
            payload.genre_id,
            payload.primary_story_mode_id,
            payload.secondary_story_mode_id,
            payload.world_id,
            payload.style_tone.as_deref().unwrap_or(""),
            payload.default_chapter_length,
            payload.estimated_chapter_count,
        )
        .await
        .map_err(HttpError::from)?;
    Ok(Json(json!({
        "success": true,
        "data": detail,
        "message": "创建小说成功。",
    })))
}

pub async fn detail(
    State(state): State<KernelState>,
    Path(id): Path<String>,
) -> HttpResult<Json<Value>> {
    let detail = state
        .runtime
        .director
        .get_novel(&id)
        .await
        .map_err(HttpError::from)?;
    Ok(Json(json!({
        "success": true,
        "data": detail,
        "message": "获取小说详情成功。",
    })))
}

pub async fn update(
    State(state): State<KernelState>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateNovelBody>,
) -> HttpResult<Json<Value>> {
    let fields = UpdateNovelFields {
        title: payload.title,
        description: payload.description,
        target_audience: payload.target_audience.map(Some),
        book_selling_point: payload.book_selling_point.map(Some),
        competing_feel: payload.competing_feel.map(Some),
        first_30_chapter_promise: payload.first_30_chapter_promise.map(Some),
        commercial_tags: payload.commercial_tags.map(|tags| Some(tags)),
        genre_id: payload.genre_id.map(Some),
        primary_story_mode_id: payload.primary_story_mode_id.map(Some),
        secondary_story_mode_id: payload.secondary_story_mode_id.map(Some),
        world_id: payload.world_id.map(Some),
        narrative_form: payload
            .narrative_form
            .as_deref()
            .and_then(novel_domain::NarrativeForm::parse)
            .map(|mode| mode.as_str().to_string()),
        writing_mode: payload
            .writing_mode
            .as_deref()
            .and_then(novel_domain::WritingMode::parse)
            .map(|mode| mode.as_str().to_string()),
        project_mode: payload
            .project_mode
            .as_deref()
            .and_then(novel_domain::ProjectMode::parse)
            .map(|mode| mode.as_str().to_string()),
        narrative_pov: payload.narrative_pov.map(Some),
        pace_preference: payload.pace_preference.map(Some),
        style_tone: payload.style_tone.map(Some),
        emotion_intensity: payload.emotion_intensity.map(Some),
        ai_freedom: payload.ai_freedom.map(Some),
        default_chapter_length: payload.default_chapter_length.map(Some),
        estimated_chapter_count: payload.estimated_chapter_count.map(Some),
        project_status: payload.project_status,
        storyline_status: payload.storyline_status,
        outline_status: payload.outline_status,
        resource_ready_score: payload.resource_ready_score,
        status: payload.status,
    };
    let detail = state
        .runtime
        .director
        .update_novel(&id, &fields)
        .await
        .map_err(HttpError::from)?;
    Ok(Json(json!({
        "success": true,
        "data": detail,
        "message": "更新小说成功。",
    })))
}

pub async fn delete(
    State(state): State<KernelState>,
    Path(id): Path<String>,
) -> HttpResult<Json<Value>> {
    state
        .runtime
        .director
        .delete_novel(&id)
        .await
        .map_err(HttpError::from)?;
    Ok(Json(json!({
        "success": true,
        "data": null,
        "message": "删除小说成功。",
    })))
}

pub async fn set_creation_experience(
    State(state): State<KernelState>,
    Path((id, experience)): Path<(String, String)>,
) -> HttpResult<Json<Value>> {
    let experience = novel_domain::CreationExperience::parse(&experience).ok_or_else(|| {
        HttpError::bad_request(format!("未知的创作经验：{experience}"))
    })?;
    let detail = state
        .runtime
        .director
        .set_creation_experience(&id, experience)
        .await
        .map_err(HttpError::from)?;
    let message = match experience {
        CreationExperience::Simple => "已切换到简易模式。",
        CreationExperience::Professional => "已切换到专业模式。",
    };
    Ok(Json(json!({
        "success": true,
        "data": detail,
        "message": message,
    })))
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ListParams {
    pub page: Option<i32>,
    pub limit: Option<i32>,
    pub search: Option<String>,
    pub status: Option<String>,
    pub narrative_form: Option<String>,
    pub writing_mode: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateNovelBody {
    pub title: String,
    pub description: Option<String>,
    pub narrative_form: Option<String>,
    pub writing_mode: Option<String>,
    pub project_mode: Option<String>,
    pub creation_experience: Option<String>,
    pub target_audience: Option<String>,
    pub book_selling_point: Option<String>,
    pub competing_feel: Option<String>,
    pub first_30_chapter_promise: Option<String>,
    pub commercial_tags: Option<Vec<String>>,
    pub genre_id: Option<String>,
    pub primary_story_mode_id: Option<String>,
    pub secondary_story_mode_id: Option<String>,
    pub world_id: Option<String>,
    pub style_tone: Option<String>,
    pub default_chapter_length: Option<i32>,
    pub estimated_chapter_count: Option<i32>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_excessive_bools)]
pub struct UpdateNovelBody {
    pub title: Option<String>,
    pub description: Option<String>,
    pub target_audience: Option<String>,
    pub book_selling_point: Option<String>,
    pub competing_feel: Option<String>,
    pub first_30_chapter_promise: Option<String>,
    pub commercial_tags: Option<Vec<String>>,
    pub genre_id: Option<String>,
    pub primary_story_mode_id: Option<String>,
    pub secondary_story_mode_id: Option<String>,
    pub world_id: Option<String>,
    pub narrative_form: Option<String>,
    pub writing_mode: Option<String>,
    pub project_mode: Option<String>,
    pub narrative_pov: Option<String>,
    pub pace_preference: Option<String>,
    pub style_tone: Option<String>,
    pub emotion_intensity: Option<String>,
    pub ai_freedom: Option<String>,
    pub default_chapter_length: Option<i32>,
    pub estimated_chapter_count: Option<i32>,
    pub project_status: Option<String>,
    pub storyline_status: Option<String>,
    pub outline_status: Option<String>,
    pub resource_ready_score: Option<i32>,
    pub status: Option<String>,
}

// 链接 ensure_can_promote 的存在，避免 import 警告。
#[allow(dead_code)]
fn _force_link() {
    let _ = ensure_can_promote;
}
