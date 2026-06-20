use std::collections::HashSet;
use std::fmt;
use std::time::Duration;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Value types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(pub String);

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Capability(pub String);

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Deliverables {
    pub files: Vec<String>,
    pub summary: String,
    pub tests_passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentError {
    Timeout,
    Crash { message: String },
    DependencyFailed { agent: AgentId },
    MaxRetriesExceeded,
}

impl fmt::Display for AgentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Timeout => write!(f, "agent timed out"),
            Self::Crash { message } => write!(f, "agent crashed: {message}"),
            Self::DependencyFailed { agent } => {
                write!(f, "dependency {agent} failed")
            }
            Self::MaxRetriesExceeded => write!(f, "max retries exceeded"),
        }
    }
}

impl std::error::Error for AgentError {}

// ---------------------------------------------------------------------------
// State machine
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum AgentState {
    Spawning,
    Idle,
    TaskReceived,
    InProgress { progress: f64 },
    Blocked { waiting_for: Vec<AgentId> },
    Complete { deliverables: Deliverables },
    Failed { error: AgentError, retry_count: u8 },
    ShuttingDown,
}

/// Errors produced by invalid state transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionError {
    InvalidTransition { from: String, to: String },
}

impl fmt::Display for TransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTransition { from, to } => {
                write!(f, "invalid transition: {from} -> {to}")
            }
        }
    }
}

impl std::error::Error for TransitionError {}

// ---------------------------------------------------------------------------
// Lifecycle manager
// ---------------------------------------------------------------------------

pub struct AgentLifecycle {
    pub id: AgentId,
    pub state: AgentState,
    pub capabilities: HashSet<Capability>,
    pub heartbeat_interval: Duration,
    pub stale_threshold: Duration,
    pub max_retries: u8,
    last_heartbeat: Option<std::time::Instant>,
    /// Tracks cumulative retry count across state transitions.
    retry_count: u8,
}

impl AgentLifecycle {
    /// Default heartbeat interval (30 seconds).
    pub const DEFAULT_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
    /// Default stale threshold (120 seconds).
    pub const DEFAULT_STALE_THRESHOLD: Duration = Duration::from_secs(120);
    /// Default max retries before giving up.
    pub const DEFAULT_MAX_RETRIES: u8 = 3;

    pub fn new(id: AgentId, capabilities: HashSet<Capability>) -> Self {
        Self {
            id,
            state: AgentState::Spawning,
            capabilities,
            heartbeat_interval: Self::DEFAULT_HEARTBEAT_INTERVAL,
            stale_threshold: Self::DEFAULT_STALE_THRESHOLD,
            max_retries: Self::DEFAULT_MAX_RETRIES,
            last_heartbeat: None,
            retry_count: 0,
        }
    }

    pub fn with_heartbeat_interval(mut self, interval: Duration) -> Self {
        self.heartbeat_interval = interval;
        self
    }

    pub fn with_stale_threshold(mut self, threshold: Duration) -> Self {
        self.stale_threshold = threshold;
        self
    }

    pub fn with_max_retries(mut self, max: u8) -> Self {
        self.max_retries = max;
        self
    }

    // -- transitions --------------------------------------------------------

    /// Spawning -> Idle
    pub fn mark_ready(&mut self) -> Result<(), TransitionError> {
        self.transition(AgentState::Idle)
    }

    /// Idle -> TaskReceived
    pub fn receive_task(&mut self) -> Result<(), TransitionError> {
        self.transition(AgentState::TaskReceived)
    }

    /// TaskReceived -> InProgress (also valid from Blocked on unblock)
    pub fn start_work(&mut self) -> Result<(), TransitionError> {
        self.transition(AgentState::InProgress { progress: 0.0 })
    }

    /// InProgress -> Blocked
    pub fn block(&mut self, waiting_for: Vec<AgentId>) -> Result<(), TransitionError> {
        self.transition(AgentState::Blocked { waiting_for })
    }

    /// Blocked -> InProgress
    pub fn unblock(&mut self) -> Result<(), TransitionError> {
        self.transition(AgentState::InProgress { progress: 0.0 })
    }

    /// InProgress -> InProgress (progress update, clamped to 0.0..=1.0)
    pub fn update_progress(&mut self, progress: f64) -> Result<(), TransitionError> {
        match &self.state {
            AgentState::InProgress { .. } => {
                self.state = AgentState::InProgress {
                    progress: progress.clamp(0.0, 1.0),
                };
                Ok(())
            }
            other => Err(TransitionError::InvalidTransition {
                from: state_name(other).to_string(),
                to: "InProgress(progress update)".to_string(),
            }),
        }
    }

    /// InProgress -> Complete
    pub fn complete(&mut self, deliverables: Deliverables) -> Result<(), TransitionError> {
        self.transition(AgentState::Complete { deliverables })
    }

    /// Any active state -> Failed.
    pub fn fail(&mut self, error: AgentError) -> Result<(), TransitionError> {
        self.state = AgentState::Failed {
            error,
            retry_count: self.retry_count,
        };
        Ok(())
    }

    /// Failed (under retry limit) -> TaskReceived. Increments the retry counter.
    pub fn retry(&mut self) -> Result<(), TransitionError> {
        match &self.state {
            AgentState::Failed { .. } if self.retry_count < self.max_retries => {
                self.retry_count += 1;
                self.state = AgentState::TaskReceived;
                Ok(())
            }
            AgentState::Failed { .. } => Err(TransitionError::InvalidTransition {
                from: "Failed(max retries)".to_string(),
                to: "TaskReceived".to_string(),
            }),
            other => Err(TransitionError::InvalidTransition {
                from: state_name(other).to_string(),
                to: "TaskReceived".to_string(),
            }),
        }
    }

    /// Any non-terminal state -> ShuttingDown.
    pub fn shutdown(&mut self) -> Result<(), TransitionError> {
        if self.is_terminal() {
            return Err(TransitionError::InvalidTransition {
                from: state_name(&self.state).to_string(),
                to: "ShuttingDown".to_string(),
            });
        }
        self.state = AgentState::ShuttingDown;
        Ok(())
    }

    // -- heartbeat ----------------------------------------------------------

    pub fn record_heartbeat(&mut self) {
        self.last_heartbeat = Some(std::time::Instant::now());
    }

    pub fn is_stale(&self) -> bool {
        match self.last_heartbeat {
            Some(ts) => ts.elapsed() > self.stale_threshold,
            None => false,
        }
    }

    pub fn heartbeat_overdue(&self) -> bool {
        match self.last_heartbeat {
            Some(ts) => ts.elapsed() > self.heartbeat_interval,
            None => false,
        }
    }

    // -- queries ------------------------------------------------------------

    pub fn is_terminal(&self) -> bool {
        matches!(
            self.state,
            AgentState::Complete { .. } | AgentState::Failed { .. } | AgentState::ShuttingDown
        )
    }

    pub fn can_retry(&self) -> bool {
        matches!(&self.state, AgentState::Failed { .. }) && self.retry_count < self.max_retries
    }

    pub fn state_name(&self) -> &'static str {
        state_name(&self.state)
    }

    // -- internals ----------------------------------------------------------

    fn transition(&mut self, target: AgentState) -> Result<(), TransitionError> {
        let from = &self.state;
        let allowed = is_valid_transition(from, &target);
        if allowed {
            self.state = target;
            Ok(())
        } else {
            Err(TransitionError::InvalidTransition {
                from: state_name(from).to_string(),
                to: state_name_owned(&target),
            })
        }
    }
}

// ---------------------------------------------------------------------------
// Transition table
// ---------------------------------------------------------------------------

fn is_valid_transition(from: &AgentState, to: &AgentState) -> bool {
    use AgentState::*;
    matches!(
        (from, to),
        (Spawning, Idle)
            | (Idle, TaskReceived)
            | (Idle, ShuttingDown)
            | (TaskReceived, InProgress { .. })
            | (TaskReceived, Failed { .. })
            | (InProgress { .. }, Blocked { .. })
            | (InProgress { .. }, InProgress { .. })
            | (InProgress { .. }, Complete { .. })
            | (InProgress { .. }, Failed { .. })
            | (InProgress { .. }, ShuttingDown)
            | (Blocked { .. }, InProgress { .. })
            | (Blocked { .. }, Failed { .. })
            | (Blocked { .. }, ShuttingDown)
            | (Failed { .. }, TaskReceived)
            | (Failed { .. }, ShuttingDown)
    )
}

fn state_name(s: &AgentState) -> &'static str {
    match s {
        AgentState::Spawning => "Spawning",
        AgentState::Idle => "Idle",
        AgentState::TaskReceived => "TaskReceived",
        AgentState::InProgress { .. } => "InProgress",
        AgentState::Blocked { .. } => "Blocked",
        AgentState::Complete { .. } => "Complete",
        AgentState::Failed { .. } => "Failed",
        AgentState::ShuttingDown => "ShuttingDown",
    }
}

fn state_name_owned(s: &AgentState) -> String {
    state_name(s).to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_lifecycle(id: &str) -> AgentLifecycle {
        AgentLifecycle::new(
            AgentId(id.to_string()),
            HashSet::from([Capability("code".to_string())]),
        )
    }

    #[test]
    fn happy_path_spawning_to_complete() {
        let mut a = make_lifecycle("worker-1");
        assert_eq!(a.state_name(), "Spawning");

        a.mark_ready().unwrap();
        assert_eq!(a.state_name(), "Idle");

        a.receive_task().unwrap();
        assert_eq!(a.state_name(), "TaskReceived");

        a.start_work().unwrap();
        assert_eq!(a.state_name(), "InProgress");

        a.update_progress(0.5).unwrap();
        match &a.state {
            AgentState::InProgress { progress } => assert!((progress - 0.5).abs() < f64::EPSILON),
            _ => panic!("expected InProgress"),
        }

        a.complete(Deliverables {
            files: vec!["src/lib.rs".to_string()],
            summary: "done".to_string(),
            tests_passed: true,
        })
        .unwrap();
        assert!(a.is_terminal());
    }

    #[test]
    fn blocked_and_unblock() {
        let mut a = make_lifecycle("worker-2");
        a.mark_ready().unwrap();
        a.receive_task().unwrap();
        a.start_work().unwrap();

        a.block(vec![AgentId("dep-1".to_string())]).unwrap();
        assert_eq!(a.state_name(), "Blocked");

        a.unblock().unwrap();
        assert_eq!(a.state_name(), "InProgress");
    }

    #[test]
    fn retry_under_limit() {
        let mut a = make_lifecycle("worker-3");
        a.mark_ready().unwrap();
        a.receive_task().unwrap();
        a.start_work().unwrap();

        a.fail(AgentError::Timeout).unwrap();
        assert_eq!(a.state_name(), "Failed");
        assert!(a.can_retry());

        a.retry().unwrap();
        assert_eq!(a.state_name(), "TaskReceived");
    }

    #[test]
    fn retry_exhausted() {
        let mut a = AgentLifecycle::new(AgentId("fragile".to_string()), HashSet::default())
            .with_max_retries(1);

        a.mark_ready().unwrap();
        a.receive_task().unwrap();
        a.start_work().unwrap();

        a.fail(AgentError::Crash {
            message: "segfault".to_string(),
        })
        .unwrap();
        assert!(a.can_retry());
        a.retry().unwrap();

        a.start_work().unwrap();
        a.fail(AgentError::Timeout).unwrap();
        assert!(!a.can_retry());
        assert!(a.retry().is_err());
    }

    #[test]
    fn invalid_transition_rejected() {
        let mut a = make_lifecycle("worker-4");
        assert!(a.receive_task().is_err());
        assert!(a.start_work().is_err());
        assert!(a.complete(Deliverables::default()).is_err());
    }

    #[test]
    fn shutdown_from_active_state() {
        let mut a = make_lifecycle("worker-5");
        a.mark_ready().unwrap();
        a.shutdown().unwrap();
        assert_eq!(a.state_name(), "ShuttingDown");
        assert!(a.is_terminal());
    }

    #[test]
    fn cannot_shutdown_after_complete() {
        let mut a = make_lifecycle("worker-6");
        a.mark_ready().unwrap();
        a.receive_task().unwrap();
        a.start_work().unwrap();
        a.complete(Deliverables::default()).unwrap();
        assert!(a.shutdown().is_err());
    }

    #[test]
    fn progress_clamped() {
        let mut a = make_lifecycle("worker-7");
        a.mark_ready().unwrap();
        a.receive_task().unwrap();
        a.start_work().unwrap();

        a.update_progress(2.0).unwrap();
        match &a.state {
            AgentState::InProgress { progress } => assert!((progress - 1.0).abs() < f64::EPSILON),
            _ => panic!("expected InProgress"),
        }

        a.update_progress(-0.5).unwrap();
        match &a.state {
            AgentState::InProgress { progress } => assert!((progress - 0.0).abs() < f64::EPSILON),
            _ => panic!("expected InProgress"),
        }
    }

    #[test]
    fn heartbeat_detection() {
        let mut a = AgentLifecycle::new(AgentId("hb-test".to_string()), HashSet::default())
            .with_heartbeat_interval(Duration::from_millis(50))
            .with_stale_threshold(Duration::from_millis(200));

        assert!(!a.is_stale());
        assert!(!a.heartbeat_overdue());

        a.record_heartbeat();
        assert!(!a.is_stale());
        assert!(!a.heartbeat_overdue());

        std::thread::sleep(Duration::from_millis(80));
        assert!(a.heartbeat_overdue());
        assert!(!a.is_stale());
    }

    #[test]
    fn serialization_roundtrip() {
        let mut a = make_lifecycle("serde-test");
        a.mark_ready().unwrap();
        a.receive_task().unwrap();
        a.start_work().unwrap();
        a.update_progress(0.42).unwrap();

        let json = serde_json::to_string(&a.state).unwrap();
        let parsed: AgentState = serde_json::from_str(&json).unwrap();
        match parsed {
            AgentState::InProgress { progress } => {
                assert!((progress - 0.42).abs() < f64::EPSILON)
            }
            _ => panic!("expected InProgress after roundtrip"),
        }
    }
}
