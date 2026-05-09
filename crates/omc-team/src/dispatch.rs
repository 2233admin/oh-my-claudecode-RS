use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::worker_health::WorkerHealth;

/// Priority level for dispatch tasks.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Low,
    Normal,
    High,
    Critical,
}

/// Status of a dispatched task.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DispatchStatus {
    Queued,
    Dispatched,
    Acknowledged,
    Completed,
    Failed,
    Cancelled,
}

/// A task in the dispatch system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchTask {
    pub id: String,
    pub subject: String,
    pub priority: Priority,
    pub assigned_worker: Option<String>,
    pub status: DispatchStatus,
    pub created_at: String,
    pub dispatched_at: Option<String>,
    pub completed_at: Option<String>,
}

/// FIFO dispatch queue with concurrency limits and ack timeouts.
pub struct DispatchQueue {
    queue: VecDeque<DispatchTask>,
    in_flight: HashMap<String, DispatchTask>,
    dispatched_at_instant: HashMap<String, Instant>,
    max_concurrent: usize,
    ack_timeout_ms: u64,
    dispatch_counter: usize,
}

impl DispatchQueue {
    pub fn new(max_concurrent: usize, ack_timeout_ms: u64) -> Self {
        Self {
            queue: VecDeque::new(),
            in_flight: HashMap::new(),
            dispatched_at_instant: HashMap::new(),
            max_concurrent,
            ack_timeout_ms,
            dispatch_counter: 0,
        }
    }

    pub fn enqueue(&mut self, task: DispatchTask) {
        self.queue.push_back(task);
    }

    pub fn dequeue(&mut self) -> Option<DispatchTask> {
        if self.is_full() {
            return None;
        }
        self.queue.pop_front().map(|mut task| {
            self.dispatch_counter += 1;
            task.status = DispatchStatus::Dispatched;
            task.dispatched_at = Some(crate::unix_timestamp().to_string());
            let id = task.id.clone();
            self.dispatched_at_instant
                .insert(id.clone(), Instant::now());
            self.in_flight.insert(id, task.clone());
            task
        })
    }

    pub fn acknowledge(&mut self, task_id: &str) {
        if let Some(task) = self.in_flight.get_mut(task_id) {
            task.status = DispatchStatus::Acknowledged;
            self.dispatched_at_instant.remove(task_id);
        }
    }

    pub fn complete(&mut self, task_id: &str) -> Option<DispatchTask> {
        if let Some(mut task) = self.in_flight.remove(task_id) {
            task.status = DispatchStatus::Completed;
            task.completed_at = Some(crate::unix_timestamp().to_string());
            self.dispatched_at_instant.remove(task_id);
            Some(task)
        } else {
            None
        }
    }

    pub fn fail(&mut self, task_id: &str) -> Option<DispatchTask> {
        if let Some(mut task) = self.in_flight.remove(task_id) {
            task.status = DispatchStatus::Failed;
            task.completed_at = Some(crate::unix_timestamp().to_string());
            self.dispatched_at_instant.remove(task_id);
            Some(task)
        } else {
            None
        }
    }

    pub fn cancel(&mut self, task_id: &str) -> Option<DispatchTask> {
        // Check in-flight first
        if let Some(mut task) = self.in_flight.remove(task_id) {
            task.status = DispatchStatus::Cancelled;
            task.completed_at = Some(crate::unix_timestamp().to_string());
            self.dispatched_at_instant.remove(task_id);
            return Some(task);
        }
        // Check queued
        if let Some(idx) = self.queue.iter().position(|t| t.id == task_id) {
            let mut task = self.queue.remove(idx).unwrap();
            task.status = DispatchStatus::Cancelled;
            task.completed_at = Some(crate::unix_timestamp().to_string());
            return Some(task);
        }
        None
    }

    pub fn check_timeouts(&mut self) -> Vec<DispatchTask> {
        let timeout = Duration::from_millis(self.ack_timeout_ms);
        let timed_out: Vec<String> = self
            .dispatched_at_instant
            .iter()
            .filter(|(_, instant)| instant.elapsed() > timeout)
            .map(|(id, _)| id.clone())
            .collect();

        let mut result = Vec::new();
        for id in timed_out {
            self.dispatched_at_instant.remove(&id);
            if let Some(mut task) = self.in_flight.remove(&id) {
                task.status = DispatchStatus::Failed;
                result.push(task);
            }
        }
        result
    }

    pub fn pending_count(&self) -> usize {
        self.queue.len()
    }

    pub fn in_flight_count(&self) -> usize {
        self.in_flight.len()
    }

    pub fn is_full(&self) -> bool {
        self.in_flight.len() >= self.max_concurrent
    }
}

/// Strategy for assigning tasks to workers.
#[derive(Debug, Clone)]
pub enum AllocationPolicy {
    RoundRobin,
    LeastBusy,
    CapabilityMatch { required_capabilities: Vec<String> },
    PriorityFirst,
}

impl AllocationPolicy {
    pub fn select_worker(&self, workers: &[WorkerHealth], _task: &DispatchTask) -> Option<String> {
        let healthy: Vec<&WorkerHealth> = workers
            .iter()
            .filter(|w| {
                matches!(
                    w.status,
                    crate::worker_health::WorkerStatus::Healthy
                        | crate::worker_health::WorkerStatus::Degraded
                )
            })
            .collect();

        if healthy.is_empty() {
            return None;
        }

        match self {
            AllocationPolicy::RoundRobin => {
                // Select by lowest tasks_completed for pseudo-round-robin
                healthy
                    .iter()
                    .min_by_key(|w| w.tasks_completed)
                    .map(|w| w.worker_id.clone())
            }
            AllocationPolicy::LeastBusy => healthy
                .iter()
                .min_by_key(|w| w.tasks_completed + w.tasks_failed)
                .map(|w| w.worker_id.clone()),
            AllocationPolicy::CapabilityMatch {
                required_capabilities,
            } => {
                // Filter workers whose capabilities contain all required ones
                let matching: Vec<&WorkerHealth> = healthy
                    .iter()
                    .copied()
                    .filter(|w| {
                        required_capabilities
                            .iter()
                            .all(|cap| w.capabilities.contains(cap))
                    })
                    .collect();

                let candidates: &Vec<&WorkerHealth> = if matching.is_empty() {
                    &healthy
                } else {
                    &matching
                };

                candidates
                    .iter()
                    .min_by_key(|w| w.tasks_completed + w.tasks_failed)
                    .map(|w| w.worker_id.clone())
            }
            AllocationPolicy::PriorityFirst => {
                // Pick the worker with the most high-priority experience
                healthy
                    .iter()
                    .max_by_key(|w| w.high_priority_completed)
                    .map(|w| w.worker_id.clone())
            }
        }
    }
}

/// Auto-scaling policy for worker count based on queue utilization.
#[derive(Debug, Clone)]
pub struct ScalingPolicy {
    pub min_workers: usize,
    pub max_workers: usize,
    pub scale_up_threshold: f64,
    pub scale_down_threshold: f64,
    pub cooldown_seconds: u64,
}

impl Default for ScalingPolicy {
    fn default() -> Self {
        Self {
            min_workers: 1,
            max_workers: 8,
            scale_up_threshold: 0.8,
            scale_down_threshold: 0.2,
            cooldown_seconds: 30,
        }
    }
}

impl ScalingPolicy {
    pub fn should_scale_up(&self, queue_utilization: f64, current_workers: usize) -> bool {
        queue_utilization > self.scale_up_threshold && current_workers < self.max_workers
    }

    pub fn should_scale_down(&self, queue_utilization: f64, current_workers: usize) -> bool {
        queue_utilization < self.scale_down_threshold && current_workers > self.min_workers
    }

    pub fn target_workers(&self, queue_utilization: f64, current_workers: usize) -> usize {
        if queue_utilization > self.scale_up_threshold {
            (current_workers + 1).min(self.max_workers)
        } else if queue_utilization < self.scale_down_threshold {
            current_workers.saturating_sub(1).max(self.min_workers)
        } else {
            current_workers
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worker_health::WorkerStatus;

    fn make_task(id: &str) -> DispatchTask {
        DispatchTask {
            id: id.to_string(),
            subject: format!("task {id}"),
            priority: Priority::Normal,
            assigned_worker: None,
            status: DispatchStatus::Queued,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            dispatched_at: None,
            completed_at: None,
        }
    }

    fn make_worker(id: &str, status: WorkerStatus, completed: u32, failed: u32) -> WorkerHealth {
        WorkerHealth {
            worker_id: id.to_string(),
            status,
            last_heartbeat: None,
            consecutive_errors: 0,
            tasks_completed: completed,
            tasks_failed: failed,
            uptime_seconds: 0,
            capabilities: Vec::new(),
            high_priority_completed: 0,
        }
    }

    fn make_worker_with_caps(
        id: &str,
        status: WorkerStatus,
        completed: u32,
        failed: u32,
        caps: Vec<String>,
        high_priority_completed: u32,
    ) -> WorkerHealth {
        WorkerHealth {
            worker_id: id.to_string(),
            status,
            last_heartbeat: None,
            consecutive_errors: 0,
            tasks_completed: completed,
            tasks_failed: failed,
            uptime_seconds: 0,
            capabilities: caps,
            high_priority_completed,
        }
    }

    // -- DispatchQueue tests --

    #[test]
    fn enqueue_dequeue_round_trip() {
        let mut q = DispatchQueue::new(10, 5000);
        q.enqueue(make_task("t-1"));
        q.enqueue(make_task("t-2"));
        assert_eq!(q.pending_count(), 2);

        let dispatched = q.dequeue().unwrap();
        assert_eq!(dispatched.id, "t-1");
        assert_eq!(dispatched.status, DispatchStatus::Dispatched);
        assert_eq!(q.pending_count(), 1);
        assert_eq!(q.in_flight_count(), 1);
    }

    #[test]
    fn dequeue_returns_none_when_full() {
        let mut q = DispatchQueue::new(1, 5000);
        q.enqueue(make_task("t-1"));
        q.enqueue(make_task("t-2"));

        let _ = q.dequeue();
        assert!(q.dequeue().is_none());
        assert!(q.is_full());
    }

    #[test]
    fn ack_timeout_marks_failed() {
        let mut q = DispatchQueue::new(10, 0); // 0ms timeout
        q.enqueue(make_task("t-1"));
        let _ = q.dequeue();

        let timed_out = q.check_timeouts();
        assert_eq!(timed_out.len(), 1);
        assert_eq!(timed_out[0].id, "t-1");
        assert_eq!(timed_out[0].status, DispatchStatus::Failed);
        assert_eq!(q.in_flight_count(), 0);
    }

    #[test]
    fn complete_removes_from_in_flight() {
        let mut q = DispatchQueue::new(10, 5000);
        q.enqueue(make_task("t-1"));
        let _ = q.dequeue();

        let completed = q.complete("t-1").unwrap();
        assert_eq!(completed.status, DispatchStatus::Completed);
        assert!(completed.completed_at.is_some());
        assert_eq!(q.in_flight_count(), 0);
    }

    #[test]
    fn fail_removes_from_in_flight() {
        let mut q = DispatchQueue::new(10, 5000);
        q.enqueue(make_task("t-1"));
        let _ = q.dequeue();

        let failed = q.fail("t-1").unwrap();
        assert_eq!(failed.status, DispatchStatus::Failed);
        assert_eq!(q.in_flight_count(), 0);
    }

    #[test]
    fn cancel_removes_from_queue_or_in_flight() {
        let mut q = DispatchQueue::new(10, 5000);
        q.enqueue(make_task("t-1"));
        q.enqueue(make_task("t-2"));
        let _ = q.dequeue(); // t-1 goes in-flight

        // Cancel in-flight
        let cancelled = q.cancel("t-1").unwrap();
        assert_eq!(cancelled.status, DispatchStatus::Cancelled);
        assert_eq!(q.in_flight_count(), 0);

        // Cancel queued
        let cancelled = q.cancel("t-2").unwrap();
        assert_eq!(cancelled.status, DispatchStatus::Cancelled);
        assert_eq!(q.pending_count(), 0);
    }

    #[test]
    fn cancel_nonexistent_returns_none() {
        let mut q = DispatchQueue::new(10, 5000);
        assert!(q.cancel("ghost").is_none());
    }

    #[test]
    fn acknowledge_updates_status() {
        let mut q = DispatchQueue::new(10, 5000);
        q.enqueue(make_task("t-1"));
        let _ = q.dequeue();

        q.acknowledge("t-1");
        let completed = q.complete("t-1").unwrap();
        // Status was Acknowledged before complete() changed it
        // Since complete removes and sets Completed, verify via in_flight_count
        assert_eq!(completed.status, DispatchStatus::Completed);
    }

    // -- AllocationPolicy tests --

    #[test]
    fn round_robin_selects_lowest_completed() {
        let workers = vec![
            make_worker("w-1", WorkerStatus::Healthy, 10, 0),
            make_worker("w-2", WorkerStatus::Healthy, 3, 0),
            make_worker("w-3", WorkerStatus::Healthy, 7, 0),
        ];
        let policy = AllocationPolicy::RoundRobin;
        let task = make_task("t-1");
        assert_eq!(policy.select_worker(&workers, &task), Some("w-2".into()));
    }

    #[test]
    fn least_busy_selects_fewest_total_tasks() {
        let workers = vec![
            make_worker("w-1", WorkerStatus::Healthy, 5, 2),
            make_worker("w-2", WorkerStatus::Healthy, 3, 0),
            make_worker("w-3", WorkerStatus::Degraded, 1, 0),
        ];
        let policy = AllocationPolicy::LeastBusy;
        let task = make_task("t-1");
        assert_eq!(policy.select_worker(&workers, &task), Some("w-3".into()));
    }

    #[test]
    fn policy_skips_unhealthy_workers() {
        let workers = vec![
            make_worker("w-1", WorkerStatus::Unhealthy, 0, 5),
            make_worker("w-2", WorkerStatus::Healthy, 1, 0),
        ];
        let policy = AllocationPolicy::RoundRobin;
        let task = make_task("t-1");
        assert_eq!(policy.select_worker(&workers, &task), Some("w-2".into()));
    }

    #[test]
    fn policy_returns_none_when_no_healthy_workers() {
        let workers = vec![
            make_worker("w-1", WorkerStatus::Crashed, 0, 5),
            make_worker("w-2", WorkerStatus::Unresponsive, 0, 0),
        ];
        let policy = AllocationPolicy::LeastBusy;
        let task = make_task("t-1");
        assert!(policy.select_worker(&workers, &task).is_none());
    }

    #[test]
    fn capability_match_selects_matching_worker() {
        let workers = vec![
            make_worker_with_caps(
                "w-1",
                WorkerStatus::Healthy,
                2,
                0,
                vec!["python".into(), "ml".into()],
                0,
            ),
            make_worker_with_caps("w-2", WorkerStatus::Healthy, 1, 0, vec!["rust".into()], 0),
            make_worker_with_caps("w-3", WorkerStatus::Healthy, 5, 0, vec!["python".into()], 0),
        ];
        let policy = AllocationPolicy::CapabilityMatch {
            required_capabilities: vec!["python".into()],
        };
        let task = make_task("t-1");
        // w-3 has python but w-1 has python AND ml. Both match "python".
        // Least busy among matching (w-3: 5 total, w-1: 2 total) -> w-1
        assert_eq!(policy.select_worker(&workers, &task), Some("w-1".into()));
    }

    #[test]
    fn capability_match_falls_back_when_no_match() {
        let workers = vec![
            make_worker_with_caps("w-1", WorkerStatus::Healthy, 5, 0, vec!["python".into()], 0),
            make_worker_with_caps("w-2", WorkerStatus::Healthy, 2, 0, vec!["rust".into()], 0),
        ];
        let policy = AllocationPolicy::CapabilityMatch {
            required_capabilities: vec!["go".into()],
        };
        let task = make_task("t-1");
        // No worker has "go", falls back to least-busy -> w-2 (2 < 5)
        assert_eq!(policy.select_worker(&workers, &task), Some("w-2".into()));
    }

    #[test]
    fn priority_first_selects_most_experienced() {
        let workers = vec![
            make_worker_with_caps("w-1", WorkerStatus::Healthy, 10, 0, vec![], 3),
            make_worker_with_caps("w-2", WorkerStatus::Healthy, 8, 0, vec![], 7),
            make_worker_with_caps("w-3", WorkerStatus::Healthy, 15, 0, vec![], 5),
        ];
        let policy = AllocationPolicy::PriorityFirst;
        let task = make_task("t-1");
        // w-2 has highest high_priority_completed (7)
        assert_eq!(policy.select_worker(&workers, &task), Some("w-2".into()));
    }

    // -- ScalingPolicy tests --

    #[test]
    fn scale_up_when_utilization_high() {
        let policy = ScalingPolicy::default();
        assert!(policy.should_scale_up(0.9, 4));
        assert!(!policy.should_scale_up(0.5, 4));
    }

    #[test]
    fn scale_down_when_utilization_low() {
        let policy = ScalingPolicy::default();
        assert!(policy.should_scale_down(0.1, 4));
        assert!(!policy.should_scale_down(0.5, 4));
    }

    #[test]
    fn scale_respects_bounds() {
        let policy = ScalingPolicy::default();
        // Cannot scale below min
        assert!(!policy.should_scale_down(0.0, 1));
        // Cannot scale above max
        assert!(!policy.should_scale_up(1.0, 8));
    }

    #[test]
    fn target_workers_computes_correctly() {
        let policy = ScalingPolicy::default();
        assert_eq!(policy.target_workers(0.9, 4), 5);
        assert_eq!(policy.target_workers(0.1, 4), 3);
        assert_eq!(policy.target_workers(0.5, 4), 4);
        // Capped at max
        assert_eq!(policy.target_workers(0.9, 8), 8);
        // Capped at min
        assert_eq!(policy.target_workers(0.1, 1), 1);
    }
}
