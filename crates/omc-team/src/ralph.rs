use serde::{Deserialize, Serialize};

/// Ralph loop states
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RalphState {
    /// Initial state — gathering requirements
    Gathering,
    /// Planning the implementation
    Planning,
    /// Executing the plan
    Executing,
    /// Reviewing results
    Reviewing,
    /// Loop complete
    Complete,
    /// Paused by user
    Paused,
}

/// Ralph loop configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RalphConfig {
    /// Maximum iterations before stopping
    pub max_iterations: u32,
    /// Current iteration count
    pub current_iteration: u32,
    /// Whether to auto-approve between iterations
    pub auto_approve: bool,
    /// State machine current state
    pub state: RalphState,
}

impl Default for RalphConfig {
    fn default() -> Self {
        Self {
            max_iterations: 10,
            current_iteration: 0,
            auto_approve: false,
            state: RalphState::Gathering,
        }
    }
}

impl RalphConfig {
    /// Advance to the next state. Returns false if already Complete or max iterations exceeded.
    pub fn advance(&mut self) -> bool {
        match self.state {
            RalphState::Gathering => {
                self.state = RalphState::Planning;
                true
            }
            RalphState::Planning => {
                self.state = RalphState::Executing;
                true
            }
            RalphState::Executing => {
                self.state = RalphState::Reviewing;
                true
            }
            RalphState::Reviewing => {
                self.current_iteration += 1;
                if self.current_iteration >= self.max_iterations {
                    self.state = RalphState::Complete;
                    false
                } else {
                    self.state = RalphState::Gathering;
                    true
                }
            }
            RalphState::Complete => false,
            RalphState::Paused => false,
        }
    }

    /// Reset to initial state
    pub fn reset(&mut self) {
        self.state = RalphState::Gathering;
        self.current_iteration = 0;
    }

    /// Check if the loop should continue
    pub fn should_continue(&self) -> bool {
        self.state != RalphState::Complete
            && self.state != RalphState::Paused
            && self.current_iteration < self.max_iterations
    }

    /// Pause the loop. Returns false if already Complete.
    pub fn pause(&mut self) -> bool {
        if self.state == RalphState::Complete {
            return false;
        }
        self.state = RalphState::Paused;
        true
    }

    /// Resume from Paused state back to Gathering. Returns false if not Paused.
    pub fn resume(&mut self) -> bool {
        if self.state != RalphState::Paused {
            return false;
        }
        self.state = RalphState::Gathering;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_starts_at_gathering() {
        let cfg = RalphConfig::default();
        assert_eq!(cfg.state, RalphState::Gathering);
        assert_eq!(cfg.current_iteration, 0);
        assert_eq!(cfg.max_iterations, 10);
        assert!(!cfg.auto_approve);
    }

    #[test]
    fn full_cycle_advances_through_all_states() {
        let mut cfg = RalphConfig {
            max_iterations: 1,
            ..Default::default()
        };

        assert_eq!(cfg.state, RalphState::Gathering);
        assert!(cfg.advance());
        assert_eq!(cfg.state, RalphState::Planning);
        assert!(cfg.advance());
        assert_eq!(cfg.state, RalphState::Executing);
        assert!(cfg.advance());
        assert_eq!(cfg.state, RalphState::Reviewing);
        // Last iteration: advance should return false and move to Complete
        assert!(!cfg.advance());
        assert_eq!(cfg.state, RalphState::Complete);
        assert_eq!(cfg.current_iteration, 1);
    }

    #[test]
    fn loops_back_to_gathering_when_iterations_remain() {
        let mut cfg = RalphConfig {
            max_iterations: 3,
            ..Default::default()
        };

        // First cycle
        assert!(cfg.advance()); // Gathering -> Planning
        assert!(cfg.advance()); // Planning -> Executing
        assert!(cfg.advance()); // Executing -> Reviewing
        assert!(cfg.advance()); // Reviewing -> Gathering (iteration 1 of 3)
        assert_eq!(cfg.state, RalphState::Gathering);
        assert_eq!(cfg.current_iteration, 1);

        // Second cycle
        assert!(cfg.advance());
        assert!(cfg.advance());
        assert!(cfg.advance());
        assert!(cfg.advance()); // iteration 2 of 3
        assert_eq!(cfg.current_iteration, 2);

        // Third cycle — should complete
        assert!(cfg.advance());
        assert!(cfg.advance());
        assert!(cfg.advance());
        assert!(!cfg.advance()); // iteration 3 of 3 -> Complete
        assert_eq!(cfg.state, RalphState::Complete);
        assert_eq!(cfg.current_iteration, 3);
    }

    #[test]
    fn advance_from_complete_returns_false() {
        let mut cfg = RalphConfig {
            state: RalphState::Complete,
            current_iteration: 10,
            max_iterations: 10,
            auto_approve: false,
        };
        assert!(!cfg.advance());
        assert_eq!(cfg.state, RalphState::Complete);
    }

    #[test]
    fn advance_from_paused_returns_false() {
        let mut cfg = RalphConfig {
            state: RalphState::Paused,
            ..Default::default()
        };
        assert!(!cfg.advance());
        assert_eq!(cfg.state, RalphState::Paused);
    }

    #[test]
    fn reset_returns_to_initial_state() {
        let mut cfg = RalphConfig {
            state: RalphState::Reviewing,
            current_iteration: 5,
            max_iterations: 10,
            auto_approve: true,
        };
        cfg.reset();
        assert_eq!(cfg.state, RalphState::Gathering);
        assert_eq!(cfg.current_iteration, 0);
        // auto_approve and max_iterations are preserved
        assert!(cfg.auto_approve);
        assert_eq!(cfg.max_iterations, 10);
    }

    #[test]
    fn should_continue_true_for_active_states() {
        let cfg = RalphConfig::default();
        assert!(cfg.should_continue());

        for state in [
            RalphState::Gathering,
            RalphState::Planning,
            RalphState::Executing,
            RalphState::Reviewing,
        ] {
            let c = RalphConfig {
                state,
                ..Default::default()
            };
            assert!(c.should_continue(), "should_continue for {state:?}");
        }
    }

    #[test]
    fn should_continue_false_for_complete_and_paused() {
        let complete = RalphConfig {
            state: RalphState::Complete,
            ..Default::default()
        };
        assert!(!complete.should_continue());

        let paused = RalphConfig {
            state: RalphState::Paused,
            ..Default::default()
        };
        assert!(!paused.should_continue());
    }

    #[test]
    fn should_continue_false_when_max_iterations_reached() {
        let cfg = RalphConfig {
            state: RalphState::Reviewing,
            current_iteration: 10,
            max_iterations: 10,
            ..Default::default()
        };
        assert!(!cfg.should_continue());
    }

    #[test]
    fn pause_transitions_to_paused() {
        let mut cfg = RalphConfig::default();
        assert!(cfg.pause());
        assert_eq!(cfg.state, RalphState::Paused);
    }

    #[test]
    fn pause_from_complete_returns_false() {
        let mut cfg = RalphConfig {
            state: RalphState::Complete,
            ..Default::default()
        };
        assert!(!cfg.pause());
        assert_eq!(cfg.state, RalphState::Complete);
    }

    #[test]
    fn resume_from_paused_returns_to_gathering() {
        let mut cfg = RalphConfig {
            state: RalphState::Paused,
            current_iteration: 3,
            ..Default::default()
        };
        assert!(cfg.resume());
        assert_eq!(cfg.state, RalphState::Gathering);
        assert_eq!(cfg.current_iteration, 3); // preserved
    }

    #[test]
    fn resume_from_non_paused_returns_false() {
        for state in [
            RalphState::Gathering,
            RalphState::Planning,
            RalphState::Executing,
            RalphState::Reviewing,
            RalphState::Complete,
        ] {
            let mut cfg = RalphConfig {
                state,
                ..Default::default()
            };
            assert!(!cfg.resume(), "resume should fail from {state:?}");
            assert_eq!(cfg.state, state);
        }
    }

    #[test]
    fn serde_roundtrip() {
        let cfg = RalphConfig {
            max_iterations: 5,
            current_iteration: 2,
            auto_approve: true,
            state: RalphState::Executing,
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let restored: RalphConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.max_iterations, 5);
        assert_eq!(restored.current_iteration, 2);
        assert!(restored.auto_approve);
        assert_eq!(restored.state, RalphState::Executing);
    }

    #[test]
    fn pause_resume_full_loop_integration() {
        let mut cfg = RalphConfig {
            max_iterations: 2,
            ..Default::default()
        };

        // Advance partway through first cycle
        cfg.advance(); // Gathering -> Planning
        cfg.advance(); // Planning -> Executing

        // Pause mid-cycle
        assert!(cfg.pause());
        assert!(!cfg.should_continue());

        // Resume
        assert!(cfg.resume());
        assert!(cfg.should_continue());
        assert_eq!(cfg.state, RalphState::Gathering);
        // Iteration count is preserved — the partial cycle did not complete
        assert_eq!(cfg.current_iteration, 0);

        // Complete the first cycle (Gathering → Planning → Executing → Reviewing → Gathering)
        cfg.advance();
        cfg.advance();
        cfg.advance();
        cfg.advance();
        assert_eq!(cfg.current_iteration, 1);
        assert_eq!(cfg.state, RalphState::Gathering);

        // Second cycle: G→P→E→R→Complete (iteration reaches max_iterations=2)
        assert!(cfg.advance()); // G→P
        assert!(cfg.advance()); // P→E
        assert!(cfg.advance()); // E→R
        assert!(!cfg.advance()); // R→Complete (iteration=2 >= max=2)
        assert_eq!(cfg.state, RalphState::Complete);
        assert_eq!(cfg.current_iteration, 2);
    }
}
