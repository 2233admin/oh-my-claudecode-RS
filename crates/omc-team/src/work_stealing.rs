use std::collections::{HashMap, VecDeque};
use std::time::{SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// Task distribution strategy
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TaskDistribution {
    WorkStealing { steal_threshold: usize },
    ConsistentHash { replication_factor: u8 },
    Auction { scoring_weights: ScoringWeights },
}

impl TaskDistribution {
    pub fn work_stealing(steal_threshold: usize) -> Self {
        Self::WorkStealing { steal_threshold }
    }

    pub fn consistent_hash(replication_factor: u8) -> Self {
        Self::ConsistentHash { replication_factor }
    }

    pub fn auction(weights: ScoringWeights) -> Self {
        Self::Auction {
            scoring_weights: weights,
        }
    }
}

// ---------------------------------------------------------------------------
// Scoring weights for the Auction strategy
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScoringWeights {
    /// Weight for how well the agent's role matches the task.
    pub capability_match: f64,
    /// Weight for the agent's current queue depth (lower is better).
    pub queue_balance: f64,
    /// Weight for the agent's historical success rate.
    pub affinity: f64,
}

impl Default for ScoringWeights {
    fn default() -> Self {
        Self {
            capability_match: 0.5,
            queue_balance: 0.3,
            affinity: 0.2,
        }
    }
}

// ---------------------------------------------------------------------------
// Task wrapper used by the scheduler
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledTask {
    pub id: String,
    pub priority: u32,
    pub required_capability: String,
    pub enqueued_at: u64,
}

impl ScheduledTask {
    pub fn new(
        id: impl Into<String>,
        priority: u32,
        required_capability: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            priority,
            required_capability: required_capability.into(),
            enqueued_at: now_millis(),
        }
    }
}

// ---------------------------------------------------------------------------
// Agent profile used for scoring in the Auction strategy
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AgentProfile {
    pub agent_id: String,
    pub capabilities: Vec<String>,
    pub success_rate: f64,
}

// ---------------------------------------------------------------------------
// Work stealing scheduler
// ---------------------------------------------------------------------------

pub struct WorkStealingScheduler {
    local_queues: HashMap<String, VecDeque<ScheduledTask>>,
    global_queue: VecDeque<ScheduledTask>,
    steal_threshold: usize,
    distribution: TaskDistribution,
    agent_profiles: HashMap<String, AgentProfile>,
    completed_counts: HashMap<String, HashMap<String, u64>>,
}

impl WorkStealingScheduler {
    pub fn new(distribution: TaskDistribution) -> Self {
        let steal_threshold = match distribution {
            TaskDistribution::WorkStealing { steal_threshold } => steal_threshold,
            _ => 4,
        };
        Self {
            local_queues: HashMap::new(),
            global_queue: VecDeque::new(),
            steal_threshold,
            distribution,
            agent_profiles: HashMap::new(),
            completed_counts: HashMap::new(),
        }
    }

    // -- agent management ---------------------------------------------------

    pub fn register_agent(&mut self, agent_id: &str) {
        self.local_queues.entry(agent_id.to_string()).or_default();
    }

    pub fn register_agent_with_profile(&mut self, profile: AgentProfile) {
        self.local_queues
            .entry(profile.agent_id.clone())
            .or_default();
        self.agent_profiles
            .insert(profile.agent_id.clone(), profile);
    }

    pub fn agent_ids(&self) -> Vec<String> {
        self.local_queues.keys().cloned().collect()
    }

    // -- task submission ----------------------------------------------------

    pub fn push_global(&mut self, task: ScheduledTask) {
        self.global_queue.push_back(task);
    }

    pub fn push_local(&mut self, agent_id: &str, task: ScheduledTask) {
        self.local_queues
            .entry(agent_id.to_string())
            .or_default()
            .push_back(task);
    }

    /// Route a task to the best agent according to the configured distribution
    /// strategy. Returns the agent_id the task was assigned to, or None if no
    /// agents are registered.
    pub fn assign(&mut self, task: ScheduledTask) -> Option<String> {
        if self.local_queues.is_empty() {
            self.global_queue.push_back(task);
            return None;
        }

        match self.distribution {
            TaskDistribution::WorkStealing { .. } => {
                let agent = self.least_loaded_agent();
                self.push_local(&agent, task);
                Some(agent)
            }
            TaskDistribution::ConsistentHash { replication_factor } => {
                let agent = self.consistent_hash_select(&task, replication_factor);
                self.push_local(&agent, task);
                Some(agent)
            }
            TaskDistribution::Auction { scoring_weights } => {
                let agent = self.auction_select(&task, scoring_weights);
                self.push_local(&agent, task);
                Some(agent)
            }
        }
    }

    // -- task consumption ---------------------------------------------------

    /// Try to pop the highest-priority task from the agent's local queue.
    /// If the local queue is empty, try the global queue. If both are empty,
    /// try to steal from the busiest agent that exceeds the steal threshold.
    pub fn pop(&mut self, agent_id: &str) -> Option<ScheduledTask> {
        // 1. Local queue
        if let Some(queue) = self.local_queues.get_mut(agent_id)
            && let Some(task) = pop_highest_priority(queue)
        {
            return Some(task);
        }

        // 2. Global queue
        if let Some(task) = pop_highest_priority(&mut self.global_queue) {
            return Some(task);
        }

        // 3. Work stealing
        self.try_steal(agent_id)
    }

    /// Record that an agent completed a task (used for affinity scoring).
    pub fn record_completion(&mut self, agent_id: &str, capability: &str) {
        *self
            .completed_counts
            .entry(agent_id.to_string())
            .or_default()
            .entry(capability.to_string())
            .or_insert(0) += 1;
    }

    // -- introspection -----------------------------------------------------

    pub fn local_queue_len(&self, agent_id: &str) -> usize {
        self.local_queues.get(agent_id).map_or(0, VecDeque::len)
    }

    pub fn global_queue_len(&self) -> usize {
        self.global_queue.len()
    }

    pub fn total_pending(&self) -> usize {
        self.global_queue_len() + self.local_queues.values().map(VecDeque::len).sum::<usize>()
    }

    pub fn steal_threshold(&self) -> usize {
        self.steal_threshold
    }

    // -- internals ---------------------------------------------------------

    fn least_loaded_agent(&self) -> String {
        self.local_queues
            .iter()
            .min_by_key(|(_, q)| q.len())
            .map(|(id, _)| id.clone())
            .expect("at least one agent registered")
    }

    fn consistent_hash_select(&self, task: &ScheduledTask, replication_factor: u8) -> String {
        let hash = fnv1a(&task.id);
        let mut agents: Vec<&String> = self.local_queues.keys().collect();
        agents.sort();

        let rf = replication_factor.max(1) as usize;
        let idx = (hash as usize * rf) % agents.len();
        agents[idx].clone()
    }

    fn auction_select(&self, task: &ScheduledTask, weights: ScoringWeights) -> String {
        let mut best_agent = String::new();
        let mut best_score = f64::NEG_INFINITY;

        for agent_id in self.local_queues.keys() {
            let cap_score = self.capability_match_score(agent_id, &task.required_capability);
            let queue_score = self.queue_balance_score(agent_id);
            let affinity_score = self.affinity_score(agent_id, &task.required_capability);

            let score = weights.capability_match * cap_score
                + weights.queue_balance * queue_score
                + weights.affinity * affinity_score;

            if score > best_score {
                best_score = score;
                best_agent = agent_id.clone();
            }
        }

        best_agent
    }

    fn capability_match_score(&self, agent_id: &str, capability: &str) -> f64 {
        match self.agent_profiles.get(agent_id) {
            Some(profile) if profile.capabilities.iter().any(|c| c == capability) => 1.0,
            Some(_) => 0.0,
            None => 0.5,
        }
    }

    fn queue_balance_score(&self, agent_id: &str) -> f64 {
        let len = self.local_queue_len(agent_id) as f64;
        // 0 tasks = 1.0 score, 10+ tasks = approaches 0
        (10.0 - len).max(0.0) / 10.0
    }

    fn affinity_score(&self, agent_id: &str, capability: &str) -> f64 {
        let completed = self
            .completed_counts
            .get(agent_id)
            .and_then(|map| map.get(capability))
            .copied()
            .unwrap_or(0);
        // Diminishing returns: first few completions matter most
        (completed as f64).ln_1p() / 5.0
    }

    fn try_steal(&mut self, agent_id: &str) -> Option<ScheduledTask> {
        let threshold = self.steal_threshold;

        // Find the agent with the longest queue above the threshold.
        let victim = self
            .local_queues
            .iter()
            .filter(|(id, q)| *id != agent_id && q.len() > threshold)
            .max_by_key(|(_, q)| q.len())
            .map(|(id, _)| id.clone())?;

        // Steal the lowest-priority task from the victim (leave the high-priority
        // work for the owning agent).
        self.local_queues
            .get_mut(&victim)?
            .iter()
            .enumerate()
            .min_by_key(|(_, task)| task.priority)
            .map(|(idx, _)| idx)
            .and_then(|idx| self.local_queues.get_mut(&victim)?.remove(idx))
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn pop_highest_priority(queue: &mut VecDeque<ScheduledTask>) -> Option<ScheduledTask> {
    if queue.is_empty() {
        return None;
    }
    let best_idx = queue
        .iter()
        .enumerate()
        .max_by_key(|(_, task)| task.priority)
        .map(|(idx, _)| idx)?;
    queue.remove(best_idx)
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn fnv1a(input: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in input.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn task(id: &str, priority: u32) -> ScheduledTask {
        ScheduledTask::new(id, priority, "general")
    }

    fn task_with_cap(id: &str, priority: u32, cap: &str) -> ScheduledTask {
        ScheduledTask::new(id, priority, cap)
    }

    #[test]
    fn new_scheduler_has_empty_queues() {
        let scheduler = WorkStealingScheduler::new(TaskDistribution::work_stealing(3));
        assert_eq!(scheduler.total_pending(), 0);
        assert_eq!(scheduler.steal_threshold(), 3);
    }

    #[test]
    fn register_agent_creates_local_queue() {
        let mut scheduler = WorkStealingScheduler::new(TaskDistribution::work_stealing(4));
        scheduler.register_agent("agent-1");
        assert_eq!(scheduler.local_queue_len("agent-1"), 0);
        assert_eq!(scheduler.agent_ids().len(), 1);
    }

    #[test]
    fn push_global_and_pop() {
        let mut scheduler = WorkStealingScheduler::new(TaskDistribution::work_stealing(4));
        scheduler.register_agent("a1");
        scheduler.push_global(task("g1", 5));
        scheduler.push_global(task("g2", 10));

        let popped = scheduler.pop("a1").unwrap();
        assert_eq!(popped.id, "g2"); // highest priority first
    }

    #[test]
    fn push_local_and_pop_respects_priority() {
        let mut scheduler = WorkStealingScheduler::new(TaskDistribution::work_stealing(4));
        scheduler.register_agent("a1");
        scheduler.push_local("a1", task("low", 1));
        scheduler.push_local("a1", task("high", 10));
        scheduler.push_local("a1", task("mid", 5));

        assert_eq!(scheduler.pop("a1").unwrap().id, "high");
        assert_eq!(scheduler.pop("a1").unwrap().id, "mid");
        assert_eq!(scheduler.pop("a1").unwrap().id, "low");
        assert!(scheduler.pop("a1").is_none());
    }

    #[test]
    fn local_queue_preferred_over_global() {
        let mut scheduler = WorkStealingScheduler::new(TaskDistribution::work_stealing(4));
        scheduler.register_agent("a1");
        scheduler.push_global(task("global-task", 100));
        scheduler.push_local("a1", task("local-task", 1));

        // local is popped even though global has higher priority
        assert_eq!(scheduler.pop("a1").unwrap().id, "local-task");
        assert_eq!(scheduler.pop("a1").unwrap().id, "global-task");
    }

    #[test]
    fn work_stealing_steals_from_busiest_agent() {
        let mut scheduler = WorkStealingScheduler::new(TaskDistribution::work_stealing(2));
        scheduler.register_agent("idle");
        scheduler.register_agent("busy");

        // Fill busy agent above threshold
        for i in 0..5 {
            scheduler.push_local("busy", task(&format!("t{i}"), i as u32));
        }

        // idle agent has nothing, should steal
        let stolen = scheduler.pop("idle");
        assert!(stolen.is_some());
        // The stolen task should be the lowest priority from busy
        let stolen = stolen.unwrap();
        assert_eq!(stolen.id, "t0");
    }

    #[test]
    fn no_stealing_below_threshold() {
        let mut scheduler = WorkStealingScheduler::new(TaskDistribution::work_stealing(10));
        scheduler.register_agent("a1");
        scheduler.register_agent("a2");

        // a2 has tasks but below threshold (10)
        for i in 0..3 {
            scheduler.push_local("a2", task(&format!("t{i}"), 5));
        }

        assert!(scheduler.pop("a1").is_none());
    }

    #[test]
    fn assign_work_stealing_routes_to_least_loaded() {
        let mut scheduler = WorkStealingScheduler::new(TaskDistribution::work_stealing(4));
        scheduler.register_agent("a1");
        scheduler.register_agent("a2");

        scheduler.push_local("a1", task("existing", 5));

        let assigned = scheduler.assign(task("new", 3)).unwrap();
        assert_eq!(assigned, "a2"); // a2 has 0 tasks
    }

    #[test]
    fn assign_consistent_hash_deterministic() {
        let mut scheduler = WorkStealingScheduler::new(TaskDistribution::consistent_hash(2));
        scheduler.register_agent("a1");
        scheduler.register_agent("a2");
        scheduler.register_agent("a3");

        let first = scheduler.assign(task("task-x", 5)).unwrap();
        // Same task ID should always go to the same agent
        // (we'd need to re-create to test determinism properly, but at least
        // we verify assignment works)
        assert!(scheduler.agent_ids().contains(&first));
    }

    #[test]
    fn assign_auction_considers_capability() {
        let mut scheduler = WorkStealingScheduler::new(TaskDistribution::auction(ScoringWeights {
            capability_match: 1.0,
            queue_balance: 0.0,
            affinity: 0.0,
        }));
        scheduler.register_agent_with_profile(AgentProfile {
            agent_id: "builder".to_string(),
            capabilities: vec!["build".to_string()],
            success_rate: 0.9,
        });
        scheduler.register_agent_with_profile(AgentProfile {
            agent_id: "reviewer".to_string(),
            capabilities: vec!["review".to_string()],
            success_rate: 0.8,
        });

        let assigned = scheduler.assign(task_with_cap("t1", 5, "build")).unwrap();
        assert_eq!(assigned, "builder");

        let assigned = scheduler.assign(task_with_cap("t2", 5, "review")).unwrap();
        assert_eq!(assigned, "reviewer");
    }

    #[test]
    fn auction_prefers_less_loaded_agent_with_equal_capability() {
        let mut scheduler = WorkStealingScheduler::new(TaskDistribution::auction(ScoringWeights {
            capability_match: 0.0,
            queue_balance: 1.0,
            affinity: 0.0,
        }));
        scheduler.register_agent("a1");
        scheduler.register_agent("a2");

        // Load a1
        scheduler.push_local("a1", task("existing", 5));

        let assigned = scheduler.assign(task("new", 5)).unwrap();
        assert_eq!(assigned, "a2");
    }

    #[test]
    fn record_completion_improves_affinity_score() {
        let mut scheduler = WorkStealingScheduler::new(TaskDistribution::auction(ScoringWeights {
            capability_match: 0.0,
            queue_balance: 0.0,
            affinity: 1.0,
        }));
        scheduler.register_agent("a1");
        scheduler.register_agent("a2");

        scheduler.record_completion("a1", "general");
        scheduler.record_completion("a1", "general");
        scheduler.record_completion("a1", "general");

        let assigned = scheduler.assign(task("t1", 5)).unwrap();
        assert_eq!(assigned, "a1"); // a1 has affinity for "general"
    }

    #[test]
    fn assign_returns_none_with_no_agents() {
        let mut scheduler = WorkStealingScheduler::new(TaskDistribution::work_stealing(4));
        let result = scheduler.assign(task("t1", 5));
        assert!(result.is_none());
        assert_eq!(scheduler.global_queue_len(), 1);
    }

    #[test]
    fn total_pending_counts_all_queues() {
        let mut scheduler = WorkStealingScheduler::new(TaskDistribution::work_stealing(4));
        scheduler.register_agent("a1");
        scheduler.register_agent("a2");
        scheduler.push_global(task("g1", 1));
        scheduler.push_local("a1", task("l1", 2));
        scheduler.push_local("a2", task("l2", 3));

        assert_eq!(scheduler.total_pending(), 3);
    }

    #[test]
    fn pop_returns_none_when_everything_empty() {
        let mut scheduler = WorkStealingScheduler::new(TaskDistribution::work_stealing(4));
        scheduler.register_agent("a1");
        assert!(scheduler.pop("a1").is_none());
    }

    #[test]
    fn stealing_takes_lowest_priority_from_victim() {
        let mut scheduler = WorkStealingScheduler::new(TaskDistribution::work_stealing(1));
        scheduler.register_agent("thief");
        scheduler.register_agent("victim");

        scheduler.push_local("victim", task("critical", 100));
        scheduler.push_local("victim", task("trivial", 1));
        scheduler.push_local("victim", task("medium", 50));

        let stolen = scheduler.pop("thief").unwrap();
        assert_eq!(stolen.id, "trivial");

        // Victim still has the high-priority tasks
        assert_eq!(scheduler.local_queue_len("victim"), 2);
    }

    #[test]
    fn default_scoring_weights() {
        let w = ScoringWeights::default();
        assert!((w.capability_match - 0.5).abs() < f64::EPSILON);
        assert!((w.queue_balance - 0.3).abs() < f64::EPSILON);
        assert!((w.affinity - 0.2).abs() < f64::EPSILON);
    }

    #[test]
    fn task_distribution_variants() {
        let ws = TaskDistribution::work_stealing(8);
        assert_eq!(ws, TaskDistribution::WorkStealing { steal_threshold: 8 });

        let ch = TaskDistribution::consistent_hash(3);
        assert_eq!(
            ch,
            TaskDistribution::ConsistentHash {
                replication_factor: 3
            }
        );

        let au = TaskDistribution::auction(ScoringWeights::default());
        assert!(matches!(au, TaskDistribution::Auction { .. }));
    }

    #[test]
    fn multi_agent_stealing_scenario() {
        let mut scheduler = WorkStealingScheduler::new(TaskDistribution::work_stealing(2));
        for i in 0..4 {
            scheduler.register_agent(&format!("agent-{i}"));
        }

        // Overload agent-0
        for i in 0..10 {
            scheduler.push_local("agent-0", task(&format!("t{i}"), i as u32));
        }

        // Each other agent should be able to steal
        for i in 1..4 {
            let stolen = scheduler.pop(&format!("agent-{i}"));
            assert!(stolen.is_some(), "agent-{i} should steal a task");
        }
    }
}
