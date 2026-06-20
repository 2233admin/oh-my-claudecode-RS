//! In-process shared memory for swarm agent coordination.
//!
//! Keys follow a hierarchical namespace scheme:
//! - `swarm/{agent-id}/status` — per-agent status
//! - `swarm/shared/*` — shared state readable by all agents
//! - `swarm/queen/*` — queen-only state
//! - `swarm/broadcast/*` — broadcast messages

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

/// Errors that can occur during shared memory operations.
#[derive(Debug, thiserror::Error)]
pub enum SharedMemoryError {
    #[error("invalid key: {0}")]
    InvalidKey(String),

    #[error("serialization error: {0}")]
    Serialize(#[from] serde_json::Error),
}

/// A single entry in the shared memory store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    /// The stored value.
    pub value: serde_json::Value,
    /// Timestamp when this entry was last written (milliseconds since epoch).
    pub updated_at_ms: u64,
    /// The namespace that owns this key (e.g. "swarm/agent-1").
    pub namespace: String,
}

/// In-process shared memory backed by a `BTreeMap` behind `Arc<RwLock>`.
#[derive(Debug, Clone)]
pub struct SharedMemory {
    store: Arc<RwLock<BTreeMap<String, MemoryEntry>>>,
    namespace: String,
}

impl SharedMemory {
    /// Create a new shared memory instance with the given namespace prefix.
    pub fn new(namespace: impl Into<String>) -> Self {
        Self {
            store: Arc::new(RwLock::new(BTreeMap::new())),
            namespace: namespace.into(),
        }
    }

    /// Return the namespace of this instance.
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    /// Store a value under `key` in this instance's namespace.
    pub async fn store(
        &self,
        key: &str,
        value: serde_json::Value,
    ) -> Result<(), SharedMemoryError> {
        Self::validate_key(key)?;
        let full_key = format!("{}/{}", self.namespace, key);
        let entry = MemoryEntry {
            value,
            updated_at_ms: now_ms(),
            namespace: self.namespace.clone(),
        };
        self.store.write().await.insert(full_key, entry);
        Ok(())
    }

    /// Retrieve the entry for `key` in this instance's namespace.
    pub async fn retrieve(&self, key: &str) -> Option<MemoryEntry> {
        let full_key = format!("{}/{}", self.namespace, key);
        self.store.read().await.get(&full_key).cloned()
    }

    /// Store a value under `key` in the `swarm/broadcast` namespace so all agents can read it.
    pub async fn broadcast(
        &self,
        key: &str,
        value: serde_json::Value,
    ) -> Result<(), SharedMemoryError> {
        Self::validate_key(key)?;
        let full_key = format!("swarm/broadcast/{}", key);
        let entry = MemoryEntry {
            value,
            updated_at_ms: now_ms(),
            namespace: "swarm/broadcast".to_string(),
        };
        self.store.write().await.insert(full_key, entry);
        Ok(())
    }

    /// List all keys whose full path starts with `prefix`.
    pub async fn list_keys(&self, prefix: &str) -> Vec<String> {
        let store = self.store.read().await;
        store
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect()
    }

    /// Remove a key from this instance's namespace. Returns the entry if it existed.
    pub async fn remove(&self, key: &str) -> Option<MemoryEntry> {
        let full_key = format!("{}/{}", self.namespace, key);
        self.store.write().await.remove(&full_key)
    }

    /// Return the total number of entries in the store.
    pub async fn len(&self) -> usize {
        self.store.read().await.len()
    }

    /// Return whether the store is empty.
    pub async fn is_empty(&self) -> bool {
        self.store.read().await.is_empty()
    }

    fn validate_key(key: &str) -> Result<(), SharedMemoryError> {
        if key.is_empty() {
            return Err(SharedMemoryError::InvalidKey(
                "key must not be empty".into(),
            ));
        }
        if key.contains("..") {
            return Err(SharedMemoryError::InvalidKey(
                "key must not contain '..'".into(),
            ));
        }
        Ok(())
    }
}

/// Milliseconds since Unix epoch.
fn now_ms() -> u64 {
    chrono::Utc::now().timestamp_millis().max(0) as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn store_and_retrieve() {
        let mem = SharedMemory::new("swarm/agent-1");
        mem.store("status", json!({"state": "active"}))
            .await
            .unwrap();

        let entry = mem.retrieve("status").await.expect("entry should exist");
        assert_eq!(entry.value, json!({"state": "active"}));
        assert_eq!(entry.namespace, "swarm/agent-1");
    }

    #[tokio::test]
    async fn retrieve_missing_key_returns_none() {
        let mem = SharedMemory::new("swarm/agent-1");
        assert!(mem.retrieve("nonexistent").await.is_none());
    }

    #[tokio::test]
    async fn broadcast_visible_across_namespaces() {
        // Share the underlying store so both agents see the same data.
        let shared = Arc::new(RwLock::new(BTreeMap::new()));
        let a = SharedMemory {
            store: Arc::clone(&shared),
            namespace: "swarm/agent-a".into(),
        };
        let b = SharedMemory {
            store: Arc::clone(&shared),
            namespace: "swarm/agent-b".into(),
        };

        a.broadcast("task_done", json!({"task": "build"}))
            .await
            .unwrap();

        // Both agents can read broadcast keys.
        let keys_b = b.list_keys("swarm/broadcast").await;
        assert_eq!(keys_b, vec!["swarm/broadcast/task_done"]);

        // Direct retrieve via full key from b's perspective.
        let store = shared.read().await;
        let entry = store.get("swarm/broadcast/task_done").unwrap();
        assert_eq!(entry.value, json!({"task": "build"}));
    }

    #[tokio::test]
    async fn list_keys_with_prefix() {
        let shared = Arc::new(RwLock::new(BTreeMap::new()));
        let mem = SharedMemory {
            store: shared,
            namespace: "swarm/queen".into(),
        };

        mem.store("plan/a", json!("alpha")).await.unwrap();
        mem.store("plan/b", json!("beta")).await.unwrap();
        mem.store("status", json!("ok")).await.unwrap();

        let keys = mem.list_keys("swarm/queen/plan").await;
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"swarm/queen/plan/a".to_string()));
        assert!(keys.contains(&"swarm/queen/plan/b".to_string()));
    }

    #[tokio::test]
    async fn remove_entry() {
        let mem = SharedMemory::new("swarm/agent-1");
        mem.store("temp", json!(42)).await.unwrap();
        assert_eq!(mem.len().await, 1);

        let removed = mem.remove("temp").await;
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().value, json!(42));
        assert!(mem.is_empty().await);
    }

    #[tokio::test]
    async fn reject_empty_key() {
        let mem = SharedMemory::new("swarm/agent-1");
        let result = mem.store("", json!("bad")).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn reject_dot_dot_key() {
        let mem = SharedMemory::new("swarm/agent-1");
        let result = mem.store("../escape", json!("bad")).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn namespace_key_structure() {
        let mem = SharedMemory::new("swarm/agent-42");
        mem.store("status", json!("running")).await.unwrap();

        let keys = mem.list_keys("swarm/agent-42/").await;
        assert_eq!(keys, vec!["swarm/agent-42/status"]);
    }

    #[tokio::test]
    async fn overwrite_existing_key() {
        let mem = SharedMemory::new("swarm/agent-1");
        mem.store("status", json!("active")).await.unwrap();
        mem.store("status", json!("idle")).await.unwrap();

        let entry = mem.retrieve("status").await.unwrap();
        assert_eq!(entry.value, json!("idle"));
        assert_eq!(mem.len().await, 1);
    }
}
