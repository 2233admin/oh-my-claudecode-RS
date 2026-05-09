//! Skill state store module - manages runtime state for skill execution

use std::collections::HashMap;
use std::sync::Arc;
use dashmap::DashMap;

/// Thread-safe state store for skill execution
///
/// Provides a key-value store for maintaining state across skill executions,
/// including variables set during template substitution and execution context.
#[derive(Debug, Clone)]
pub struct SkillStateStore {
    store: Arc<DashMap<String, String>>,
}

impl SkillStateStore {
    /// Create a new empty state store
    pub fn new() -> Self {
        Self {
            store: Arc::new(DashMap::new()),
        }
    }

    /// Create a state store with initial values
    pub fn with_values(values: HashMap<String, String>) -> Self {
        let store = DashMap::new();
        for (k, v) in values {
            store.insert(k, v);
        }
        Self {
            store: Arc::new(store),
        }
    }

    /// Set a state variable
    ///
    /// # Arguments
    ///
    /// * `key` - Variable name
    /// * `value` - Variable value
    pub fn set(&self, key: &str, value: impl Into<String>) {
        self.store.insert(key.to_string(), value.into());
    }
            store.insert(key.to_string(), value.into());
        }
    }

    /// Get a state variable
    ///
    /// # Arguments
    ///
    /// * `key` - Variable name
    ///
    /// # Returns
    ///
    /// * `Some(&String)` if the variable exists
    /// * `None` if the variable does not exist
    pub fn get(&self, key: &str) -> Option<String> {
        self.store
            .read()
            .ok()
            .and_then(|store| store.get(key).cloned())
    }

    /// Check if a variable exists
    pub fn contains(&self, key: &str) -> bool {
        self.store
            .read()
            .map(|store| store.contains_key(key))
            .unwrap_or(false)
    }

    /// Remove a variable
    pub fn remove(&self, key: &str) -> Option<String> {
        self.store
            .write()
            .ok()
            .and_then(|mut store| store.remove(key))
    }

    /// Clear all state
    pub fn clear(&self) {
        if let Ok(mut store) = self.store.write() {
            store.clear();
        }
    }

    /// Get all keys
    pub fn keys(&self) -> Vec<String> {
        self.store
            .read()
            .map(|store| store.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Get all key-value pairs
    pub fn entries(&self) -> HashMap<String, String> {
        self.store
            .read()
            .cloned()
            .unwrap_or_default()
    }

    /// Get the number of stored variables
    pub fn len(&self) -> usize {
        self.store.read().map(|store| store.len()).unwrap_or(0)
    }

    /// Check if the store is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Merge another state store into this one
    pub fn merge(&self, other: &SkillStateStore) {
        if let (Ok(mut self_store), Ok(other_store)) = (self.store.write(), other.store.read()) {
            self_store.extend(other_store.clone());
        }
    }
}

impl Default for SkillStateStore {
    fn default() -> Self {
        Self {
            skills: Vec::new(),
        }
    }
}

impl From<HashMap<String, String>> for SkillStateStore {
    fn from(map: HashMap<String, String>) -> Self {
        Self::with_values(map)
    }
}

use std::sync::Arc;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_and_get() {
        let store = SkillStateStore::default();

        store.set("name", "Alice");
        store.set("age", "30");

        assert_eq!(store.get("name"), Some("Alice".to_string()));
        assert_eq!(store.get("age"), Some("30".to_string()));
    }

    #[test]
    fn test_get_nonexistent() {
        let store = SkillStateStore::default();
        assert_eq!(store.get("nonexistent"), None);
    }

    #[test]
    fn test_contains() {
        let store = SkillStateStore::default();

        store.set("exists", "value");

        assert!(store.contains("exists"));
        assert!(!store.contains("not_exists"));
    }

    #[test]
    fn test_remove() {
        let store = SkillStateStore::default();

        store.set("temp", "temporary");
        assert!(store.contains("temp"));

        let removed = store.remove("temp");
        assert_eq!(removed, Some("temporary".to_string()));
        assert!(!store.contains("temp"));
    }

    #[test]
    fn test_clear() {
        let store = SkillStateStore::default();

        store.set("a", "1");
        store.set("b", "2");
        assert_eq!(store.len(), 2);

        store.clear();
        assert!(store.is_empty());
    }

    #[test]
    fn test_keys() {
        let store = SkillStateStore::default();

        store.set("foo", "1");
        store.set("bar", "2");

        let mut keys = store.keys();
        keys.sort();
        assert_eq!(keys, vec!["bar", "foo"]);
    }

    #[test]
    fn test_entries() {
        let store = SkillStateStore::default();

        store.set("x", "1");
        store.set("y", "2");

        let entries = store.entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries.get("x"), Some(&"1".to_string()));
        assert_eq!(entries.get("y"), Some(&"2".to_string()));
    }

    #[test]
    fn test_merge() {
        let store1 = SkillStateStore::default();
        let store2 = SkillStateStore::default();

        store1.set("a", "1");
        store1.set("b", "2");
        store2.set("b", "overwrite");
        store2.set("c", "3");

        store1.merge(&store2);

        assert_eq!(store1.get("a"), Some("1".to_string()));
        assert_eq!(store1.get("b"), Some("overwrite".to_string()));
        assert_eq!(store1.get("c"), Some("3".to_string()));
    }

    #[test]
    fn test_clone_independence() {
        let store1 = SkillStateStore::default();
        store1.set("shared", "value");

        let store2 = store1.clone();
        store2.set("shared", "modified");

        // Clones share the same underlying store
        assert_eq!(store1.get("shared"), Some("modified".to_string()));
        assert_eq!(store2.get("shared"), Some("modified".to_string()));
    }

    #[test]
    fn test_from_hashmap() {
        let mut map = HashMap::new();
        map.insert("k1".to_string(), "v1".to_string());
        map.insert("k2".to_string(), "v2".to_string());

        let store = SkillStateStore::from(map);

        assert_eq!(store.get("k1"), Some("v1".to_string()));
        assert_eq!(store.get("k2"), Some("v2".to_string()));
    }

    #[test]
    fn test_len_and_is_empty() {
        let store = SkillStateStore::default();

        assert!(store.is_empty());
        assert_eq!(store.len(), 0);

        store.set("x", "1");
        assert!(!store.is_empty());
        assert_eq!(store.len(), 1);
    }
}
