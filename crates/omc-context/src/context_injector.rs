//! Context Injector
//!
//! Manages collection and injection of context entries from multiple sources.
//! Supports priority ordering and deduplication.
//!
//! Ported from oh-my-claudecode's features/context-injector.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Source identifier for context injection.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ContextSourceType {
    KeywordDetector,
    RulesInjector,
    DirectoryAgents,
    DirectoryReadme,
    BoulderState,
    SessionContext,
    Learner,
    Beads,
    ProjectMemory,
    Custom(String),
}

impl ContextSourceType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::KeywordDetector => "keyword-detector",
            Self::RulesInjector => "rules-injector",
            Self::DirectoryAgents => "directory-agents",
            Self::DirectoryReadme => "directory-readme",
            Self::BoulderState => "boulder-state",
            Self::SessionContext => "session-context",
            Self::Learner => "learner",
            Self::Beads => "beads",
            Self::ProjectMemory => "project-memory",
            Self::Custom(s) => s.as_str(),
        }
    }
}

/// Priority levels for context ordering. Higher priority appears first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum ContextPriority {
    Critical = 0,
    High = 1,
    #[default]
    Normal = 2,
    Low = 3,
}

/// A single context entry registered by a source.
#[derive(Debug, Clone)]
pub struct ContextEntry {
    pub id: String,
    pub source: ContextSourceType,
    pub content: String,
    pub priority: ContextPriority,
    pub timestamp: i64,
    pub metadata: Option<serde_json::Value>,
}

/// Options for registering context.
#[derive(Debug, Clone)]
pub struct RegisterContextOptions {
    pub id: String,
    pub source: ContextSourceType,
    pub content: String,
    pub priority: Option<ContextPriority>,
    pub metadata: Option<serde_json::Value>,
}

/// Result of getting pending context for a session.
#[derive(Debug, Clone)]
pub struct PendingContext {
    pub merged: String,
    pub entries: Vec<ContextEntry>,
    pub has_content: bool,
}

/// Injection strategy for context.
#[derive(Debug, Clone, Copy, Default)]
pub enum InjectionStrategy {
    #[default]
    Prepend,
    Append,
    Wrap,
}

/// Result of an injection operation.
#[derive(Debug, Clone)]
pub struct InjectionResult {
    pub injected: bool,
    pub context_length: usize,
    pub entry_count: usize,
}

const CONTEXT_SEPARATOR: &str = "\n\n---\n\n";
const DEFAULT_SEPARATOR: &str = "\n\n---\n\n";

/// Collects and manages context entries for sessions.
#[derive(Debug, Clone, Default)]
pub struct ContextCollector {
    sessions: Arc<RwLock<HashMap<String, HashMap<String, ContextEntry>>>>,
}

impl ContextCollector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a context entry for a session.
    /// If an entry with the same source:id already exists, it will be replaced.
    pub async fn register(&self, session_id: &str, options: RegisterContextOptions) {
        let mut sessions = self.sessions.write().await;
        let session_map = sessions
            .entry(session_id.to_string())
            .or_insert_with(HashMap::new);

        let key = format!("{}:{}", options.source.as_str(), options.id);
        let entry = ContextEntry {
            id: options.id,
            source: options.source,
            content: options.content,
            priority: options.priority.unwrap_or_default(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            metadata: options.metadata,
        };

        session_map.insert(key, entry);
    }

    /// Get pending context for a session without consuming it.
    pub async fn get_pending(&self, session_id: &str) -> PendingContext {
        let sessions = self.sessions.read().await;
        let Some(session_map) = sessions.get(session_id) else {
            return PendingContext {
                merged: String::new(),
                entries: Vec::new(),
                has_content: false,
            };
        };

        if session_map.is_empty() {
            return PendingContext {
                merged: String::new(),
                entries: Vec::new(),
                has_content: false,
            };
        }

        let mut entries: Vec<ContextEntry> = session_map.values().cloned().collect();
        Self::sort_entries(&mut entries);

        let merged = entries
            .iter()
            .map(|e| e.content.as_str())
            .collect::<Vec<_>>()
            .join(CONTEXT_SEPARATOR);

        let has_content = !entries.is_empty();
        PendingContext {
            merged,
            entries,
            has_content,
        }
    }

    /// Get and consume pending context for a session.
    pub async fn consume(&self, session_id: &str) -> PendingContext {
        let pending = self.get_pending(session_id).await;
        self.clear(session_id).await;
        pending
    }

    /// Clear all context for a session.
    pub async fn clear(&self, session_id: &str) {
        let mut sessions = self.sessions.write().await;
        sessions.remove(session_id);
    }

    /// Check if a session has pending context.
    pub async fn has_pending(&self, session_id: &str) -> bool {
        let sessions = self.sessions.read().await;
        sessions.get(session_id).is_some_and(|m| !m.is_empty())
    }

    /// Get count of entries for a session.
    pub async fn entry_count(&self, session_id: &str) -> usize {
        let sessions = self.sessions.read().await;
        sessions.get(session_id).map_or(0, |m| m.len())
    }

    /// Remove a specific entry from a session.
    pub async fn remove_entry(&self, session_id: &str, source: &str, id: &str) -> bool {
        let mut sessions = self.sessions.write().await;
        let Some(session_map) = sessions.get_mut(session_id) else {
            return false;
        };
        let key = format!("{source}:{id}");
        session_map.remove(&key).is_some()
    }

    /// Get all active session IDs.
    pub async fn active_sessions(&self) -> Vec<String> {
        let sessions = self.sessions.read().await;
        sessions.keys().cloned().collect()
    }

    fn sort_entries(entries: &mut [ContextEntry]) {
        entries.sort_by(|a, b| {
            a.priority
                .cmp(&b.priority)
                .then(a.timestamp.cmp(&b.timestamp))
        });
    }
}

/// Inject pending context into a text string.
pub async fn inject_context_into_text(
    collector: &ContextCollector,
    session_id: &str,
    text: &str,
    strategy: InjectionStrategy,
) -> (String, InjectionResult) {
    if !collector.has_pending(session_id).await {
        return (
            text.to_string(),
            InjectionResult {
                injected: false,
                context_length: 0,
                entry_count: 0,
            },
        );
    }

    let pending = collector.consume(session_id).await;
    let result = match strategy {
        InjectionStrategy::Prepend => {
            format!("{}{}{}", pending.merged, DEFAULT_SEPARATOR, text)
        }
        InjectionStrategy::Append => {
            format!("{}{}{}", text, DEFAULT_SEPARATOR, pending.merged)
        }
        InjectionStrategy::Wrap => {
            format!(
                "<injected-context>\n{}\n</injected-context>{}{}",
                pending.merged, DEFAULT_SEPARATOR, text
            )
        }
    };

    let context_length = pending.merged.len();
    let entry_count = pending.entries.len();
    (
        result,
        InjectionResult {
            injected: true,
            context_length,
            entry_count,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_register_and_consume() {
        let collector = ContextCollector::new();
        collector
            .register(
                "sess-1",
                RegisterContextOptions {
                    id: "test".into(),
                    source: ContextSourceType::ProjectMemory,
                    content: "hello world".into(),
                    priority: None,
                    metadata: None,
                },
            )
            .await;

        assert!(collector.has_pending("sess-1").await);
        let pending = collector.consume("sess-1").await;
        assert_eq!(pending.merged, "hello world");
        assert!(!collector.has_pending("sess-1").await);
    }

    #[tokio::test]
    async fn test_priority_ordering() {
        let collector = ContextCollector::new();
        collector
            .register(
                "s",
                RegisterContextOptions {
                    id: "low".into(),
                    source: ContextSourceType::Custom("a".into()),
                    content: "low".into(),
                    priority: Some(ContextPriority::Low),
                    metadata: None,
                },
            )
            .await;
        collector
            .register(
                "s",
                RegisterContextOptions {
                    id: "critical".into(),
                    source: ContextSourceType::Custom("b".into()),
                    content: "critical".into(),
                    priority: Some(ContextPriority::Critical),
                    metadata: None,
                },
            )
            .await;

        let pending = collector.get_pending("s").await;
        assert_eq!(pending.entries[0].content, "critical");
        assert_eq!(pending.entries[1].content, "low");
    }

    #[tokio::test]
    async fn test_deduplication() {
        let collector = ContextCollector::new();
        collector
            .register(
                "s",
                RegisterContextOptions {
                    id: "dup".into(),
                    source: ContextSourceType::ProjectMemory,
                    content: "first".into(),
                    priority: None,
                    metadata: None,
                },
            )
            .await;
        collector
            .register(
                "s",
                RegisterContextOptions {
                    id: "dup".into(),
                    source: ContextSourceType::ProjectMemory,
                    content: "second".into(),
                    priority: None,
                    metadata: None,
                },
            )
            .await;

        assert_eq!(collector.entry_count("s").await, 1);
        let pending = collector.consume("s").await;
        assert_eq!(pending.merged, "second");
    }

    #[tokio::test]
    async fn test_inject_context_into_text() {
        let collector = ContextCollector::new();
        collector
            .register(
                "s",
                RegisterContextOptions {
                    id: "ctx".into(),
                    source: ContextSourceType::ProjectMemory,
                    content: "injected".into(),
                    priority: None,
                    metadata: None,
                },
            )
            .await;

        let (result, inj) =
            inject_context_into_text(&collector, "s", "original", InjectionStrategy::Prepend).await;
        assert!(inj.injected);
        assert!(result.starts_with("injected"));
        assert!(result.contains("original"));
    }
}
