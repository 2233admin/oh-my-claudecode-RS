use std::collections::{HashMap, HashSet, VecDeque};
use std::future::Future;
use std::time::{Duration, Instant};

pub type TaskId = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Priority {
    Critical = 0,
    High = 1,
    Normal = 2,
    Low = 3,
}

impl Priority {
    pub const COUNT: usize = 4;

    pub fn index(self) -> usize {
        self as usize
    }
}

#[derive(Debug, Clone)]
pub struct Task {
    pub id: TaskId,
    pub priority: Priority,
    pub dependencies: HashSet<TaskId>,
}

#[derive(Debug, Clone)]
pub struct TaskResult {
    pub id: TaskId,
    pub output: String,
}

#[derive(Debug, Clone)]
pub struct TaskGraph {
    tasks: HashMap<TaskId, Task>,
    /// reverse adjacency: maps a dependency to the tasks that depend on it
    dependents: HashMap<TaskId, HashSet<TaskId>>,
}

impl TaskGraph {
    pub fn new() -> Self {
        Self {
            tasks: HashMap::default(),
            dependents: HashMap::default(),
        }
    }

    pub fn add_task(&mut self, task: Task) {
        for dep in &task.dependencies {
            self.dependents
                .entry(dep.clone())
                .or_default()
                .insert(task.id.clone());
        }
        self.tasks.insert(task.id.clone(), task);
    }

    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }

    /// Returns the set of task ids that have no dependencies (source nodes).
    pub fn source_tasks(&self) -> HashSet<TaskId> {
        self.tasks
            .values()
            .filter(|t| t.dependencies.is_empty())
            .map(|t| t.id.clone())
            .collect()
    }

    /// Returns task ids whose dependencies are all in `completed`.
    pub fn ready_tasks(&self, completed: &HashSet<TaskId>) -> Vec<TaskId> {
        self.tasks
            .values()
            .filter(|t| !completed.contains(&t.id))
            .filter(|t| t.dependencies.iter().all(|dep| completed.contains(dep)))
            .map(|t| t.id.clone())
            .collect()
    }

    /// Validate the graph has no cycles. Returns Ok(()) or an error describing the cycle.
    pub fn validate(&self) -> Result<(), String> {
        let mut visited = HashSet::default();
        let mut in_stack = HashSet::default();

        for id in self.tasks.keys() {
            if !visited.contains(id) && self.has_cycle(id, &mut visited, &mut in_stack) {
                return Err(format!("cycle detected involving task {id}"));
            }
        }
        Ok(())
    }

    fn has_cycle(
        &self,
        id: &TaskId,
        visited: &mut HashSet<TaskId>,
        in_stack: &mut HashSet<TaskId>,
    ) -> bool {
        visited.insert(id.clone());
        in_stack.insert(id.clone());

        if let Some(task) = self.tasks.get(id) {
            for dep in &task.dependencies {
                if !visited.contains(dep) {
                    if self.has_cycle(dep, visited, in_stack) {
                        return true;
                    }
                } else if in_stack.contains(dep) {
                    return true;
                }
            }
        }

        in_stack.remove(id);
        false
    }

    /// Execute all tasks respecting dependency order. `executor` maps task id to a future.
    /// Tasks at the same "wave" (all deps satisfied) run concurrently.
    pub async fn execute<F, Fut>(&self, executor: F) -> Result<Vec<TaskResult>, String>
    where
        F: Fn(&TaskId) -> Fut,
        Fut: Future<Output = Result<String, String>> + Send + 'static,
    {
        self.validate()?;

        let mut completed: HashSet<TaskId> = HashSet::default();
        let mut results: Vec<TaskResult> = Vec::default();

        loop {
            let ready = self.ready_tasks(&completed);
            if ready.is_empty() {
                if completed.len() == self.tasks.len() {
                    break;
                }
                return Err("deadlock: no ready tasks but not all tasks completed".to_string());
            }

            let futures: Vec<_> = ready
                .iter()
                .map(|id| async {
                    let output = executor(id).await?;
                    Ok::<(TaskId, String), String>((id.clone(), output))
                })
                .collect();

            let wave_results = futures::future::try_join_all(futures).await?;

            for (id, output) in wave_results {
                completed.insert(id.clone());
                results.push(TaskResult { id, output });
            }
        }

        Ok(results)
    }
}

impl Default for TaskGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// Multi-level feedback queue scheduler with priority levels and aging.
///
/// Tasks in lower-priority queues get promoted after waiting longer than
/// `aging_threshold`, preventing starvation. `weights` control the relative
/// share of each level when dequeuing.
pub struct PriorityScheduler {
    queues: [VecDeque<Task>; Priority::COUNT],
    weights: [f64; Priority::COUNT],
    aging_threshold: Duration,
    aging_boost: f64,
    enqueue_times: HashMap<TaskId, Instant>,
    dequeue_counts: [usize; Priority::COUNT],
    total_dequeued: usize,
}

impl PriorityScheduler {
    pub fn new(weights: [f64; 4], aging_threshold: Duration, aging_boost: f64) -> Self {
        Self {
            queues: [
                VecDeque::default(), // Critical
                VecDeque::default(), // High
                VecDeque::default(), // Normal
                VecDeque::default(), // Low
            ],
            weights,
            aging_threshold,
            aging_boost,
            enqueue_times: HashMap::default(),
            dequeue_counts: [0; Priority::COUNT],
            total_dequeued: 0,
        }
    }

    /// Enqueue a task into its priority level.
    pub fn enqueue(&mut self, task: Task) {
        let id = task.id.clone();
        let idx = task.priority.index();
        self.queues[idx].push_back(task);
        self.enqueue_times.insert(id, Instant::now());
    }

    /// Returns the total number of tasks across all queues.
    pub fn pending_count(&self) -> usize {
        self.queues
            .iter()
            .map(std::collections::VecDeque::len)
            .sum()
    }

    /// Dequeue the next task, applying aging to promote long-waiting lower-priority tasks.
    /// Returns None if all queues are empty.
    pub fn dequeue(&mut self) -> Option<Task> {
        self.apply_aging();
        self.dequeue_by_weight()
    }

    fn apply_aging(&mut self) {
        let now = Instant::now();
        // Run aging repeatedly until no more promotions occur.
        // Tasks eligible for aging promote one level per pass, so multi-level
        // promotions (e.g., Low -> Critical) require multiple passes.
        loop {
            let promoted = self.apply_aging_inner(now);
            if promoted == 0 {
                break;
            }
        }
    }

    fn apply_aging_inner(&mut self, now: Instant) -> usize {
        // Collect tasks to promote from each non-highest level.
        // aging_boost acts as a multiplier on waiting time: effective_time = wait * boost.
        // Higher boost = faster aging (tasks reach threshold sooner).
        let mut to_promote: Vec<(usize, Vec<Task>)> = Vec::default();
        for level in 1..Priority::COUNT {
            let mut promoted = Vec::default();
            let mut kept = VecDeque::default();
            while let Some(task) = self.queues[level].pop_front() {
                let should_promote = self.enqueue_times.get(&task.id).is_some_and(|&t| {
                    let wait = now.duration_since(t);
                    let effective = wait.mul_f64(self.aging_boost);
                    effective >= self.aging_threshold
                });
                if should_promote {
                    promoted.push(task);
                } else {
                    kept.push_back(task);
                }
            }
            self.queues[level] = kept;
            if !promoted.is_empty() {
                to_promote.push((level, promoted));
            }
        }
        let mut total_promoted = 0;
        for (level, tasks) in to_promote {
            let target = level - 1;
            total_promoted += tasks.len();
            for mut task in tasks {
                task.priority = match target {
                    0 => Priority::Critical,
                    1 => Priority::High,
                    2 => Priority::Normal,
                    _ => Priority::Low,
                };
                self.queues[target].push_back(task);
            }
        }
        total_promoted
    }

    fn dequeue_by_weight(&mut self) -> Option<Task> {
        // Use strict priority order when weights are equal (default case)
        // This ensures critical tasks are always dequeued first
        let all_equal = self
            .weights
            .windows(2)
            .all(|w| (w[0] - w[1]).abs() < f64::EPSILON);
        if all_equal || self.weights.iter().all(|&w| w <= 0.0) {
            for level in 0..Priority::COUNT {
                if let Some(task) = self.queues[level].pop_front() {
                    self.enqueue_times.remove(&task.id);
                    return Some(task);
                }
            }
            return None;
        }

        // Find the non-empty queue whose actual share is furthest below its target share.
        // This implements weighted fair scheduling across priority levels.
        let total_weight: f64 = self.weights.iter().sum();
        let mut best_level: Option<usize> = None;
        let mut best_deficit = f64::NEG_INFINITY;

        for level in 0..Priority::COUNT {
            if self.queues[level].is_empty() {
                continue;
            }
            let target_ratio = self.weights[level] / total_weight;
            let actual_ratio = if self.total_dequeued > 0 {
                self.dequeue_counts[level] as f64 / self.total_dequeued as f64
            } else {
                0.0
            };
            let deficit = target_ratio - actual_ratio;
            if deficit > best_deficit {
                best_deficit = deficit;
                best_level = Some(level);
            }
        }

        if let Some(level) = best_level {
            let task = self.queues[level].pop_front().unwrap();
            self.enqueue_times.remove(&task.id);
            self.dequeue_counts[level] += 1;
            self.total_dequeued += 1;
            Some(task)
        } else {
            None
        }
    }

    /// Peek at the number of tasks in each priority level.
    pub fn level_counts(&self) -> [usize; Priority::COUNT] {
        [
            self.queues[0].len(),
            self.queues[1].len(),
            self.queues[2].len(),
            self.queues[3].len(),
        ]
    }
}

impl Default for PriorityScheduler {
    fn default() -> Self {
        Self {
            queues: Default::default(),
            weights: [1.0; Priority::COUNT],
            aging_threshold: Duration::from_secs(60),
            aging_boost: 0.1,
            enqueue_times: HashMap::default(),
            dequeue_counts: [0; Priority::COUNT],
            total_dequeued: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(id: &str, priority: Priority, deps: &[&str]) -> Task {
        Task {
            id: id.to_string(),
            priority,
            dependencies: deps.iter().map(std::string::ToString::to_string).collect(),
        }
    }

    #[test]
    fn empty_graph_has_no_sources() {
        let g = TaskGraph::default();
        assert!(g.source_tasks().is_empty());
        assert!(g.ready_tasks(&HashSet::default()).is_empty());
    }

    #[test]
    fn single_task_is_source_and_ready() {
        let mut g = TaskGraph::default();
        g.add_task(task("a", Priority::Normal, &[]));
        assert_eq!(g.source_tasks(), HashSet::from(["a".to_string()]));
        assert_eq!(g.ready_tasks(&HashSet::default()), vec!["a".to_string()]);
    }

    #[test]
    fn dependent_task_not_ready_until_dependency_completed() {
        let mut g = TaskGraph::new();
        g.add_task(task("a", Priority::Normal, &[]));
        g.add_task(task("b", Priority::Normal, &["a"]));

        let ready = g.ready_tasks(&HashSet::default());
        assert_eq!(ready, vec!["a".to_string()]);

        let mut completed = HashSet::default();
        completed.insert("a".to_string());
        let ready = g.ready_tasks(&completed);
        assert_eq!(ready, vec!["b".to_string()]);
    }

    #[test]
    fn diamond_dag_ready_waves() {
        //   a
        //  / \
        // b   c
        //  \ /
        //   d
        let mut g = TaskGraph::default();
        g.add_task(task("a", Priority::Normal, &[]));
        g.add_task(task("b", Priority::Normal, &["a"]));
        g.add_task(task("c", Priority::Normal, &["a"]));
        g.add_task(task("d", Priority::Normal, &["b", "c"]));

        // wave 1: a
        let w1: HashSet<_> = g.ready_tasks(&HashSet::default()).into_iter().collect();
        assert_eq!(w1, HashSet::from(["a".to_string()]));

        // wave 2: b, c
        let mut done: HashSet<_> = ["a"].iter().map(std::string::ToString::to_string).collect();
        let w2: HashSet<_> = g.ready_tasks(&done).into_iter().collect();
        assert_eq!(w2, HashSet::from(["b".to_string(), "c".to_string()]));

        // wave 3: d (after b+c)
        done.extend(["b", "c"].iter().map(std::string::ToString::to_string));
        let w3: HashSet<_> = g.ready_tasks(&done).into_iter().collect();
        assert_eq!(w3, HashSet::from(["d".to_string()]));
    }

    #[test]
    fn cycle_detection_works() {
        let mut g = TaskGraph::default();
        g.add_task(task("a", Priority::Normal, &["b"]));
        g.add_task(task("b", Priority::Normal, &["a"]));
        assert!(g.validate().unwrap_err().contains("cycle"));
    }

    #[test]
    fn validate_passes_for_acyclic_graph() {
        let mut g = TaskGraph::default();
        g.add_task(task("a", Priority::Normal, &[]));
        g.add_task(task("b", Priority::Normal, &["a"]));
        g.add_task(task("c", Priority::Normal, &["a"]));
        g.add_task(task("d", Priority::Normal, &["b", "c"]));
        assert!(g.validate().is_ok());
    }

    #[test]
    fn execute_runs_in_dependency_order() {
        let mut g = TaskGraph::default();
        g.add_task(task("a", Priority::Critical, &[]));
        g.add_task(task("b", Priority::High, &["a"]));
        g.add_task(task("c", Priority::Normal, &["a"]));
        g.add_task(task("d", Priority::Low, &["b", "c"]));

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let results = rt
            .block_on(g.execute(|id| {
                let id = id.clone();
                async move { Ok(format!("done:{id}")) }
            }))
            .unwrap();

        assert_eq!(results.len(), 4);

        let order: Vec<_> = results.iter().map(|r| r.id.as_str()).collect();
        let a_pos = order.iter().position(|x| *x == "a").unwrap();
        let b_pos = order.iter().position(|x| *x == "b").unwrap();
        let c_pos = order.iter().position(|x| *x == "c").unwrap();
        let d_pos = order.iter().position(|x| *x == "d").unwrap();
        assert!(a_pos < b_pos);
        assert!(a_pos < c_pos);
        assert!(b_pos < d_pos);
        assert!(c_pos < d_pos);
    }

    #[test]
    fn execute_propagates_errors() {
        let mut g = TaskGraph::default();
        g.add_task(task("a", Priority::Normal, &[]));
        g.add_task(task("b", Priority::Normal, &[]));

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let err = rt
            .block_on(g.execute(|id| {
                let id = id.clone();
                async move {
                    if id == "b" {
                        Err("task b failed".to_string())
                    } else {
                        Ok("ok".to_string())
                    }
                }
            }))
            .unwrap_err();
        assert!(err.contains("task b failed"));
    }

    #[test]
    fn execute_rejects_cyclic_graph() {
        let mut g = TaskGraph::default();
        g.add_task(task("a", Priority::Normal, &["b"]));
        g.add_task(task("b", Priority::Normal, &["a"]));

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let err = rt
            .block_on(g.execute(|_| async { Ok("ok".to_string()) }))
            .unwrap_err();
        assert!(err.contains("cycle"));
    }

    #[test]
    fn priority_scheduler_basic_dequeue_order() {
        let mut sched = PriorityScheduler::default();
        sched.enqueue(task("low", Priority::Low, &[]));
        sched.enqueue(task("critical", Priority::Critical, &[]));
        sched.enqueue(task("normal", Priority::Normal, &[]));
        sched.enqueue(task("high", Priority::High, &[]));

        assert_eq!(sched.dequeue().unwrap().id, "critical");
        assert_eq!(sched.dequeue().unwrap().id, "high");
        assert_eq!(sched.dequeue().unwrap().id, "normal");
        assert_eq!(sched.dequeue().unwrap().id, "low");
        assert!(sched.dequeue().is_none());
    }

    #[test]
    fn priority_scheduler_level_counts() {
        let mut sched = PriorityScheduler::default();
        sched.enqueue(task("a", Priority::Critical, &[]));
        sched.enqueue(task("b", Priority::Critical, &[]));
        sched.enqueue(task("c", Priority::Normal, &[]));
        sched.enqueue(task("d", Priority::Low, &[]));

        let counts = sched.level_counts();
        assert_eq!(counts, [2, 0, 1, 1]);
        assert_eq!(sched.pending_count(), 4);
    }

    #[test]
    fn priority_scheduler_aging_promotes() {
        let mut sched = PriorityScheduler::new(
            [1.0; 4],                 // equal weights for strict priority
            Duration::from_millis(0), // instant aging
            1.0,                      // no boost multiplier
        );
        sched.enqueue(task("low1", Priority::Low, &[]));
        sched.enqueue(task("low2", Priority::Low, &[]));
        sched.enqueue(task("normal1", Priority::Normal, &[]));

        // With zero aging threshold, all items promote to Critical (level 0).
        // Since aging promotes higher-level items first, "normal1" reaches
        // level 0 before "low1" and "low2". With equal weights (strict priority),
        // all items are at the same level and dequeued in FIFO order at that level.
        let first = sched.dequeue().unwrap();
        assert_eq!(first.id, "normal1"); // first to reach level 0 via aging
    }

    #[test]
    fn priority_scheduler_empty_returns_none() {
        let mut sched = PriorityScheduler::default();
        assert!(sched.dequeue().is_none());
    }

    #[test]
    fn graph_builder_methods() {
        let mut g = TaskGraph::default();
        assert_eq!(g.task_count(), 0);
        g.add_task(task("x", Priority::High, &[]));
        assert_eq!(g.task_count(), 1);
    }
}
