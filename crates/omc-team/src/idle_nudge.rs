use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NudgeConfig {
    pub delay_ms: u64,
    pub max_count: u32,
    pub message: String,
}

impl Default for NudgeConfig {
    fn default() -> Self {
        Self {
            delay_ms: 30_000,
            max_count: 3,
            message: "Please continue working or report a blocker.".to_string(),
        }
    }
}

#[derive(Debug, Clone, Default)]
struct NudgeState {
    nudge_count: u32,
    last_nudge_at: Option<String>,
    last_activity_at: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct NudgeTracker {
    workers: HashMap<String, NudgeState>,
    config: NudgeConfig,
}

impl NudgeTracker {
    pub fn new(config: NudgeConfig) -> Self {
        Self {
            workers: HashMap::default(),
            config,
        }
    }

    pub fn record_activity(&mut self, worker_id: &str) {
        let state = self.workers.entry(worker_id.to_string()).or_default();
        state.last_activity_at = Some(now_iso());
        state.nudge_count = 0;
    }

    /// Check workers for idleness and nudge those who have been idle longer than `delay_ms`.
    /// Returns the IDs of workers that were nudged.
    pub fn check_and_nudge(&mut self, worker_ids: &[String]) -> Vec<String> {
        let now_ms = chrono::Utc::now().timestamp_millis() as u64;
        let mut nudged = Vec::default();

        for worker_id in worker_ids {
            let state = self.workers.entry(worker_id.clone()).or_default();

            if state.nudge_count >= self.config.max_count {
                continue;
            }

            let should_nudge = if let Some(ref activity) = state.last_activity_at {
                if let Some(activity_ms) = parse_iso_to_ms(activity) {
                    now_ms.saturating_sub(activity_ms) >= self.config.delay_ms
                } else {
                    true
                }
            } else {
                true
            };

            if should_nudge {
                state.nudge_count += 1;
                state.last_nudge_at = Some(now_iso());
                nudged.push(worker_id.clone());
            }
        }

        nudged
    }

    pub fn get_summary(&self) -> HashMap<String, (u32, Option<String>)> {
        self.workers
            .iter()
            .map(|(id, state)| (id.clone(), (state.nudge_count, state.last_nudge_at.clone())))
            .collect()
    }

    pub fn total_nudges(&self) -> u32 {
        self.workers.values().map(|s| s.nudge_count).sum()
    }

    pub fn reset_worker(&mut self, worker_id: &str) {
        self.workers.remove(worker_id);
    }

    /// Inject a synthetic activity timestamp for a worker (used in tests).
    #[cfg(test)]
    fn set_activity_at(&mut self, worker_id: &str, ts: &str) {
        let state = self.workers.entry(worker_id.to_string()).or_default();
        state.last_activity_at = Some(ts.to_string());
    }
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn parse_iso_to_ms(iso: &str) -> Option<u64> {
    chrono::DateTime::parse_from_rfc3339(iso)
        .ok()
        .map(|dt| dt.timestamp_millis() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_tracker_has_zero_nudges() {
        let tracker = NudgeTracker::new(NudgeConfig::default());
        assert_eq!(tracker.total_nudges(), 0);
        assert!(tracker.get_summary().is_empty());
    }

    #[test]
    fn record_activity_prevents_nudge() {
        let mut tracker = NudgeTracker::new(NudgeConfig {
            delay_ms: 60_000,
            max_count: 3,
            message: "nudge".to_string(),
        });

        tracker.record_activity("w1");

        // Activity was just recorded, so immediate check should not trigger nudge
        let nudged = tracker.check_and_nudge(&["w1".to_string()]);
        assert!(nudged.is_empty());
    }

    #[test]
    fn nudge_after_delay() {
        let mut tracker = NudgeTracker::new(NudgeConfig {
            delay_ms: 5000,
            max_count: 3,
            message: "nudge".to_string(),
        });

        // Set activity to 10 seconds ago
        let past = chrono::Utc::now() - chrono::Duration::seconds(10);
        tracker.set_activity_at("w1", &past.to_rfc3339());

        let nudged = tracker.check_and_nudge(&["w1".to_string()]);
        assert_eq!(nudged, vec!["w1"]);
        assert_eq!(tracker.total_nudges(), 1);
    }

    #[test]
    fn max_count_stops_nudging() {
        let mut tracker = NudgeTracker::new(NudgeConfig {
            delay_ms: 1000,
            max_count: 2,
            message: "nudge".to_string(),
        });

        // Set activity far in the past so every check triggers
        let past = chrono::Utc::now() - chrono::Duration::hours(1);
        tracker.set_activity_at("w1", &past.to_rfc3339());

        let workers = vec!["w1".to_string()];

        // First nudge
        let nudged = tracker.check_and_nudge(&workers);
        assert_eq!(nudged.len(), 1);

        // Second nudge
        let nudged = tracker.check_and_nudge(&workers);
        assert_eq!(nudged.len(), 1);

        // Third attempt - max reached
        let nudged = tracker.check_and_nudge(&workers);
        assert!(nudged.is_empty());
        assert_eq!(tracker.total_nudges(), 2);
    }

    #[test]
    fn reset_worker_clears_state() {
        let mut tracker = NudgeTracker::new(NudgeConfig {
            delay_ms: 1000,
            max_count: 3,
            message: "nudge".to_string(),
        });

        let past = chrono::Utc::now() - chrono::Duration::hours(1);
        tracker.set_activity_at("w1", &past.to_rfc3339());

        tracker.check_and_nudge(&["w1".to_string()]);
        assert!(tracker.total_nudges() > 0);

        tracker.reset_worker("w1");
        assert_eq!(tracker.total_nudges(), 0);
        assert!(tracker.get_summary().is_empty());
    }

    #[test]
    fn multiple_workers_independent() {
        let mut tracker = NudgeTracker::new(NudgeConfig {
            delay_ms: 5000,
            max_count: 3,
            message: "nudge".to_string(),
        });

        let past = chrono::Utc::now() - chrono::Duration::seconds(10);
        tracker.set_activity_at("w1", &past.to_rfc3339());
        tracker.set_activity_at("w2", &past.to_rfc3339());

        // Only nudge w1
        let nudged = tracker.check_and_nudge(&["w1".to_string()]);
        assert_eq!(nudged, vec!["w1"]);
        assert_eq!(tracker.total_nudges(), 1);

        // w2 not nudged yet because it wasn't in the check list
        let summary = tracker.get_summary();
        assert_eq!(summary.get("w2").unwrap().0, 0);
    }
}
