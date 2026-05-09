use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GovernanceError {
    DelegationOnly,
    PlanApprovalRequired,
    NestedTeamsNotAllowed,
    OneTeamPerSessionExceeded,
    CleanupRequiresAllInactive,
}

impl std::fmt::Display for GovernanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DelegationOnly => {
                write!(
                    f,
                    "governance requires delegation-only mode; direct task creation is blocked"
                )
            }
            Self::PlanApprovalRequired => {
                write!(f, "governance requires plan approval before execution")
            }
            Self::NestedTeamsNotAllowed => {
                write!(f, "nested team creation is disabled by governance policy")
            }
            Self::OneTeamPerSessionExceeded => {
                write!(f, "only one team per leader session is allowed")
            }
            Self::CleanupRequiresAllInactive => {
                write!(f, "team cleanup requires all workers to be inactive first")
            }
        }
    }
}

impl std::error::Error for GovernanceError {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SentinelError {
    AccessDenied(String),
    CommandBlocked(String),
    PhaseTransitionBlocked { from: String, to: String },
}

impl std::fmt::Display for SentinelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AccessDenied(msg) => write!(f, "access denied: {msg}"),
            Self::CommandBlocked(cmd) => write!(f, "command blocked: {cmd}"),
            Self::PhaseTransitionBlocked { from, to } => {
                write!(f, "phase transition blocked: {from} -> {to}")
            }
        }
    }
}

impl std::error::Error for SentinelError {}

// ---------------------------------------------------------------------------
// Governance configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamGovernance {
    pub delegation_only: bool,
    pub plan_approval_required: bool,
    pub nested_teams_allowed: bool,
    pub one_team_per_leader_session: bool,
    pub cleanup_requires_all_workers_inactive: bool,
}

impl Default for TeamGovernance {
    fn default() -> Self {
        Self {
            delegation_only: false,
            plan_approval_required: false,
            nested_teams_allowed: false,
            one_team_per_leader_session: true,
            cleanup_requires_all_workers_inactive: true,
        }
    }
}

impl TeamGovernance {
    pub fn validate_team_creation(&self, existing_teams: usize) -> Result<(), GovernanceError> {
        if self.one_team_per_leader_session && existing_teams >= 1 {
            return Err(GovernanceError::OneTeamPerSessionExceeded);
        }
        Ok(())
    }

    pub fn validate_delegation(&self, task: &str) -> Result<(), GovernanceError> {
        if self.delegation_only && !task.contains("delegate") {
            return Err(GovernanceError::DelegationOnly);
        }
        Ok(())
    }

    pub fn validate_nested_team(&self) -> Result<(), GovernanceError> {
        if !self.nested_teams_allowed {
            return Err(GovernanceError::NestedTeamsNotAllowed);
        }
        Ok(())
    }

    pub fn validate_cleanup(&self, active_workers: usize) -> Result<(), GovernanceError> {
        if self.cleanup_requires_all_workers_inactive && active_workers > 0 {
            return Err(GovernanceError::CleanupRequiresAllInactive);
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Transport policy
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportPolicy {
    pub display_mode: DisplayMode,
    pub worker_launch_mode: WorkerLaunchMode,
    pub dispatch_mode: DispatchMode,
    pub dispatch_ack_timeout_ms: u64,
}

impl Default for TransportPolicy {
    fn default() -> Self {
        Self {
            display_mode: DisplayMode::SplitPane,
            worker_launch_mode: WorkerLaunchMode::Interactive,
            dispatch_mode: DispatchMode::HookPreferred,
            dispatch_ack_timeout_ms: 5000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DisplayMode {
    SplitPane,
    Background,
    Headless,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum WorkerLaunchMode {
    Interactive,
    Background,
    Detached,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DispatchMode {
    HookPreferred,
    DirectOnly,
    HookOnly,
}

// ---------------------------------------------------------------------------
// Worker permissions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerPermissions {
    pub allowed_paths: Vec<String>,
    pub denied_paths: Vec<String>,
    pub allowed_commands: Vec<String>,
    pub max_file_size: usize,
}

impl Default for WorkerPermissions {
    fn default() -> Self {
        Self {
            allowed_paths: vec!["src/**".to_string()],
            denied_paths: vec![
                ".env".to_string(),
                ".env.*".to_string(),
                "*.key".to_string(),
                "*.pem".to_string(),
            ],
            allowed_commands: vec!["cargo".to_string(), "git".to_string(), "npm".to_string()],
            max_file_size: 1_048_576, // 1 MiB
        }
    }
}

// ---------------------------------------------------------------------------
// Enforcement mode
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EnforcementMode {
    Off,
    Audit,
    Enforce,
}

// ---------------------------------------------------------------------------
// File operation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum FileOperation {
    Read,
    Write,
    Delete,
}

// ---------------------------------------------------------------------------
// Audit log
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub timestamp: String,
    pub actor: String,
    pub action: String,
    pub target: String,
    pub outcome: AuditOutcome,
    pub details: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AuditOutcome {
    Allowed,
    Blocked,
    Warned,
}

pub struct AuditLog {
    entries: Vec<AuditEntry>,
    log_path: Option<PathBuf>,
}

impl Default for AuditLog {
    fn default() -> Self {
        Self {}
    }
}

impl AuditLog {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            log_path: None,
        }
    }

    pub fn with_file_path(path: PathBuf) -> Self {
        Self {
            entries: Vec::new(),
            log_path: Some(path),
        }
    }

    pub fn record(&mut self, entry: AuditEntry) {
        self.entries.push(entry);
    }

    pub fn entries(&self) -> &[AuditEntry] {
        &self.entries
    }

    pub fn flush(&self) -> Result<(), std::io::Error> {
        if let Some(path) = &self.log_path {
            let mut lines = Vec::with_capacity(self.entries.len());
            for entry in &self.entries {
                let line = serde_json::to_string(entry)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                lines.push(line);
            }
            fs::write(path, lines.join("\n") + "\n")?;
        }
        Ok(())
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

// ---------------------------------------------------------------------------
// Sentinel gate
// ---------------------------------------------------------------------------

pub struct SentinelGate {
    governance: TeamGovernance,
    permissions: WorkerPermissions,
    enforcement: EnforcementMode,
    audit_log: Mutex<AuditLog>,
}

impl SentinelGate {
    pub fn new(
        governance: TeamGovernance,
        permissions: WorkerPermissions,
        enforcement: EnforcementMode,
    ) -> Self {
        Self {
            governance,
            permissions,
            enforcement,
            audit_log: Mutex::new(AuditLog::default()),
        }
    }

    pub fn audit_log(&self) -> std::sync::MutexGuard<'_, AuditLog> {
        self.audit_log.lock().unwrap()
    }

    pub fn audit_log_mut(&self) -> std::sync::MutexGuard<'_, AuditLog> {
        self.audit_log.lock().unwrap()
    }

    pub fn check_task_dispatch(&self, task: &str, worker: &str) -> Result<(), SentinelError> {
        if self.enforcement == EnforcementMode::Off {
            return Ok(());
        }

        let delegation_check = self.governance.validate_delegation(task);
        let outcome = if delegation_check.is_ok() {
            AuditOutcome::Allowed
        } else if self.enforcement == EnforcementMode::Audit {
            AuditOutcome::Warned
        } else {
            AuditOutcome::Blocked
        };

        // SAFETY: audit_log is borrowed mutably only via audit_log_mut(), which
        // is not reachable while check_task_dispatch holds &self.
        self.log_entry(AuditEntry {
            timestamp: chrono::Utc::now().to_rfc3339(),
            actor: worker.to_string(),
            action: "task_dispatch".to_string(),
            target: task.to_string(),
            outcome: outcome.clone(),
            details: delegation_check.as_ref().map_err(|e| e.to_string()),
        });

        match (delegation_check, &self.enforcement) {
            (Ok(_), _) => Ok(()),
            (Err(_), EnforcementMode::Audit) => Ok(()),
            (Err(e), EnforcementMode::Enforce) => Err(SentinelError::AccessDenied(e.to_string())),
            (Err(_), EnforcementMode::Off) => Ok(()),
        }
    }

    pub fn check_file_access(
        &self,
        path: &str,
        operation: FileOperation,
    ) -> Result<(), SentinelError> {
        if self.enforcement == EnforcementMode::Off {
            return Ok(());
        }

        let is_denied = self
            .permissions
            .denied_paths
            .iter()
            .any(|pattern| glob_match(pattern, path));

        let is_allowed = self
            .permissions
            .allowed_paths
            .iter()
            .any(|pattern| glob_match(pattern, path));

        let blocked = if is_denied {
            Some(format!("path {path} matches denied pattern"))
        } else if operation == FileOperation::Delete {
            Some(format!("delete operation on {path} is always blocked"))
        } else if !is_allowed {
            Some(format!("path {path} is not in allowed paths"))
        } else {
            None
        };

        let outcome = match (&blocked, &self.enforcement) {
            (None, _) => AuditOutcome::Allowed,
            (Some(_), EnforcementMode::Audit) => AuditOutcome::Warned,
            (Some(_), _) => AuditOutcome::Blocked,
        };

        self.log_entry(AuditEntry {
            timestamp: chrono::Utc::now().to_rfc3339(),
            actor: String::default(),
            action: format!("file_{operation:?}").to_lowercase(),
            target: path.to_string(),
            outcome: outcome.clone(),
            details: blocked.clone(),
        });

        match (blocked, &self.enforcement) {
            (None, _) => Ok(()),
            (Some(_), EnforcementMode::Audit) => Ok(()),
            (Some(msg), EnforcementMode::Enforce) => Err(SentinelError::AccessDenied(msg)),
            (Some(_), EnforcementMode::Off) => Ok(()),
        }
    }

    pub fn check_command_execution(&self, command: &str) -> Result<(), SentinelError> {
        if self.enforcement == EnforcementMode::Off {
            return Ok(());
        }

        let first_word = command.split_whitespace().next().unwrap_or("");
        let is_allowed = self
            .permissions
            .allowed_commands
            .iter()
            .any(|prefix| first_word == prefix);

        let blocked = if !is_allowed {
            Some(format!("command '{first_word}' is not in allowed commands"))
        } else {
            None
        };

        let outcome = match (&blocked, &self.enforcement) {
            (None, _) => AuditOutcome::Allowed,
            (Some(_), EnforcementMode::Audit) => AuditOutcome::Warned,
            (Some(_), _) => AuditOutcome::Blocked,
        };

        self.log_entry(AuditEntry {
            timestamp: chrono::Utc::now().to_rfc3339(),
            actor: String::default(),
            action: "command_execution".to_string(),
            target: command.to_string(),
            outcome: outcome.clone(),
            details: blocked.clone(),
        });

        match (blocked, &self.enforcement) {
            (None, _) => Ok(()),
            (Some(_), EnforcementMode::Audit) => Ok(()),
            (Some(msg), EnforcementMode::Enforce) => Err(SentinelError::CommandBlocked(msg)),
            (Some(_), EnforcementMode::Off) => Ok(()),
        }
    }

    pub fn check_phase_transition(&self, from: &str, to: &str) -> Result<(), SentinelError> {
        if self.enforcement == EnforcementMode::Off {
            return Ok(());
        }

        let valid = is_valid_phase_transition(from, to);

        let outcome = if valid {
            AuditOutcome::Allowed
        } else if self.enforcement == EnforcementMode::Audit {
            AuditOutcome::Warned
        } else {
            AuditOutcome::Blocked
        };

        self.log_entry(AuditEntry {
            timestamp: chrono::Utc::now().to_rfc3339(),
            actor: String::default(),
            action: "phase_transition".to_string(),
            target: format!("{from}->{to}"),
            outcome: outcome.clone(),
            details: if valid {
                None
            } else {
                Some(format!("invalid transition: {from} -> {to}"))
            },
        });

        if valid || self.enforcement == EnforcementMode::Audit {
            Ok(())
        } else {
            Err(SentinelError::PhaseTransitionBlocked {
                from: from.to_string(),
                to: to.to_string(),
            })
        }
    }

    fn log_entry(&self, entry: AuditEntry) {
        self.audit_log.lock().unwrap().record(entry);
    }
}

/// Simple glob-style match supporting `*` and `**` patterns.
fn glob_match(pattern: &str, value: &str) -> bool {
    // Normalize: ** matches any path segment, * matches within a segment
    let pattern_parts: Vec<&str> = pattern.split('/').collect();
    let value_parts: Vec<&str> = value.split('/').collect();
    glob_match_parts(&pattern_parts, &value_parts)
}

fn glob_match_parts(pattern: &[&str], value: &[&str]) -> bool {
    if pattern.is_empty() {
        return value.is_empty();
    }
    if pattern[0] == "**" {
        // ** matches zero or more segments
        if glob_match_parts(&pattern[1..], value) {
            return true;
        }
        if !value.is_empty() && glob_match_parts(pattern, &value[1..]) {
            return true;
        }
        return false;
    }
    if value.is_empty() {
        return false;
    }
    if simple_match(pattern[0], value[0]) {
        glob_match_parts(&pattern[1..], &value[1..])
    } else {
        false
    }
}

fn simple_match(pattern: &str, value: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    // Support *.ext pattern
    if let Some(suffix) = pattern.strip_prefix("*.") {
        return value.ends_with(&format!(".{suffix}"));
    }
    pattern == value
}

fn is_valid_phase_transition(from: &str, to: &str) -> bool {
    matches!(
        (from, to),
        ("pending", "in_progress")
            | ("pending", "cancelled")
            | ("in_progress", "completed")
            | ("in_progress", "blocked")
            | ("in_progress", "cancelled")
            | ("blocked", "in_progress")
            | ("blocked", "cancelled")
            | ("completed", "archived")
            | ("review", "approved")
            | ("review", "rejected")
            | ("rejected", "in_progress")
            | ("approved", "completed")
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Governance defaults --

    #[test]
    fn default_governance_values() {
        let g = TeamGovernance::default();
        assert!(!g.delegation_only);
        assert!(!g.plan_approval_required);
        assert!(!g.nested_teams_allowed);
        assert!(g.one_team_per_leader_session);
        assert!(g.cleanup_requires_all_workers_inactive);
    }

    #[test]
    fn default_transport_policy() {
        let tp = TransportPolicy::default();
        assert_eq!(tp.display_mode, DisplayMode::SplitPane);
        assert_eq!(tp.worker_launch_mode, WorkerLaunchMode::Interactive);
        assert_eq!(tp.dispatch_mode, DispatchMode::HookPreferred);
        assert_eq!(tp.dispatch_ack_timeout_ms, 5000);
    }

    #[test]
    fn default_worker_permissions() {
        let wp = WorkerPermissions::default();
        assert!(wp.allowed_paths.contains(&"src/**".to_string()));
        assert!(wp.denied_paths.contains(&".env".to_string()));
        assert!(wp.allowed_commands.contains(&"cargo".to_string()));
        assert_eq!(wp.max_file_size, 1_048_576);
    }

    // -- Team creation validation --

    #[test]
    fn one_team_per_session_blocks_second() {
        let mut g = TeamGovernance::default();
        g.one_team_per_leader_session = true;
        assert!(g.validate_team_creation(0).is_ok());
        assert!(g.validate_team_creation(1).is_err());
        assert!(g.validate_team_creation(2).is_err());
    }

    #[test]
    fn multiple_teams_allowed_when_disabled() {
        let mut g = TeamGovernance::default();
        g.one_team_per_leader_session = false;
        assert!(g.validate_team_creation(0).is_ok());
        assert!(g.validate_team_creation(5).is_ok());
    }

    // -- Delegation --

    #[test]
    fn delegation_only_blocks_non_delegation() {
        let mut g = TeamGovernance::default();
        g.delegation_only = true;
        assert!(g.validate_delegation("implement feature X").is_err());
        assert!(g.validate_delegation("delegate task to worker").is_ok());
    }

    #[test]
    fn delegation_allowed_by_default() {
        let g = TeamGovernance::default();
        assert!(g.validate_delegation("anything").is_ok());
    }

    // -- Nested teams --

    #[test]
    fn nested_teams_blocked_when_disabled() {
        let g = TeamGovernance::default(); // nested_teams_allowed = false
        assert!(g.validate_nested_team().is_err());
    }

    #[test]
    fn nested_teams_allowed_when_enabled() {
        let mut g = TeamGovernance::default();
        g.nested_teams_allowed = true;
        assert!(g.validate_nested_team().is_ok());
    }

    // -- Cleanup --

    #[test]
    fn cleanup_blocked_with_active_workers() {
        let g = TeamGovernance::default(); // cleanup_requires_all_workers_inactive = true
        assert!(g.validate_cleanup(0).is_ok());
        assert!(g.validate_cleanup(1).is_err());
        assert!(g.validate_cleanup(3).is_err());
    }

    #[test]
    fn cleanup_allowed_when_not_required() {
        let mut g = TeamGovernance::default();
        g.cleanup_requires_all_workers_inactive = false;
        assert!(g.validate_cleanup(5).is_ok());
    }

    // -- Sentinel: file access --

    #[test]
    fn sentinel_allowed_path_passes_in_enforce() {
        let gate = sentinel_gate_enforce();
        assert!(
            gate.check_file_access("src/main.rs", FileOperation::Read)
                .is_ok()
        );
        assert!(
            gate.check_file_access("src/lib.rs", FileOperation::Write)
                .is_ok()
        );
    }

    #[test]
    fn sentinel_denied_path_blocks_in_enforce() {
        let gate = sentinel_gate_enforce();
        assert!(gate.check_file_access(".env", FileOperation::Read).is_err());
        assert!(
            gate.check_file_access("secret.key", FileOperation::Read)
                .is_err()
        );
    }

    #[test]
    fn sentinel_delete_always_blocked_in_enforce() {
        let gate = sentinel_gate_enforce();
        assert!(
            gate.check_file_access("src/main.rs", FileOperation::Delete)
                .is_err()
        );
    }

    #[test]
    fn sentinel_unlisted_path_blocks_in_enforce() {
        let gate = sentinel_gate_enforce();
        assert!(
            gate.check_file_access("random/file.txt", FileOperation::Read)
                .is_err()
        );
    }

    // -- Sentinel: command execution --

    #[test]
    fn sentinel_allowed_command_passes() {
        let gate = sentinel_gate_enforce();
        assert!(gate.check_command_execution("cargo test").is_ok());
        assert!(gate.check_command_execution("git status").is_ok());
    }

    #[test]
    fn sentinel_unknown_command_blocks_in_enforce() {
        let gate = sentinel_gate_enforce();
        assert!(gate.check_command_execution("rm -rf /").is_err());
        assert!(
            gate.check_command_execution("curl http://evil.com")
                .is_err()
        );
    }

    // -- Sentinel: audit mode --

    #[test]
    fn sentinel_audit_mode_logs_but_does_not_block() {
        let gate = sentinel_gate_audit();
        // Denied path should warn but not block
        assert!(gate.check_file_access(".env", FileOperation::Read).is_ok());
        // Unknown command should warn but not block
        assert!(gate.check_command_execution("rm -rf /").is_ok());
        // Entries are recorded
        assert_eq!(gate.audit_log().entries().len(), 2);

        // Delegation-only violation should warn but not block
        let mut g = TeamGovernance::default();
        g.delegation_only = true;
        let gate2 = SentinelGate::new(g, WorkerPermissions::default(), EnforcementMode::Audit);
        assert!(
            gate2
                .check_task_dispatch("implement feature", "worker-1")
                .is_ok()
        );
        assert_eq!(gate2.audit_log().entries().len(), 1);
        assert_eq!(gate2.audit_log().entries()[0].outcome, AuditOutcome::Warned);
    }

    // -- Sentinel: off mode --

    #[test]
    fn sentinel_off_mode_allows_everything() {
        let gate = SentinelGate::new(
            TeamGovernance::default(),
            WorkerPermissions::default(),
            EnforcementMode::Off,
        );
        assert!(gate.check_file_access(".env", FileOperation::Read).is_ok());
        assert!(gate.check_command_execution("anything").is_ok());
        assert!(gate.check_task_dispatch("any task", "worker-1").is_ok());
        assert!(gate.check_phase_transition("completed", "pending").is_ok());
    }

    // -- Sentinel: phase transitions --

    #[test]
    fn sentinel_valid_phase_transition_passes() {
        let gate = sentinel_gate_enforce();
        assert!(
            gate.check_phase_transition("pending", "in_progress")
                .is_ok()
        );
        assert!(
            gate.check_phase_transition("in_progress", "completed")
                .is_ok()
        );
        assert!(
            gate.check_phase_transition("blocked", "in_progress")
                .is_ok()
        );
        assert!(gate.check_phase_transition("review", "approved").is_ok());
        assert!(gate.check_phase_transition("approved", "completed").is_ok());
    }

    #[test]
    fn sentinel_invalid_phase_transition_blocks() {
        let gate = sentinel_gate_enforce();
        assert!(gate.check_phase_transition("completed", "pending").is_err());
        assert!(
            gate.check_phase_transition("archived", "in_progress")
                .is_err()
        );
        assert!(
            gate.check_phase_transition("completed", "in_progress")
                .is_err()
        );
    }

    // -- Audit log --

    #[test]
    fn audit_log_record_and_entries() {
        let mut log = AuditLog::default();
        assert!(log.entries().is_empty());

        log.record(AuditEntry {
            timestamp: "2026-01-01T00:00:00Z".to_string(),
            actor: "worker-1".to_string(),
            action: "task_dispatch".to_string(),
            target: "task-1".to_string(),
            outcome: AuditOutcome::Allowed,
            details: None,
        });
        assert_eq!(log.entries().len(), 1);
        assert_eq!(log.entries()[0].actor, "worker-1");
        assert_eq!(log.entries()[0].outcome, AuditOutcome::Allowed);
    }

    #[test]
    fn audit_log_flush_to_file() {
        let dir = std::env::temp_dir().join(format!("omc-audit-test-{}", crate::unix_timestamp()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("audit.jsonl");

        let mut log = AuditLog::with_file_path(path.clone());
        log.record(AuditEntry {
            timestamp: "2026-01-01T00:00:00Z".to_string(),
            actor: "agent-1".to_string(),
            action: "file_write".to_string(),
            target: "src/main.rs".to_string(),
            outcome: AuditOutcome::Allowed,
            details: None,
        });
        log.record(AuditEntry {
            timestamp: "2026-01-01T00:00:01Z".to_string(),
            actor: "agent-2".to_string(),
            action: "command_execution".to_string(),
            target: "rm -rf /".to_string(),
            outcome: AuditOutcome::Blocked,
            details: Some("command not allowed".to_string()),
        });

        log.flush().unwrap();

        let content = fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.trim().lines().collect();
        assert_eq!(lines.len(), 2);

        let entry: AuditEntry = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(entry.action, "file_write");
        assert_eq!(entry.outcome, AuditOutcome::Allowed);

        let entry: AuditEntry = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(entry.outcome, AuditOutcome::Blocked);
        assert!(entry.details.is_some());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn audit_log_clear() {
        let mut log = AuditLog::default();
        log.record(AuditEntry {
            timestamp: String::default(),
            actor: String::default(),
            action: String::default(),
            target: String::default(),
            outcome: AuditOutcome::Allowed,
            details: None,
        });
        assert_eq!(log.entries().len(), 1);
        log.clear();
        assert!(log.entries().is_empty());
    }

    #[test]
    fn audit_log_flush_without_path_is_noop() {
        let log = AuditLog::default();
        assert!(log.flush().is_ok());
    }

    // -- Glob matching --

    #[test]
    fn glob_match_simple() {
        assert!(glob_match("src/**", "src/main.rs"));
        assert!(glob_match("src/**", "src/lib/governance.rs"));
        assert!(!glob_match("src/**", "tests/main.rs"));
    }

    #[test]
    fn glob_match_extension() {
        assert!(glob_match("*.env", ".env"));
        assert!(glob_match("*.key", "secret.key"));
        assert!(glob_match("*.pem", "cert.pem"));
        assert!(!glob_match("*.key", "main.rs"));
    }

    #[test]
    fn glob_match_exact() {
        assert!(glob_match(".env", ".env"));
        assert!(!glob_match(".env", ".env.local"));
    }

    // -- Error display --

    #[test]
    fn governance_error_display() {
        let err = GovernanceError::DelegationOnly;
        assert!(!err.to_string().is_empty());
        let err = SentinelError::CommandBlocked("rm".to_string());
        assert!(err.to_string().contains("rm"));
    }

    // -- Serialization roundtrip --

    #[test]
    fn governance_serialization_roundtrip() {
        let g = TeamGovernance::default();
        let json = serde_json::to_string(&g).unwrap();
        let restored: TeamGovernance = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.delegation_only, g.delegation_only);
        assert_eq!(
            restored.one_team_per_leader_session,
            g.one_team_per_leader_session
        );
    }

    #[test]
    fn audit_entry_serialization_roundtrip() {
        let entry = AuditEntry {
            timestamp: "2026-01-01T00:00:00Z".to_string(),
            actor: "agent-1".to_string(),
            action: "task_dispatch".to_string(),
            target: "task-1".to_string(),
            outcome: AuditOutcome::Warned,
            details: Some("test".to_string()),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let restored: AuditEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.outcome, AuditOutcome::Warned);
        assert_eq!(restored.details, Some("test".to_string()));
    }

    // -- Helpers --

    fn sentinel_gate_enforce() -> SentinelGate {
        SentinelGate::new(
            TeamGovernance::default(),
            WorkerPermissions::default(),
            EnforcementMode::Enforce,
        )
    }

    fn sentinel_gate_audit() -> SentinelGate {
        SentinelGate::new(
            TeamGovernance::default(),
            WorkerPermissions::default(),
            EnforcementMode::Audit,
        )
    }
}
