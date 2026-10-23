use novel_domain::{ArtifactStatus, ArtifactView};
use serde_json::Value;

pub const PRODUCTION_PLAN_VERSION: &str = "novel-production-v2";
pub const PRODUCTION_PLAN_VERSION_V1: &str = "novel-production-v1";
pub const CHAPTER_PLAN_VERSION: &str = "chapter-chain-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepScope {
    Book,
    Chapter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepGuard {
    Always,
    WhenPositive {
        artifact: &'static str,
        field: &'static str,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StepSpec {
    pub id: &'static str,
    pub requires: &'static [&'static str],
    pub produces: &'static str,
    pub gateable: bool,
    pub scope: StepScope,
    pub guard: StepGuard,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowDefinition {
    pub version: &'static str,
    pub chapter_version: &'static str,
    pub steps: Vec<StepSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NextAction {
    RunStep {
        step_id: &'static str,
    },
    OpenGate {
        step_id: &'static str,
        artifact_type: &'static str,
    },
    Succeed,
    Blocked {
        step_id: &'static str,
    },
}

pub fn book_scope() -> &'static str {
    "book"
}

pub fn chapter_scope(chapter_id: &str) -> String {
    format!("chapter:{chapter_id}")
}

pub fn production_plan() -> WorkflowDefinition {
    plan_with_version(PRODUCTION_PLAN_VERSION)
}

pub fn production_plan_v1() -> WorkflowDefinition {
    plan_with_version(PRODUCTION_PLAN_VERSION_V1)
}

pub fn plan_for_version(version: &str) -> Option<WorkflowDefinition> {
    match version {
        PRODUCTION_PLAN_VERSION => Some(production_plan()),
        PRODUCTION_PLAN_VERSION_V1 => Some(production_plan_v1()),
        _ => None,
    }
}

fn plan_with_version(version: &'static str) -> WorkflowDefinition {
    WorkflowDefinition {
        version,
        chapter_version: CHAPTER_PLAN_VERSION,
        steps: vec![
            step(
                "story_macro",
                &["novel_seed"],
                "story_macro",
                false,
                StepScope::Book,
                StepGuard::Always,
            ),
            step(
                "character_cast",
                &["story_macro"],
                "character_cast",
                true,
                StepScope::Book,
                StepGuard::Always,
            ),
            step(
                "volume_strategy",
                &["character_cast"],
                "volume_strategy",
                true,
                StepScope::Book,
                StepGuard::Always,
            ),
            step(
                "chapter_list",
                &["volume_strategy"],
                "chapter_list",
                false,
                StepScope::Book,
                StepGuard::Always,
            ),
            step(
                "context",
                &["chapter_list"],
                "context",
                false,
                StepScope::Chapter,
                StepGuard::Always,
            ),
            step(
                "draft",
                &["context"],
                "chapter_draft",
                false,
                StepScope::Chapter,
                StepGuard::Always,
            ),
            step(
                "consistency_review",
                &["chapter_draft"],
                "consistency_review",
                false,
                StepScope::Chapter,
                StepGuard::Always,
            ),
            step(
                "bounded_repair",
                &["consistency_review"],
                "repair_notes",
                false,
                StepScope::Chapter,
                StepGuard::WhenPositive {
                    artifact: "consistency_review",
                    field: "issues",
                },
            ),
            step(
                "approval",
                &["consistency_review"],
                "chapter_approval",
                true,
                StepScope::Chapter,
                StepGuard::Always,
            ),
            step(
                "commit",
                &["chapter_approval"],
                "chapter_commit",
                false,
                StepScope::Chapter,
                StepGuard::Always,
            ),
        ],
    }
}

fn step(
    id: &'static str,
    requires: &'static [&'static str],
    produces: &'static str,
    gateable: bool,
    scope: StepScope,
    guard: StepGuard,
) -> StepSpec {
    StepSpec {
        id,
        requires,
        produces,
        gateable,
        scope,
        guard,
    }
}

pub fn find_step<'a>(plan: &'a WorkflowDefinition, step_id: &str) -> Option<&'a StepSpec> {
    plan.steps.iter().find(|step| step.id == step_id)
}

pub fn next_action(
    plan: &WorkflowDefinition,
    artifacts: &[ArtifactView],
    chapter_id: &str,
) -> NextAction {
    let chapter_key = chapter_scope(chapter_id);
    for step in &plan.steps {
        if !guard_allows(step.guard, artifacts, &chapter_key) {
            continue;
        }
        let scope = scope_key(step.scope, &chapter_key);
        let ready = requirements_met(plan, step, artifacts, &chapter_key);
        match latest(artifacts, step.produces, scope) {
            None if ready => return NextAction::RunStep { step_id: step.id },
            None => return NextAction::Blocked { step_id: step.id },
            Some(artifact)
                if artifact.protected_user_content
                    || artifact.status == ArtifactStatus::UserEdited => {}
            Some(artifact) if artifact.status == ArtifactStatus::Stale && ready => {
                return NextAction::RunStep { step_id: step.id };
            }
            Some(artifact) if artifact.status == ArtifactStatus::Stale => {
                return NextAction::Blocked { step_id: step.id };
            }
            Some(artifact) if step.gateable && artifact.status == ArtifactStatus::Draft => {
                return NextAction::OpenGate {
                    step_id: step.id,
                    artifact_type: step.produces,
                };
            }
            Some(artifact) if artifact.status.satisfies_dependency(step.gateable) => {}
            Some(_) if ready => return NextAction::RunStep { step_id: step.id },
            Some(_) => return NextAction::Blocked { step_id: step.id },
        }
    }
    NextAction::Succeed
}

fn requirements_met(
    plan: &WorkflowDefinition,
    step: &StepSpec,
    artifacts: &[ArtifactView],
    chapter_key: &str,
) -> bool {
    step.requires
        .iter()
        .all(|required| dependency_ready(plan, artifacts, required, chapter_key))
}

fn dependency_ready(
    plan: &WorkflowDefinition,
    artifacts: &[ArtifactView],
    artifact_type: &str,
    chapter_key: &str,
) -> bool {
    let scope = scope_for_type(plan, artifact_type, chapter_key);
    let gateable = plan
        .steps
        .iter()
        .find(|step| step.produces == artifact_type)
        .is_some_and(|step| step.gateable);
    latest(artifacts, artifact_type, scope).is_some_and(|artifact| {
        artifact.status.satisfies_dependency(gateable) || artifact.protected_user_content
    })
}

fn guard_allows(guard: StepGuard, artifacts: &[ArtifactView], chapter_key: &str) -> bool {
    match guard {
        StepGuard::Always => true,
        StepGuard::WhenPositive { artifact, field } => latest(artifacts, artifact, chapter_key)
            .and_then(|item| serde_json::from_str::<Value>(&item.payload_json).ok())
            .and_then(|value| value.get(field).and_then(Value::as_i64))
            .is_some_and(|count| count > 0),
    }
}

fn scope_key(scope: StepScope, chapter_key: &str) -> &str {
    match scope {
        StepScope::Book => book_scope(),
        StepScope::Chapter => chapter_key,
    }
}

fn scope_for_type<'a>(
    plan: &WorkflowDefinition,
    artifact_type: &str,
    chapter_key: &'a str,
) -> &'a str {
    if artifact_type == "novel_seed" {
        return book_scope();
    }
    match plan
        .steps
        .iter()
        .find(|step| step.produces == artifact_type)
        .map(|step| step.scope)
    {
        Some(StepScope::Book) | None => book_scope(),
        Some(StepScope::Chapter) => chapter_key,
    }
}

fn latest<'a>(
    artifacts: &'a [ArtifactView],
    artifact_type: &str,
    scope: &str,
) -> Option<&'a ArtifactView> {
    artifacts
        .iter()
        .filter(|artifact| artifact.artifact_type == artifact_type && artifact.scope == scope)
        .max_by_key(|artifact| artifact.version)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(
        kind: &str,
        scope: &str,
        version: i64,
        status: ArtifactStatus,
        payload: &str,
    ) -> ArtifactView {
        ArtifactView {
            artifact_type: kind.into(),
            scope: scope.into(),
            version,
            status,
            protected_user_content: status == ArtifactStatus::UserEdited,
            payload_json: payload.into(),
        }
    }

    #[test]
    fn starts_at_story_macro_when_only_the_seed_exists() {
        let plan = production_plan();
        let facts = vec![artifact(
            "novel_seed",
            "book",
            1,
            ArtifactStatus::Confirmed,
            "{}",
        )];
        assert_eq!(
            next_action(&plan, &facts, "c1"),
            NextAction::RunStep {
                step_id: "story_macro"
            }
        );
    }

    #[test]
    fn draft_gate_opens_until_the_author_confirms() {
        let plan = production_plan();
        let chapter = chapter_scope("c1");
        let mut facts = vec![
            artifact("novel_seed", "book", 1, ArtifactStatus::Confirmed, "{}"),
            artifact("story_macro", "book", 1, ArtifactStatus::Confirmed, "{}"),
            artifact("character_cast", "book", 1, ArtifactStatus::Draft, "{}"),
        ];
        assert_eq!(
            next_action(&plan, &facts, "c1"),
            NextAction::OpenGate {
                step_id: "character_cast",
                artifact_type: "character_cast"
            }
        );
        facts[2].status = ArtifactStatus::Confirmed;
        facts.push(artifact(
            "volume_strategy",
            "book",
            1,
            ArtifactStatus::Confirmed,
            "{}",
        ));
        facts.push(artifact(
            "chapter_list",
            "book",
            1,
            ArtifactStatus::Confirmed,
            "{}",
        ));
        facts.push(artifact(
            "context",
            &chapter,
            1,
            ArtifactStatus::Confirmed,
            "{}",
        ));
        facts.push(artifact(
            "chapter_draft",
            &chapter,
            1,
            ArtifactStatus::UserEdited,
            "{}",
        ));
        let action = next_action(&plan, &facts, "c1");
        assert_eq!(
            action,
            NextAction::RunStep {
                step_id: "consistency_review"
            }
        );
    }

    #[test]
    fn repair_is_skipped_when_review_has_no_issues() {
        let plan = production_plan();
        let chapter = chapter_scope("c1");
        let facts = vec![
            artifact("novel_seed", "book", 1, ArtifactStatus::Confirmed, "{}"),
            artifact("story_macro", "book", 1, ArtifactStatus::Confirmed, "{}"),
            artifact("character_cast", "book", 1, ArtifactStatus::Confirmed, "{}"),
            artifact(
                "volume_strategy",
                "book",
                1,
                ArtifactStatus::Confirmed,
                "{}",
            ),
            artifact("chapter_list", "book", 1, ArtifactStatus::Confirmed, "{}"),
            artifact("context", &chapter, 1, ArtifactStatus::Confirmed, "{}"),
            artifact("chapter_draft", &chapter, 1, ArtifactStatus::Draft, "{}"),
            artifact(
                "consistency_review",
                &chapter,
                1,
                ArtifactStatus::Confirmed,
                r#"{"issues":0}"#,
            ),
        ];
        assert_eq!(
            next_action(&plan, &facts, "c1"),
            NextAction::RunStep {
                step_id: "approval"
            }
        );
    }
}
