use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::Semaphore;

use super::BackgroundTaskConfig;

pub struct ConcurrencyManager {
    config: BackgroundTaskConfig,
    semaphores: Mutex<HashMap<String, Arc<Semaphore>>>,
    active_counts: Mutex<HashMap<String, usize>>,
}

impl ConcurrencyManager {
    pub fn new(config: BackgroundTaskConfig) -> Self {
        Self {
            config,
            semaphores: Mutex::new(HashMap::new()),
            active_counts: Mutex::new(HashMap::new()),
        }
    }

    pub fn get_concurrency_limit(&self, key: &str) -> usize {
        if let Some(&limit) = self.config.model_concurrency.get(key) {
            return if limit == 0 { usize::MAX } else { limit };
        }

        let provider = key.split('/').next().unwrap_or(key);
        if let Some(&limit) = self.config.provider_concurrency.get(provider) {
            return if limit == 0 { usize::MAX } else { limit };
        }

        if let Some(limit) = self.config.default_concurrency {
            return if limit == 0 { usize::MAX } else { limit };
        }

        5
    }

    pub async fn acquire(&self, key: &str) {
        let limit = self.get_concurrency_limit(key);
        if limit == usize::MAX {
            return;
        }

        let semaphore = {
            let mut sems = self.semaphores.lock().unwrap();
            sems.entry(key.to_string())
                .or_insert_with(|| Arc::new(Semaphore::new(limit)))
                .clone()
        };

        let _permit = semaphore.acquire().await.unwrap();
        let mut counts = self.active_counts.lock().unwrap();
        *counts.entry(key.to_string()).or_insert(0) += 1;
        drop(counts);

        // Keep the permit alive by forgetting it. We manually track counts.
        // When release() is called, we add a permit back.
        _permit.forget();
    }

    pub fn release(&self, key: &str) {
        let limit = self.get_concurrency_limit(key);
        if limit == usize::MAX {
            return;
        }

        let mut counts = self.active_counts.lock().unwrap();
        let count = counts.entry(key.to_string()).or_insert(0);
        if *count > 0 {
            *count -= 1;
        }

        let sems = self.semaphores.lock().unwrap();
        if let Some(sem) = sems.get(key) {
            sem.add_permits(1);
        }
    }

    pub fn get_count(&self, key: &str) -> usize {
        *self.active_counts.lock().unwrap().get(key).unwrap_or(&0)
    }

    pub fn get_queue_length(&self, key: &str) -> usize {
        let sems = self.semaphores.lock().unwrap();
        if sems.get(key).is_some() {
            let limit = self.get_concurrency_limit(key);
            let count = self.get_count(key);
            if count >= limit {
                // Available permits is 0, queued = waiters
                // This is an approximation since tokio doesn't expose waiter count
                return count.saturating_sub(limit);
            }
        }
        0
    }

    pub fn is_at_capacity(&self, key: &str) -> bool {
        let limit = self.get_concurrency_limit(key);
        if limit == usize::MAX {
            return false;
        }
        self.get_count(key) >= limit
    }

    pub fn get_active_counts(&self) -> HashMap<String, usize> {
        self.active_counts.lock().unwrap().clone()
    }

    pub fn clear(&self) {
        self.active_counts.lock().unwrap().clear();
        self.semaphores.lock().unwrap().clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_concurrency_limit_is_5() {
        let mgr = ConcurrencyManager::new(BackgroundTaskConfig::default());
        assert_eq!(mgr.get_concurrency_limit("some-model"), 5);
    }

    #[test]
    fn model_specific_limit() {
        let config = BackgroundTaskConfig {
            model_concurrency: HashMap::from([("claude-3-opus".to_string(), 2)]),
            ..Default::default()
        };
        let mgr = ConcurrencyManager::new(config);
        assert_eq!(mgr.get_concurrency_limit("claude-3-opus"), 2);
        assert_eq!(mgr.get_concurrency_limit("claude-3-sonnet"), 5);
    }

    #[test]
    fn provider_specific_limit() {
        let config = BackgroundTaskConfig {
            provider_concurrency: HashMap::from([("anthropic".to_string(), 10)]),
            ..Default::default()
        };
        let mgr = ConcurrencyManager::new(config);
        assert_eq!(mgr.get_concurrency_limit("anthropic/claude-3"), 10);
    }

    #[test]
    fn default_concurrency_zero_means_unlimited() {
        let config = BackgroundTaskConfig {
            default_concurrency: Some(0),
            ..Default::default()
        };
        let mgr = ConcurrencyManager::new(config);
        assert_eq!(mgr.get_concurrency_limit("any"), usize::MAX);
    }

    #[tokio::test]
    async fn acquire_and_release() {
        let config = BackgroundTaskConfig {
            default_concurrency: Some(2),
            ..Default::default()
        };
        let mgr = ConcurrencyManager::new(config);

        mgr.acquire("test").await;
        assert_eq!(mgr.get_count("test"), 1);

        mgr.acquire("test").await;
        assert_eq!(mgr.get_count("test"), 2);
        assert!(mgr.is_at_capacity("test"));

        mgr.release("test");
        assert_eq!(mgr.get_count("test"), 1);
        assert!(!mgr.is_at_capacity("test"));
    }

    #[tokio::test]
    async fn unlimited_key_never_blocks() {
        let config = BackgroundTaskConfig {
            default_concurrency: Some(0),
            ..Default::default()
        };
        let mgr = ConcurrencyManager::new(config);

        for _ in 0..100 {
            mgr.acquire("fast").await;
        }
        assert_eq!(mgr.get_count("fast"), 0); // Unlimited skips tracking
    }

    #[test]
    fn clear_resets_state() {
        let mgr = ConcurrencyManager::new(BackgroundTaskConfig::default());
        {
            let mut counts = mgr.active_counts.lock().unwrap();
            counts.insert("test".to_string(), 5);
        }
        mgr.clear();
        assert_eq!(mgr.get_count("test"), 0);
    }
}
