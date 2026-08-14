//! Host-neutral decision contract for clarify/plan/execute/verify workflows.
//!
//! Hosts and the existing team runtime provide evidence; this module never
//! starts an agent or persists lifecycle state.

use serde::{Deserialize, Serialize};

pub const WORKFLOW_SCHEMA_VERSION: &str = "omc.workflow.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowStage {
    Initializing,
    Clarifying,
    Planning,
    Executing,
    Verifying,
    Fixing,
    Paused,
    Completed,
    Failed,
}

impl WorkflowStage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Initializing => "initializing",
            Self::Clarifying => "clarifying",
            Self::Planning => "planning",
            Self::Executing => "executing",
            Self::Verifying => "verifying",
            Self::Fixing => "fixing",
            Self::Paused => "paused",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowContext {
    #[serde(default)]
    pub requirements_clarified: bool,
    #[serde(default)]
    pub all_tasks_assigned: bool,
    #[serde(default)]
    pub plan_approved: bool,
    #[serde(default)]
    pub all_tasks_completed: bool,
    #[serde(default)]
    pub verification_passed: bool,
    #[serde(default)]
    pub has_failures: bool,
    #[serde(default)]
    pub has_blockers: bool,
    #[serde(default)]
    pub fix_attempts: u32,
    #[serde(default = "default_max_fix_attempts")]
    pub max_fix_attempts: u32,
}

impl Default for WorkflowContext {
    fn default() -> Self {
        Self {
            requirements_clarified: false,
            all_tasks_assigned: false,
            plan_approved: false,
            all_tasks_completed: false,
            verification_passed: false,
            has_failures: false,
            has_blockers: false,
            fix_attempts: 0,
            max_fix_attempts: default_max_fix_attempts(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowAdvanceRequest {
    pub current_stage: WorkflowStage,
    #[serde(flatten)]
    pub context: WorkflowContext,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowDecision {
    Advance,
    Wait,
    Terminal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowAdvancePayload {
    pub schema_version: String,
    pub current_stage: WorkflowStage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_stage: Option<WorkflowStage>,
    pub decision: WorkflowDecision,
    pub reason: String,
}

pub fn advance_workflow(request: &WorkflowAdvanceRequest) -> WorkflowAdvancePayload {
    let next_stage = next_stage(request.current_stage, &request.context);
    let decision = match next_stage {
        Some(_) => WorkflowDecision::Advance,
        None if request.current_stage.is_terminal() => WorkflowDecision::Terminal,
        None => WorkflowDecision::Wait,
    };
    let reason = match next_stage {
        Some(stage) => format!("{} -> {}", request.current_stage.as_str(), stage.as_str()),
        None if request.current_stage.is_terminal() => {
            format!("{} is terminal", request.current_stage.as_str())
        }
        None if request.context.has_blockers => "waiting for blockers to clear".into(),
        None => format!(
            "waiting for evidence to leave {}",
            request.current_stage.as_str()
        ),
    };
    WorkflowAdvancePayload {
        schema_version: WORKFLOW_SCHEMA_VERSION.into(),
        current_stage: request.current_stage,
        next_stage,
        decision,
        reason,
    }
}

fn next_stage(current: WorkflowStage, context: &WorkflowContext) -> Option<WorkflowStage> {
    if current.is_terminal() {
        return None;
    }
    match current {
        WorkflowStage::Initializing => {
            if context.has_failures {
                Some(WorkflowStage::Failed)
            } else if context.requirements_clarified {
                Some(WorkflowStage::Planning)
            } else {
                Some(WorkflowStage::Clarifying)
            }
        }
        WorkflowStage::Clarifying => {
            if context.has_failures {
                Some(WorkflowStage::Failed)
            } else if context.has_blockers {
                Some(WorkflowStage::Paused)
            } else if context.requirements_clarified {
                Some(WorkflowStage::Planning)
            } else {
                None
            }
        }
        WorkflowStage::Planning => {
            if context.has_failures {
                Some(WorkflowStage::Failed)
            } else if context.has_blockers {
                Some(WorkflowStage::Paused)
            } else if context.all_tasks_assigned && context.plan_approved {
                Some(WorkflowStage::Executing)
            } else {
                None
            }
        }
        WorkflowStage::Executing => {
            if context.has_failures {
                if context.fix_attempts < context.max_fix_attempts {
                    Some(WorkflowStage::Fixing)
                } else {
                    Some(WorkflowStage::Failed)
                }
            } else if context.has_blockers {
                Some(WorkflowStage::Paused)
            } else if context.all_tasks_completed {
                Some(WorkflowStage::Verifying)
            } else {
                None
            }
        }
        WorkflowStage::Verifying => {
            if context.has_failures {
                if context.fix_attempts < context.max_fix_attempts {
                    Some(WorkflowStage::Fixing)
                } else {
                    Some(WorkflowStage::Failed)
                }
            } else if context.has_blockers {
                Some(WorkflowStage::Paused)
            } else if context.verification_passed {
                Some(WorkflowStage::Completed)
            } else {
                Some(WorkflowStage::Fixing)
            }
        }
        WorkflowStage::Fixing => {
            if context.fix_attempts >= context.max_fix_attempts {
                Some(WorkflowStage::Failed)
            } else if context.has_blockers {
                Some(WorkflowStage::Paused)
            } else if context.all_tasks_completed {
                Some(WorkflowStage::Verifying)
            } else {
                Some(WorkflowStage::Executing)
            }
        }
        WorkflowStage::Paused => {
            if context.has_blockers {
                None
            } else if context.requirements_clarified && !context.plan_approved {
                Some(WorkflowStage::Planning)
            } else {
                Some(WorkflowStage::Executing)
            }
        }
        WorkflowStage::Completed | WorkflowStage::Failed => None,
    }
}

fn default_max_fix_attempts() -> u32 {
    3
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follows_clarify_plan_execute_verify_lifecycle() {
        let mut request = WorkflowAdvanceRequest {
            current_stage: WorkflowStage::Initializing,
            context: WorkflowContext::default(),
        };
        assert_eq!(
            advance_workflow(&request).next_stage,
            Some(WorkflowStage::Clarifying)
        );
        request.current_stage = WorkflowStage::Clarifying;
        request.context.requirements_clarified = true;
        assert_eq!(
            advance_workflow(&request).next_stage,
            Some(WorkflowStage::Planning)
        );
        request.current_stage = WorkflowStage::Planning;
        request.context.all_tasks_assigned = true;
        request.context.plan_approved = true;
        assert_eq!(
            advance_workflow(&request).next_stage,
            Some(WorkflowStage::Executing)
        );
        request.current_stage = WorkflowStage::Executing;
        request.context.all_tasks_completed = true;
        assert_eq!(
            advance_workflow(&request).next_stage,
            Some(WorkflowStage::Verifying)
        );
        request.current_stage = WorkflowStage::Verifying;
        request.context.verification_passed = true;
        assert_eq!(
            advance_workflow(&request).next_stage,
            Some(WorkflowStage::Completed)
        );
    }

    #[test]
    fn failures_are_bounded_and_wait_does_not_fabricate_progress() {
        let failed = WorkflowAdvanceRequest {
            current_stage: WorkflowStage::Executing,
            context: WorkflowContext {
                has_failures: true,
                fix_attempts: 3,
                ..WorkflowContext::default()
            },
        };
        assert_eq!(
            advance_workflow(&failed).next_stage,
            Some(WorkflowStage::Failed)
        );
        let waiting = WorkflowAdvanceRequest {
            current_stage: WorkflowStage::Planning,
            context: WorkflowContext::default(),
        };
        assert_eq!(advance_workflow(&waiting).decision, WorkflowDecision::Wait);
        assert!(advance_workflow(&waiting).next_stage.is_none());
    }
}
