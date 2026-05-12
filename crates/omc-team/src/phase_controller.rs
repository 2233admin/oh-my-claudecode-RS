use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TeamPhase {
    Initializing,
    Planning,
    Executing,
    Fixing,
    Completed,
    Failed,
    Paused,
}

impl TeamPhase {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Initializing => "initializing",
            Self::Planning => "planning",
            Self::Executing => "executing",
            Self::Fixing => "fixing",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Paused => "paused",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed)
    }

    pub fn is_active(&self) -> bool {
        matches!(self, Self::Planning | Self::Executing | Self::Fixing)
    }
}

#[derive(Debug, Clone)]
pub struct PhaseTransition {
    pub from: TeamPhase,
    pub to: TeamPhase,
    pub reason: String,
    pub timestamp: String,
}

pub struct PhaseController {
    current: TeamPhase,
    history: Vec<PhaseTransition>,
}

impl PhaseController {
    pub fn new() -> Self {
        Self {
            current: TeamPhase::Initializing,
            history: Vec::new(),
        }
    }

    pub fn current(&self) -> TeamPhase {
        self.current
    }

    pub fn history(&self) -> &[PhaseTransition] {
        &self.history
    }

    pub fn transition(&mut self, to: TeamPhase, reason: String) -> Result<(), PhaseError> {
        if !self.can_transition(to) {
            return Err(PhaseError::InvalidTransition {
                from: self.current,
                to,
            });
        }
        self.history.push(PhaseTransition {
            from: self.current,
            to,
            reason,
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
        self.current = to;
        Ok(())
    }

    pub fn can_transition(&self, to: TeamPhase) -> bool {
        valid_transitions(self.current).contains(&to)
    }

    pub fn infer_next_phase(&self, ctx: &PhaseContext) -> Option<TeamPhase> {
        match self.current {
            TeamPhase::Initializing => {
                if ctx.has_failures {
                    Some(TeamPhase::Failed)
                } else if ctx.all_tasks_assigned {
                    Some(TeamPhase::Planning)
                } else {
                    None
                }
            }
            TeamPhase::Planning => {
                if ctx.has_failures {
                    Some(TeamPhase::Failed)
                } else if ctx.all_tasks_assigned {
                    Some(TeamPhase::Executing)
                } else {
                    None
                }
            }
            TeamPhase::Executing => {
                if ctx.has_failures {
                    if ctx.fix_attempts < ctx.max_fix_attempts {
                        Some(TeamPhase::Fixing)
                    } else {
                        Some(TeamPhase::Failed)
                    }
                } else if ctx.has_blockers {
                    Some(TeamPhase::Paused)
                } else if ctx.all_tasks_completed {
                    if ctx.review_passed {
                        Some(TeamPhase::Completed)
                    } else {
                        Some(TeamPhase::Fixing)
                    }
                } else {
                    None
                }
            }
            TeamPhase::Fixing => {
                if ctx.fix_attempts >= ctx.max_fix_attempts {
                    Some(TeamPhase::Failed)
                } else if ctx.has_failures {
                    None
                } else if ctx.all_tasks_completed {
                    Some(TeamPhase::Completed)
                } else {
                    Some(TeamPhase::Executing)
                }
            }
            TeamPhase::Paused => {
                if !ctx.has_blockers {
                    Some(TeamPhase::Executing)
                } else {
                    None
                }
            }
            TeamPhase::Completed | TeamPhase::Failed => None,
        }
    }
}

impl Default for PhaseController {
    fn default() -> Self {
        Self::new()
    }
}

pub struct PhaseContext {
    pub all_tasks_assigned: bool,
    pub all_tasks_completed: bool,
    pub has_failures: bool,
    pub has_blockers: bool,
    pub review_passed: bool,
    pub fix_attempts: u32,
    pub max_fix_attempts: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhaseError {
    InvalidTransition { from: TeamPhase, to: TeamPhase },
}

impl std::fmt::Display for PhaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidTransition { from, to } => {
                write!(
                    f,
                    "invalid phase transition: {} -> {}",
                    from.as_str(),
                    to.as_str()
                )
            }
        }
    }
}

impl std::error::Error for PhaseError {}

fn valid_transitions(from: TeamPhase) -> &'static [TeamPhase] {
    match from {
        TeamPhase::Initializing => &[TeamPhase::Planning, TeamPhase::Failed],
        TeamPhase::Planning => &[TeamPhase::Executing, TeamPhase::Paused, TeamPhase::Failed],
        TeamPhase::Executing => &[
            TeamPhase::Fixing,
            TeamPhase::Completed,
            TeamPhase::Paused,
            TeamPhase::Failed,
        ],
        TeamPhase::Fixing => &[
            TeamPhase::Executing,
            TeamPhase::Completed,
            TeamPhase::Failed,
        ],
        TeamPhase::Completed => &[],
        TeamPhase::Failed => &[],
        TeamPhase::Paused => &[TeamPhase::Executing, TeamPhase::Planning],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_at_initializing() {
        let pc = PhaseController::default();
        assert_eq!(pc.current(), TeamPhase::Initializing);
        assert!(pc.history().is_empty());
    }

    #[test]
    fn terminal_states() {
        assert!(TeamPhase::Completed.is_terminal());
        assert!(TeamPhase::Failed.is_terminal());
        assert!(!TeamPhase::Executing.is_terminal());
        assert!(!TeamPhase::Initializing.is_terminal());
    }

    #[test]
    fn active_states() {
        assert!(TeamPhase::Planning.is_active());
        assert!(TeamPhase::Executing.is_active());
        assert!(TeamPhase::Fixing.is_active());
        assert!(!TeamPhase::Initializing.is_active());
        assert!(!TeamPhase::Completed.is_active());
        assert!(!TeamPhase::Paused.is_active());
    }

    #[test]
    fn valid_transition_initializing_to_planning() {
        let mut pc = PhaseController::default();
        assert!(pc.can_transition(TeamPhase::Planning));
        pc.transition(TeamPhase::Planning, "tasks assigned".into())
            .unwrap();
        assert_eq!(pc.current(), TeamPhase::Planning);
        assert_eq!(pc.history().len(), 1);
    }

    #[test]
    fn valid_transition_full_lifecycle() {
        let mut pc = PhaseController::default();
        pc.transition(TeamPhase::Planning, "init done".into())
            .unwrap();
        pc.transition(TeamPhase::Executing, "plan approved".into())
            .unwrap();
        pc.transition(TeamPhase::Fixing, "test failure".into())
            .unwrap();
        pc.transition(TeamPhase::Executing, "fix applied".into())
            .unwrap();
        pc.transition(TeamPhase::Completed, "all done".into())
            .unwrap();
        assert_eq!(pc.current(), TeamPhase::Completed);
        assert!(pc.current().is_terminal());
        assert_eq!(pc.history().len(), 5);
    }

    #[test]
    fn invalid_transition_rejected() {
        let mut pc = PhaseController::default();
        let err = pc
            .transition(TeamPhase::Completed, "skip".into())
            .unwrap_err();
        assert_eq!(
            err,
            PhaseError::InvalidTransition {
                from: TeamPhase::Initializing,
                to: TeamPhase::Completed
            }
        );
        assert_eq!(pc.current(), TeamPhase::Initializing);
        assert!(pc.history().is_empty());
    }

    #[test]
    fn cannot_transition_from_terminal() {
        let mut pc = PhaseController::default();
        pc.transition(TeamPhase::Failed, "fatal".into()).unwrap();
        assert!(!pc.can_transition(TeamPhase::Executing));
        let err = pc
            .transition(TeamPhase::Executing, "retry".into())
            .unwrap_err();
        assert_eq!(
            err,
            PhaseError::InvalidTransition {
                from: TeamPhase::Failed,
                to: TeamPhase::Executing,
            }
        );
    }

    #[test]
    fn pause_and_resume() {
        let mut pc = PhaseController::default();
        pc.transition(TeamPhase::Planning, "init done".into())
            .unwrap();
        pc.transition(TeamPhase::Paused, "blocked".into()).unwrap();
        assert_eq!(pc.current(), TeamPhase::Paused);
        pc.transition(TeamPhase::Executing, "unblocked".into())
            .unwrap();
        assert_eq!(pc.current(), TeamPhase::Executing);
    }

    #[test]
    fn history_tracks_transitions() {
        let mut pc = PhaseController::default();
        pc.transition(TeamPhase::Planning, "r1".into()).unwrap();
        pc.transition(TeamPhase::Executing, "r2".into()).unwrap();
        let history = pc.history();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].from, TeamPhase::Initializing);
        assert_eq!(history[0].to, TeamPhase::Planning);
        assert_eq!(history[0].reason, "r1");
        assert_eq!(history[1].from, TeamPhase::Planning);
        assert_eq!(history[1].to, TeamPhase::Executing);
        assert_eq!(history[1].reason, "r2");
    }

    #[test]
    fn infer_initializing_to_planning() {
        let pc = PhaseController::default();
        let ctx = PhaseContext {
            all_tasks_assigned: true,
            all_tasks_completed: false,
            has_failures: false,
            has_blockers: false,
            review_passed: false,
            fix_attempts: 0,
            max_fix_attempts: 3,
        };
        assert_eq!(pc.infer_next_phase(&ctx), Some(TeamPhase::Planning));
    }

    #[test]
    fn infer_executing_to_completed_when_review_passes() {
        let mut pc = PhaseController::default();
        pc.transition(TeamPhase::Planning, "done".into()).unwrap();
        pc.transition(TeamPhase::Executing, "go".into()).unwrap();
        let ctx = PhaseContext {
            all_tasks_assigned: true,
            all_tasks_completed: true,
            has_failures: false,
            has_blockers: false,
            review_passed: true,
            fix_attempts: 0,
            max_fix_attempts: 3,
        };
        assert_eq!(pc.infer_next_phase(&ctx), Some(TeamPhase::Completed));
    }

    #[test]
    fn infer_executing_to_fixing_on_failure() {
        let mut pc = PhaseController::default();
        pc.transition(TeamPhase::Planning, "done".into()).unwrap();
        pc.transition(TeamPhase::Executing, "go".into()).unwrap();
        let ctx = PhaseContext {
            all_tasks_assigned: true,
            all_tasks_completed: false,
            has_failures: true,
            has_blockers: false,
            review_passed: false,
            fix_attempts: 1,
            max_fix_attempts: 3,
        };
        assert_eq!(pc.infer_next_phase(&ctx), Some(TeamPhase::Fixing));
    }

    #[test]
    fn infer_fixing_exhausted_to_failed() {
        let mut pc = PhaseController::default();
        pc.transition(TeamPhase::Planning, "done".into()).unwrap();
        pc.transition(TeamPhase::Executing, "go".into()).unwrap();
        pc.transition(TeamPhase::Fixing, "fail".into()).unwrap();
        let ctx = PhaseContext {
            all_tasks_assigned: true,
            all_tasks_completed: false,
            has_failures: true,
            has_blockers: false,
            review_passed: false,
            fix_attempts: 3,
            max_fix_attempts: 3,
        };
        assert_eq!(pc.infer_next_phase(&ctx), Some(TeamPhase::Failed));
    }

    #[test]
    fn infer_terminal_returns_none() {
        let mut pc = PhaseController::default();
        pc.transition(TeamPhase::Failed, "fatal".into()).unwrap();
        let ctx = PhaseContext {
            all_tasks_assigned: true,
            all_tasks_completed: true,
            has_failures: false,
            has_blockers: false,
            review_passed: true,
            fix_attempts: 0,
            max_fix_attempts: 3,
        };
        assert_eq!(pc.infer_next_phase(&ctx), None);
    }

    #[test]
    fn infer_executing_paused_on_blockers() {
        let mut pc = PhaseController::default();
        pc.transition(TeamPhase::Planning, "done".into()).unwrap();
        pc.transition(TeamPhase::Executing, "go".into()).unwrap();
        let ctx = PhaseContext {
            all_tasks_assigned: true,
            all_tasks_completed: false,
            has_failures: false,
            has_blockers: true,
            review_passed: false,
            fix_attempts: 0,
            max_fix_attempts: 3,
        };
        assert_eq!(pc.infer_next_phase(&ctx), Some(TeamPhase::Paused));
    }

    #[test]
    fn as_str_roundtrip() {
        let phases = [
            TeamPhase::Initializing,
            TeamPhase::Planning,
            TeamPhase::Executing,
            TeamPhase::Fixing,
            TeamPhase::Completed,
            TeamPhase::Failed,
            TeamPhase::Paused,
        ];
        for p in &phases {
            let s = p.as_str();
            let deserialized: TeamPhase = serde_json::from_str(&format!("\"{s}\"")).unwrap();
            assert_eq!(*p, deserialized);
        }
    }
}
