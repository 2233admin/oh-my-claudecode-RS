use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Severity {
    Block,
    Warn,
    Log,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForbiddenAction {
    pub code: String,
    pub description: String,
    pub severity: Severity,
    pub roles: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub timestamp: String,
    pub agent_id: String,
    pub action: String,
    pub rule_code: String,
    pub severity: Severity,
    pub outcome: Outcome,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Outcome {
    Blocked,
    Warned,
    Allowed,
}

pub struct ActionGuard {
    rules: Vec<ForbiddenAction>,
    audit_log: Vec<AuditEntry>,
}

impl ActionGuard {
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            audit_log: Vec::new(),
        }
    }

    pub fn add_rule(&mut self, rule: ForbiddenAction) {
        self.rules.push(rule);
    }

    pub fn load_rules(&mut self, rules: Vec<ForbiddenAction>) {
        self.rules.extend(rules);
    }

    pub fn check(
        &mut self,
        agent_id: &str,
        role: &str,
        action: &str,
    ) -> Result<(), &ForbiddenAction> {
        let mut matched_block = None;
        let mut matched_warn = None;
        let mut matched_log = None;

        for rule in &self.rules {
            if rule.roles.iter().any(|r| r == role) && rule.description.contains(action) {
                match rule.severity {
                    Severity::Block => {
                        if matched_block.is_none() {
                            matched_block = Some(rule);
                        }
                    }
                    Severity::Warn => {
                        if matched_warn.is_none() {
                            matched_warn = Some(rule);
                        }
                    }
                    Severity::Log => {
                        if matched_log.is_none() {
                            matched_log = Some(rule);
                        }
                    }
                }
            }
        }

        if let Some(rule) = matched_block {
            let entry = AuditEntry {
                timestamp: crate::unix_timestamp().to_string(),
                agent_id: agent_id.to_string(),
                action: action.to_string(),
                rule_code: rule.code.clone(),
                severity: Severity::Block,
                outcome: Outcome::Blocked,
            };
            self.audit_log.push(entry);
            return Err(rule);
        }

        if let Some(rule) = matched_warn {
            let entry = AuditEntry {
                timestamp: crate::unix_timestamp().to_string(),
                agent_id: agent_id.to_string(),
                action: action.to_string(),
                rule_code: rule.code.clone(),
                severity: Severity::Warn,
                outcome: Outcome::Warned,
            };
            self.audit_log.push(entry);
        }

        if let Some(rule) = matched_log {
            let entry = AuditEntry {
                timestamp: crate::unix_timestamp().to_string(),
                agent_id: agent_id.to_string(),
                action: action.to_string(),
                rule_code: rule.code.clone(),
                severity: Severity::Log,
                outcome: Outcome::Allowed,
            };
            self.audit_log.push(entry);
        }

        Ok(())
    }

    pub fn audit_log(&self) -> &[AuditEntry] {
        &self.audit_log
    }

    pub fn clear_log(&mut self) {
        self.audit_log.clear();
    }

    pub fn rules(&self) -> &[ForbiddenAction] {
        &self.rules
    }
}

impl Default for ActionGuard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block_rule() -> ForbiddenAction {
        ForbiddenAction {
            code: "F001".to_string(),
            description: "Lead may not execute code directly".to_string(),
            severity: Severity::Block,
            roles: vec!["Lead".to_string()],
        }
    }

    fn warn_rule() -> ForbiddenAction {
        ForbiddenAction {
            code: "W001".to_string(),
            description: "Executor should not modify CI config".to_string(),
            severity: Severity::Warn,
            roles: vec!["Executor".to_string()],
        }
    }

    fn log_rule() -> ForbiddenAction {
        ForbiddenAction {
            code: "L001".to_string(),
            description: "Researcher read-only audit".to_string(),
            severity: Severity::Log,
            roles: vec!["Researcher".to_string()],
        }
    }

    #[test]
    fn block_action_returns_err() {
        let mut guard = ActionGuard::default();
        guard.add_rule(block_rule());
        let result = guard.check("agent-1", "Lead", "execute code directly");
        assert!(result.is_err());
        let rule = result.unwrap_err();
        assert_eq!(rule.code, "F001");
        assert_eq!(guard.audit_log().len(), 1);
        assert_eq!(guard.audit_log()[0].outcome, Outcome::Blocked);
    }

    #[test]
    fn warn_action_returns_ok_but_logs() {
        let mut guard = ActionGuard::default();
        guard.add_rule(warn_rule());
        let result = guard.check("agent-2", "Executor", "modify CI config");
        assert!(result.is_ok());
        assert_eq!(guard.audit_log().len(), 1);
        assert_eq!(guard.audit_log()[0].outcome, Outcome::Warned);
    }

    #[test]
    fn log_action_returns_ok() {
        let mut guard = ActionGuard::default();
        guard.add_rule(log_rule());
        let result = guard.check("agent-3", "Researcher", "read-only audit");
        assert!(result.is_ok());
        assert_eq!(guard.audit_log().len(), 1);
        assert_eq!(guard.audit_log()[0].outcome, Outcome::Allowed);
    }

    #[test]
    fn role_specific_rules_only_apply_to_matching_roles() {
        let mut guard = ActionGuard::default();
        guard.add_rule(block_rule());
        // Executor is not blocked by a rule targeting Lead
        let result = guard.check("agent-4", "Executor", "execute code directly");
        assert!(result.is_ok());
        assert!(guard.audit_log().is_empty());
    }

    #[test]
    fn audit_log_accumulates_entries() {
        let mut guard = ActionGuard::default();
        guard.add_rule(block_rule());
        guard.add_rule(warn_rule());
        guard.add_rule(log_rule());

        let _ = guard.check("a1", "Lead", "execute code directly");
        let _ = guard.check("a2", "Executor", "modify CI config");
        let _ = guard.check("a3", "Researcher", "read-only audit");

        assert_eq!(guard.audit_log().len(), 3);
    }

    #[test]
    fn clear_log_empties_entries() {
        let mut guard = ActionGuard::default();
        guard.add_rule(block_rule());
        let _ = guard.check("a1", "Lead", "execute code directly");
        assert_eq!(guard.audit_log().len(), 1);
        guard.clear_log();
        assert!(guard.audit_log().is_empty());
    }

    #[test]
    fn load_rules_populates_rules() {
        let mut guard = ActionGuard::default();
        assert!(guard.rules().is_empty());
        guard.load_rules(vec![block_rule(), warn_rule()]);
        assert_eq!(guard.rules().len(), 2);
    }
}
