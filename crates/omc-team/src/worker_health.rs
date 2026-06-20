use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// Status of a managed worker.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum WorkerStatus {
    Starting,
    Healthy,
    Degraded,
    Unhealthy,
    Unresponsive,
    Stopped,
    Crashed,
}

/// Snapshot of a single worker's health state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerHealth {
    pub worker_id: String,
    pub status: WorkerStatus,
    pub last_heartbeat: Option<String>,
    pub consecutive_errors: u32,
    pub tasks_completed: u32,
    pub tasks_failed: u32,
    pub uptime_seconds: u64,
    pub capabilities: Vec<String>,
    pub high_priority_completed: u32,
}

/// Health-check result for a single worker.
#[derive(Debug, Clone)]
pub struct WorkerHealthReport {
    pub worker_id: String,
    pub previous_status: WorkerStatus,
    pub current_status: WorkerStatus,
    pub consecutive_errors: u32,
}

/// Tracks heartbeat, success/error counts, and derives status for all workers.
pub struct HealthMonitor {
    workers: HashMap<String, WorkerHealth>,
    registered_at: HashMap<String, Instant>,
    last_heartbeat_instant: HashMap<String, Instant>,
    heartbeat_timeout_ms: u64,
    unhealthy_threshold: u32,
}

impl HealthMonitor {
    pub fn new(heartbeat_timeout_ms: u64, unhealthy_threshold: u32) -> Self {
        Self {
            workers: HashMap::default(),
            registered_at: HashMap::default(),
            last_heartbeat_instant: HashMap::default(),
            heartbeat_timeout_ms,
            unhealthy_threshold,
        }
    }

    pub fn register_worker(&mut self, worker_id: String) {
        let now = Instant::now();
        let health = WorkerHealth {
            worker_id: worker_id.clone(),
            status: WorkerStatus::Starting,
            last_heartbeat: None,
            consecutive_errors: 0,
            tasks_completed: 0,
            tasks_failed: 0,
            uptime_seconds: 0,
            capabilities: Vec::default(),
            high_priority_completed: 0,
        };
        self.workers.insert(worker_id.clone(), health);
        self.registered_at.insert(worker_id.clone(), now);
        self.last_heartbeat_instant.insert(worker_id, now);
    }

    pub fn record_heartbeat(&mut self, worker_id: &str) {
        if let Some(health) = self.workers.get_mut(worker_id) {
            if health.status == WorkerStatus::Starting
                || health.status == WorkerStatus::Unresponsive
            {
                health.status = WorkerStatus::Healthy;
            }
            health.last_heartbeat = Some(crate::unix_timestamp().to_string());
        }
        self.last_heartbeat_instant
            .insert(worker_id.to_string(), Instant::now());
    }

    pub fn record_success(&mut self, worker_id: &str) {
        if let Some(health) = self.workers.get_mut(worker_id) {
            health.tasks_completed += 1;
            health.consecutive_errors = 0;
            if health.status == WorkerStatus::Degraded {
                health.status = WorkerStatus::Healthy;
            }
        }
    }

    pub fn record_error(&mut self, worker_id: &str) {
        if let Some(health) = self.workers.get_mut(worker_id) {
            health.tasks_failed += 1;
            health.consecutive_errors += 1;
            if health.consecutive_errors >= self.unhealthy_threshold {
                health.status = WorkerStatus::Unhealthy;
            } else if health.consecutive_errors > 0 {
                health.status = WorkerStatus::Degraded;
            }
        }
    }

    pub fn check_health(&mut self) -> Vec<WorkerHealthReport> {
        let mut reports = Vec::default();
        let timeout = Duration::from_millis(self.heartbeat_timeout_ms);

        for (worker_id, health) in self.workers.iter_mut() {
            let previous = health.status.clone();

            // Update uptime
            if let Some(registered) = self.registered_at.get(worker_id) {
                health.uptime_seconds = registered.elapsed().as_secs();
            }

            // Check heartbeat timeout
            if let Some(last) = self.last_heartbeat_instant.get(worker_id)
                && last.elapsed() > timeout
                && health.status != WorkerStatus::Stopped
                && health.status != WorkerStatus::Crashed
            {
                health.status = WorkerStatus::Unresponsive;
            }

            if health.status != previous {
                reports.push(WorkerHealthReport {
                    worker_id: worker_id.clone(),
                    previous_status: previous,
                    current_status: health.status.clone(),
                    consecutive_errors: health.consecutive_errors,
                });
            }
        }
        reports
    }

    pub fn get_worker(&self, worker_id: &str) -> Option<&WorkerHealth> {
        self.workers.get(worker_id)
    }

    pub fn remove_worker(&mut self, worker_id: &str) {
        self.workers.remove(worker_id);
        self.registered_at.remove(worker_id);
        self.last_heartbeat_instant.remove(worker_id);
    }

    pub fn active_worker_count(&self) -> usize {
        self.workers
            .values()
            .filter(|h| {
                matches!(
                    h.status,
                    WorkerStatus::Healthy | WorkerStatus::Degraded | WorkerStatus::Starting
                )
            })
            .count()
    }
}

/// Configuration for how aggressively to restart failed workers.
#[derive(Debug, Clone)]
pub struct RestartPolicy {
    pub max_restarts: u32,
    pub restart_delay_ms: u64,
    pub backoff_multiplier: f64,
    pub max_delay_ms: u64,
}

impl Default for RestartPolicy {
    fn default() -> Self {
        Self {
            max_restarts: 3,
            restart_delay_ms: 5000,
            backoff_multiplier: 2.0,
            max_delay_ms: 60_000,
        }
    }
}

/// Tracks per-worker restart counts and computes backoff delays.
pub struct RestartManager {
    policy: RestartPolicy,
    restart_counts: HashMap<String, u32>,
    last_restart: HashMap<String, Instant>,
}

impl RestartManager {
    pub fn new(policy: RestartPolicy) -> Self {
        Self {
            policy,
            restart_counts: HashMap::default(),
            last_restart: HashMap::default(),
        }
    }

    pub fn should_restart(&self, worker_id: &str) -> bool {
        let count = self.restart_counts.get(worker_id).copied().unwrap_or(0);
        count < self.policy.max_restarts
    }

    pub fn record_restart(&mut self, worker_id: &str) {
        let count = self
            .restart_counts
            .entry(worker_id.to_string())
            .or_insert(0);
        *count += 1;
        self.last_restart
            .insert(worker_id.to_string(), Instant::now());
    }

    pub fn get_delay(&self, worker_id: &str) -> Duration {
        let count = self.restart_counts.get(worker_id).copied().unwrap_or(0);
        let base = self.policy.restart_delay_ms as f64;
        let delay = base * self.policy.backoff_multiplier.powi(count as i32);
        let capped = delay.min(self.policy.max_delay_ms as f64);
        Duration::from_millis(capped as u64)
    }

    pub fn reset(&mut self, worker_id: &str) {
        self.restart_counts.remove(worker_id);
        self.last_restart.remove(worker_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- HealthMonitor tests --

    #[test]
    fn register_sets_starting_status() {
        let mut mon = HealthMonitor::new(60_000, 3);
        mon.register_worker("w-1".into());
        let w = mon.get_worker("w-1").unwrap();
        assert_eq!(w.status, WorkerStatus::Starting);
        assert_eq!(w.consecutive_errors, 0);
    }

    #[test]
    fn heartbeat_transitions_starting_to_healthy() {
        let mut mon = HealthMonitor::new(60_000, 3);
        mon.register_worker("w-1".into());
        mon.record_heartbeat("w-1");
        assert_eq!(mon.get_worker("w-1").unwrap().status, WorkerStatus::Healthy);
    }

    #[test]
    fn error_accumulation_reaches_unhealthy() {
        let mut mon = HealthMonitor::new(60_000, 3);
        mon.register_worker("w-1".into());
        mon.record_heartbeat("w-1");

        mon.record_error("w-1");
        assert_eq!(
            mon.get_worker("w-1").unwrap().status,
            WorkerStatus::Degraded
        );
        mon.record_error("w-1");
        assert_eq!(
            mon.get_worker("w-1").unwrap().status,
            WorkerStatus::Degraded
        );
        mon.record_error("w-1");
        assert_eq!(
            mon.get_worker("w-1").unwrap().status,
            WorkerStatus::Unhealthy
        );
    }

    #[test]
    fn success_resets_consecutive_errors() {
        let mut mon = HealthMonitor::new(60_000, 3);
        mon.register_worker("w-1".into());
        mon.record_heartbeat("w-1");
        mon.record_error("w-1");
        mon.record_error("w-1");
        assert_eq!(
            mon.get_worker("w-1").unwrap().status,
            WorkerStatus::Degraded
        );

        mon.record_success("w-1");
        assert_eq!(mon.get_worker("w-1").unwrap().status, WorkerStatus::Healthy);
        assert_eq!(mon.get_worker("w-1").unwrap().consecutive_errors, 0);
    }

    #[test]
    fn unresponsive_after_timeout() {
        let mut mon = HealthMonitor::new(10, 3);
        mon.register_worker("w-1".into());
        mon.record_heartbeat("w-1");

        std::thread::sleep(Duration::from_millis(15));
        let reports = mon.check_health();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].current_status, WorkerStatus::Unresponsive);
    }

    #[test]
    fn active_worker_count_excludes_dead_workers() {
        let mut mon = HealthMonitor::new(60_000, 3);
        mon.register_worker("w-1".into());
        mon.register_worker("w-2".into());
        mon.register_worker("w-3".into());
        assert_eq!(mon.active_worker_count(), 3);

        mon.remove_worker("w-2");
        assert_eq!(mon.active_worker_count(), 2);
    }

    #[test]
    fn remove_worker_cleans_up() {
        let mut mon = HealthMonitor::new(60_000, 3);
        mon.register_worker("w-1".into());
        mon.remove_worker("w-1");
        assert!(mon.get_worker("w-1").is_none());
        assert_eq!(mon.active_worker_count(), 0);
    }

    // -- RestartManager tests --

    #[test]
    fn should_restart_until_max() {
        let mut mgr = RestartManager::new(RestartPolicy {
            max_restarts: 2,
            ..Default::default()
        });
        assert!(mgr.should_restart("w-1"));
        mgr.record_restart("w-1");
        assert!(mgr.should_restart("w-1"));
        mgr.record_restart("w-1");
        assert!(!mgr.should_restart("w-1"));
    }

    #[test]
    fn backoff_delay_increases() {
        let mgr = RestartManager::new(RestartPolicy {
            max_restarts: 5,
            restart_delay_ms: 1000,
            backoff_multiplier: 2.0,
            max_delay_ms: 30_000,
        });
        // Before any restarts, delay = base
        assert_eq!(mgr.get_delay("w-1"), Duration::from_millis(1000));
    }

    #[test]
    fn backoff_delay_caps_at_max() {
        let mut mgr = RestartManager::new(RestartPolicy {
            max_restarts: 10,
            restart_delay_ms: 1000,
            backoff_multiplier: 10.0,
            max_delay_ms: 5000,
        });
        mgr.record_restart("w-1"); // 1000 * 10 = 10000 -> capped 5000
        assert_eq!(mgr.get_delay("w-1"), Duration::from_millis(5000));
    }

    #[test]
    fn reset_clears_restart_count() {
        let mut mgr = RestartManager::new(RestartPolicy {
            max_restarts: 2,
            ..Default::default()
        });
        mgr.record_restart("w-1");
        mgr.record_restart("w-1");
        assert!(!mgr.should_restart("w-1"));

        mgr.reset("w-1");
        assert!(mgr.should_restart("w-1"));
        assert_eq!(mgr.get_delay("w-1"), Duration::from_millis(5000));
    }
}
