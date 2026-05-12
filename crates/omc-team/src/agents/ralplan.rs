use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ralplan {
    pub title: String,
    pub phases: Vec<RalplanPhase>,
    pub current_phase: usize,
    pub status: ConsensusStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RalplanPhase {
    pub name: String,
    pub description: String,
    pub status: PhaseStatus,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhaseStatus {
    Pending,
    InProgress,
    Blocked,
    Complete,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsensusStatus {
    Draft,
    UnderReview,
    Approved,
    Rejected(String),
}

impl Ralplan {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            phases: Vec::default(),
            current_phase: 0,
            status: ConsensusStatus::Draft,
        }
    }

    pub fn add_phase(&mut self, name: impl Into<String>, description: impl Into<String>) {
        self.phases.push(RalplanPhase {
            name: name.into(),
            description: description.into(),
            status: PhaseStatus::Pending,
            blockers: Vec::default(),
        });
    }

    pub fn advance(&mut self) -> bool {
        if self.current_phase >= self.phases.len() {
            return false;
        }
        let phase = &mut self.phases[self.current_phase];
        if phase.status == PhaseStatus::Blocked {
            return false;
        }
        phase.status = PhaseStatus::Complete;
        self.current_phase += 1;
        if self.current_phase < self.phases.len() {
            self.phases[self.current_phase].status = PhaseStatus::InProgress;
        }
        true
    }

    pub fn approve(&mut self) {
        self.status = ConsensusStatus::Approved;
    }

    pub fn reject(&mut self, reason: impl Into<String>) {
        self.status = ConsensusStatus::Rejected(reason.into());
    }

    pub fn current_phase(&self) -> Option<&RalplanPhase> {
        self.phases.get(self.current_phase)
    }

    pub fn is_complete(&self) -> bool {
        self.current_phase >= self.phases.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_plan_starts_empty_in_draft() {
        let plan = Ralplan::new("Test Plan");
        assert_eq!(plan.title, "Test Plan");
        assert!(plan.phases.is_empty());
        assert_eq!(plan.current_phase, 0);
        assert_eq!(plan.status, ConsensusStatus::Draft);
    }

    #[test]
    fn add_phase_appends_pending_phase() {
        let mut plan = Ralplan::new("Test");
        plan.add_phase("Phase 1", "First phase");
        plan.add_phase("Phase 2", "Second phase");
        assert_eq!(plan.phases.len(), 2);
        assert_eq!(plan.phases[0].name, "Phase 1");
        assert_eq!(plan.phases[0].status, PhaseStatus::Pending);
        assert_eq!(plan.phases[1].name, "Phase 2");
        assert_eq!(plan.phases[1].status, PhaseStatus::Pending);
    }

    #[test]
    fn advance_completes_current_phase_and_moves_to_next() {
        let mut plan = Ralplan::new("Test");
        plan.add_phase("A", "alpha");
        plan.add_phase("B", "beta");

        assert!(plan.advance());
        assert_eq!(plan.phases[0].status, PhaseStatus::Complete);
        assert_eq!(plan.current_phase, 1);
        assert_eq!(plan.phases[1].status, PhaseStatus::InProgress);
    }

    #[test]
    fn advance_returns_false_when_all_phases_done() {
        let mut plan = Ralplan::new("Test");
        plan.add_phase("A", "alpha");
        plan.advance();
        assert!(!plan.advance());
        assert_eq!(plan.current_phase, 1);
    }

    #[test]
    fn advance_returns_false_on_empty_plan() {
        let mut plan = Ralplan::new("Empty");
        assert!(!plan.advance());
    }

    #[test]
    fn advance_blocked_phase_fails() {
        let mut plan = Ralplan::new("Test");
        plan.add_phase("A", "alpha");
        plan.phases[0].status = PhaseStatus::Blocked;
        assert!(!plan.advance());
        assert_eq!(plan.phases[0].status, PhaseStatus::Blocked);
    }

    #[test]
    fn approve_sets_approved_status() {
        let mut plan = Ralplan::new("Test");
        plan.approve();
        assert_eq!(plan.status, ConsensusStatus::Approved);
    }

    #[test]
    fn reject_stores_reason() {
        let mut plan = Ralplan::new("Test");
        plan.reject("insufficient coverage");
        assert_eq!(
            plan.status,
            ConsensusStatus::Rejected("insufficient coverage".to_string())
        );
    }

    #[test]
    fn current_phase_returns_active_phase() {
        let mut plan = Ralplan::new("Test");
        plan.add_phase("A", "alpha");
        plan.add_phase("B", "beta");

        assert_eq!(plan.current_phase().unwrap().name, "A");
        plan.advance();
        assert_eq!(plan.current_phase().unwrap().name, "B");
    }

    #[test]
    fn current_phase_returns_none_when_complete() {
        let mut plan = Ralplan::new("Test");
        plan.add_phase("A", "alpha");
        plan.advance();
        assert!(plan.current_phase().is_none());
    }

    #[test]
    fn is_complete_reflects_all_phases_done() {
        let mut plan = Ralplan::new("Test");
        plan.add_phase("A", "alpha");
        assert!(!plan.is_complete());
        plan.advance();
        assert!(plan.is_complete());
    }

    #[test]
    fn serde_roundtrip() {
        let mut plan = Ralplan::new("Serialization Test");
        plan.add_phase("Design", "architectural review");
        plan.add_phase("Build", "implementation");
        plan.advance();
        plan.approve();

        let json = serde_json::to_string(&plan).unwrap();
        let restored: Ralplan = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.title, plan.title);
        assert_eq!(restored.phases.len(), 2);
        assert_eq!(restored.current_phase, 1);
        assert_eq!(restored.status, ConsensusStatus::Approved);
        assert_eq!(restored.phases[0].status, PhaseStatus::Complete);
    }
}
