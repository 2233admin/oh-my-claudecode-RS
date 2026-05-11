use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum AgentRole {
    Lead,
    Planner,
    Executor,
    Reviewer,
    Researcher,
    SecurityAuditor,
    Custom(String),
}

#[derive(Debug, Clone, Default)]
pub struct AgentSession {
    pub messages: Vec<String>,
    pub context_tokens: usize,
}

#[derive(Debug, Default)]
pub struct SharedState {
    pub tool_registry: HashMap<String, serde_json::Value>,
    pub memory: HashMap<String, String>,
}

pub struct AgentHandle {
    id: String,
    role: AgentRole,
    shared: Arc<RwLock<SharedState>>,
    session: AgentSession,
}

impl AgentHandle {
    pub fn new(id: impl Into<String>, role: AgentRole) -> Self {
        Self {
            id: id.into(),
            role,
            shared: Arc::new(RwLock::new(SharedState::default())),
            session: AgentSession::default(),
        }
    }

    pub fn fork(&self, new_id: impl Into<String>) -> Self {
        Self {
            id: new_id.into(),
            role: self.role.clone(),
            shared: Arc::clone(&self.shared),
            session: AgentSession::default(),
        }
    }

    pub fn fork_with_context(&self, new_id: impl Into<String>) -> Self {
        Self {
            id: new_id.into(),
            role: self.role.clone(),
            shared: Arc::clone(&self.shared),
            session: AgentSession {
                messages: self.session.messages.clone(),
                context_tokens: self.session.context_tokens,
            },
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn role(&self) -> &AgentRole {
        &self.role
    }

    pub fn session(&self) -> &AgentSession {
        &self.session
    }

    pub fn session_mut(&mut self) -> &mut AgentSession {
        &mut self.session
    }

    pub fn shared(&self) -> &Arc<RwLock<SharedState>> {
        &self.shared
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fork_creates_new_agent_with_empty_session() {
        let mut parent = AgentHandle::new("lead-1", AgentRole::Lead);
        parent.session_mut().messages.push("hello".to_string());
        parent.session_mut().context_tokens = 100;

        let child = parent.fork("executor-1");
        assert_eq!(child.id(), "executor-1");
        assert_eq!(*child.role(), AgentRole::Lead);
        assert!(child.session().messages.is_empty());
        assert_eq!(child.session().context_tokens, 0);
    }

    #[test]
    fn fork_with_context_copies_session_messages() {
        let mut parent = AgentHandle::new("lead-1", AgentRole::Lead);
        parent.session_mut().messages.push("msg1".to_string());
        parent.session_mut().messages.push("msg2".to_string());
        parent.session_mut().context_tokens = 200;

        let child = parent.fork_with_context("executor-2");
        assert_eq!(child.session().messages.len(), 2);
        assert_eq!(child.session().messages[0], "msg1");
        assert_eq!(child.session().context_tokens, 200);
    }

    #[tokio::test]
    async fn fork_agents_share_memory_mutations() {
        let parent = AgentHandle::new("lead-1", AgentRole::Lead);
        let child = parent.fork("executor-1");

        // Write via parent
        {
            let mut state = parent.shared().write().await;
            state.memory.insert("key".to_string(), "value".to_string());
        }

        // Read via child
        {
            let state = child.shared().read().await;
            assert_eq!(state.memory.get("key").map(std::string::String::as_str), Some("value"));
        }
    }

    #[tokio::test]
    async fn multiple_forks_dont_conflict() {
        let parent = AgentHandle::new("lead-1", AgentRole::Lead);
        let mut child_a = parent.fork("a");
        let mut child_b = parent.fork("b");

        child_a.session_mut().messages.push("from-a".to_string());
        child_b.session_mut().messages.push("from-b".to_string());

        assert_eq!(child_a.session().messages, vec!["from-a"]);
        assert_eq!(child_b.session().messages, vec!["from-b"]);

        // Shared state mutations from both don't conflict
        {
            let mut state = child_a.shared().write().await;
            state
                .memory
                .insert("a-key".to_string(), "a-val".to_string());
        }
        {
            let mut state = child_b.shared().write().await;
            state
                .memory
                .insert("b-key".to_string(), "b-val".to_string());
        }

        let state = parent.shared().read().await;
        assert_eq!(state.memory.get("a-key").map(std::string::String::as_str), Some("a-val"));
        assert_eq!(state.memory.get("b-key").map(std::string::String::as_str), Some("b-val"));
    }
}
