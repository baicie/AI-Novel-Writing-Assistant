//! 版本化工作流定义、下一步决策和中断恢复。调度所依赖的事务在存储层提交。

mod plan;
mod recover;

pub use plan::{
    CHAPTER_PLAN_VERSION, NextAction, PRODUCTION_PLAN_VERSION, StepGuard, StepScope, StepSpec,
    WorkflowDefinition, book_scope, chapter_scope, find_step, next_action, production_plan,
};
pub use recover::{AttemptPhase, AttemptView, RecoveryDecision, ResumeIntent, decide};

pub use novel_domain::{ArtifactStatus, ArtifactView};
