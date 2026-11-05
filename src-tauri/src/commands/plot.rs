use novel_application::{
    AppError, CausalityEdgeView, CreatePlotThreadInput, NewCausalityEdge, NovelRepository,
    PlotThreadEventView, PlotThreadView, PlotTransition, StoryFactView,
};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::state::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePlotThreadRequest {
    pub plot_id: String,
    pub novel_id: String,
    pub title: String,
    pub body: String,
    pub chapter_id: String,
    pub source_revision: i64,
    pub evidence: String,
    pub command_id: String,
}

#[tauri::command]
pub async fn create_plot_thread(
    state: State<'_, AppState>,
    request: CreatePlotThreadRequest,
) -> Result<PlotThreadView, AppError> {
    let request_hash = novel_application::sha256_hex(
        format!(
            "{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}",
            request.plot_id,
            request.novel_id,
            request.title,
            request.body,
            request.chapter_id,
            request.source_revision,
            request.evidence
        )
        .as_bytes(),
    );
    state
        .plot
        .create(CreatePlotThreadInput {
            id: request.plot_id,
            novel_id: request.novel_id,
            title: request.title,
            body: request.body,
            chapter_id: request.chapter_id,
            source_revision: request.source_revision,
            evidence: request.evidence,
            command_id: request.command_id,
            request_hash,
            now_ms: novel_platform::now_ms(),
        })
        .await
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransitionPlotThreadRequest {
    pub plot_id: String,
    pub novel_id: String,
    pub expected_version: i64,
    pub action: String,
    pub chapter_id: String,
    pub source_revision: i64,
    pub evidence: String,
    pub command_id: String,
}

#[tauri::command]
pub async fn transition_plot_thread(
    state: State<'_, AppState>,
    request: TransitionPlotThreadRequest,
) -> Result<PlotThreadView, AppError> {
    let request_hash = novel_application::sha256_hex(
        format!(
            "{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}",
            request.plot_id,
            request.novel_id,
            request.expected_version,
            request.action,
            request.chapter_id,
            request.source_revision,
            request.evidence
        )
        .as_bytes(),
    );
    state
        .plot
        .transition(PlotTransition {
            plot_id: request.plot_id,
            novel_id: request.novel_id,
            expected_version: request.expected_version,
            action: request.action,
            chapter_id: request.chapter_id,
            source_revision: request.source_revision,
            evidence: request.evidence,
            command_id: request.command_id,
            request_hash,
            now_ms: novel_platform::now_ms(),
        })
        .await
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCausalityRequest {
    pub edge_id: String,
    pub novel_id: String,
    pub from_kind: String,
    pub from_id: String,
    pub to_kind: String,
    pub to_id: String,
    pub relation: String,
    pub chapter_id: String,
    pub source_revision: i64,
    pub evidence: String,
}

#[tauri::command]
pub async fn create_plot_causality(
    state: State<'_, AppState>,
    request: CreateCausalityRequest,
) -> Result<CausalityEdgeView, AppError> {
    state
        .plot
        .create_edge(NewCausalityEdge {
            id: request.edge_id,
            novel_id: request.novel_id,
            from_kind: request.from_kind,
            from_id: request.from_id,
            to_kind: request.to_kind,
            to_id: request.to_id,
            relation: request.relation,
            chapter_id: request.chapter_id,
            source_revision: request.source_revision,
            evidence: request.evidence,
            now_ms: novel_platform::now_ms(),
        })
        .await
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineEntryView {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub chapter_id: String,
    pub chapter_order: i32,
    pub chapter_title: Option<String>,
    pub volume_id: Option<String>,
    pub volume_title: Option<String>,
    pub source_revision: Option<String>,
    pub evidence: Option<String>,
    pub status: String,
    pub entity_name: Option<String>,
    pub plot_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlotWorkspaceView {
    pub novel_id: String,
    pub chapter_id: String,
    pub chapter_order: i32,
    pub chapter_version: i64,
    pub threads: Vec<PlotThreadView>,
    pub events: Vec<PlotThreadEventView>,
    pub edges: Vec<CausalityEdgeView>,
    pub timeline: Vec<TimelineEntryView>,
    pub timeline_truncated: bool,
    pub timeline_limit: usize,
}

const TIMELINE_LIMIT: usize = 500;

#[tauri::command]
pub async fn get_plot_workspace(
    state: State<'_, AppState>,
    novel_id: String,
    chapter_id: String,
) -> Result<PlotWorkspaceView, AppError> {
    let chapter = state.kernel.get_chapter(&novel_id, &chapter_id).await?;
    let as_of = chapter.sort_order;
    let threads = state.plot.list(&novel_id, as_of).await?;
    let events = state
        .plot
        .events(&novel_id, as_of, i64::from(TIMELINE_LIMIT as i32) + 1)
        .await?;
    let edges = state.plot.edges(&novel_id).await?;
    let story_facts = state.kernel.list_story_facts(&novel_id, as_of).await?;
    let headers = state.kernel.list_chapter_headers(&novel_id).await?;
    let volumes = state.kernel.list_volumes(&novel_id).await?;
    let header_by_id: std::collections::HashMap<&str, &novel_application::ChapterHeader> = headers
        .iter()
        .map(|header| (header.id.as_str(), header))
        .collect();
    let volume_titles: std::collections::HashMap<&str, &str> = volumes
        .iter()
        .map(|volume| (volume.id.as_str(), volume.title.as_str()))
        .collect();

    let mut timeline: Vec<TimelineEntryView> = story_facts
        .iter()
        .filter(|fact| {
            matches!(
                fact.fact_type.as_str(),
                "timeline_event" | "character_state"
            )
        })
        .map(|fact| story_fact_entry(fact, &header_by_id, &volume_titles))
        .collect();
    timeline.extend(events.iter().map(|event| {
        TimelineEntryView {
            id: event.id.clone(),
            kind: format!("plot_{}", event.kind),
            title: event.title.clone(),
            body: event.evidence.clone(),
            chapter_id: event.chapter_id.clone(),
            chapter_order: event.chapter_order,
            chapter_title: event.chapter_title.clone(),
            volume_id: header_by_id
                .get(event.chapter_id.as_str())
                .and_then(|header| header.volume_id.clone()),
            volume_title: header_by_id
                .get(event.chapter_id.as_str())
                .and_then(|header| header.volume_id.as_deref())
                .and_then(|volume| volume_titles.get(volume).map(|title| (*title).to_string())),
            source_revision: None,
            evidence: Some(event.evidence.clone()),
            status: event.to_status.clone(),
            entity_name: None,
            plot_id: Some(event.plot_id.clone()),
        }
    }));
    timeline.sort_by(|left, right| {
        left.chapter_order
            .cmp(&right.chapter_order)
            .then_with(|| left.kind.cmp(&right.kind))
            .then_with(|| left.id.cmp(&right.id))
    });
    let timeline_truncated = timeline.len() > TIMELINE_LIMIT;
    timeline.truncate(TIMELINE_LIMIT);
    let events_truncated = events.len() > TIMELINE_LIMIT;
    let mut events = events;
    events.truncate(TIMELINE_LIMIT);

    Ok(PlotWorkspaceView {
        novel_id,
        chapter_id,
        chapter_order: as_of,
        chapter_version: chapter.official_version,
        threads,
        events,
        edges,
        timeline,
        timeline_truncated: timeline_truncated || events_truncated,
        timeline_limit: TIMELINE_LIMIT,
    })
}

fn story_fact_entry(
    fact: &StoryFactView,
    headers: &std::collections::HashMap<&str, &novel_application::ChapterHeader>,
    volume_titles: &std::collections::HashMap<&str, &str>,
) -> TimelineEntryView {
    let header = fact
        .source_chapter_id
        .as_deref()
        .and_then(|id| headers.get(id).copied());
    TimelineEntryView {
        id: fact.id.clone(),
        kind: fact.fact_type.clone(),
        title: fact
            .character_name
            .clone()
            .unwrap_or_else(|| fact.fact_type.clone()),
        body: fact.body.clone(),
        chapter_id: fact.source_chapter_id.clone().unwrap_or_default(),
        chapter_order: fact.chapter_order,
        chapter_title: header.map(|item| item.title.clone()),
        volume_id: header.and_then(|item| item.volume_id.clone()),
        volume_title: header
            .and_then(|item| item.volume_id.as_deref())
            .and_then(|volume| volume_titles.get(volume).map(|title| (*title).to_string())),
        source_revision: fact.source_revision.clone(),
        evidence: fact.evidence_quote.clone(),
        status: fact.status.clone(),
        entity_name: fact.character_name.clone(),
        plot_id: None,
    }
}
