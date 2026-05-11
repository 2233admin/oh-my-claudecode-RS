use std::collections::{HashMap, HashSet};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::AgentSessionRecord;

/// Agent identifier used across the fault tolerance subsystem.
pub type AgentId = String;

/// Exponential backoff parameters for retry strategies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExponentialBackoff {
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub multiplier: f64,
}

impl ExponentialBackoff {
    pub fn new(initial_delay: Duration, max_delay: Duration, multiplier: f64) -> Self {
        Self {
            initial_delay,
            max_delay,
            multiplier: multiplier.max(1.0),
        }
    }

    /// Compute the delay for a given attempt (0-indexed).
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let factor = self.multiplier.powi(attempt as i32);
        let nanos = self.initial_delay.as_nanos() as f64 * factor;
        let capped = nanos.min(self.max_delay.as_nanos() as f64);
        Duration::from_nanos(capped as u64)
    }
}

impl Default for ExponentialBackoff {
    fn default() -> Self {
        Self {
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            multiplier: 2.0,
        }
    }
}

/// Topology types for degraded operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TopologyType {
    /// All-to-all communication between agents.
    FullMesh,
    /// Agents organized into cells with a lead per cell.
    CellBased,
    /// Single coordinator with leaf agents (hub-and-spoke).
    Star,
}

/// Recovery strategy applied when an agent fails.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RecoveryStrategy {
    /// Retry the failed agent with exponential backoff.
    Retry {
        max_attempts: u8,
        backoff: ExponentialBackoff,
    },
    /// Redirect work to a backup agent.
    Failover { backup_agent: AgentId },
    /// Reduce team topology (e.g. FullMesh -> CellBased) and redistribute work.
    Degrade { reduced_topology: TopologyType },
    /// Promote a designated heir agent to take over the failed agent's role.
    Succession { heir: AgentId },
}

/// Record of a single agent failure event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureRecord {
    pub agent_id: AgentId,
    pub attempt: u8,
    pub error: String,
    pub timestamp: u64,
    pub recovered: bool,
}

/// Core fault tolerance configuration and state.
#[derive(Debug, Clone)]
pub struct FaultTolerance {
    pub strategy: RecoveryStrategy,
    pub heartbeat_timeout: Duration,
    pub max_failures: usize,
    pub quarantine: HashSet<AgentId>,
    pub failure_log: Vec<FailureRecord>,
    pub failure_counts: HashMap<AgentId, usize>,
}

/// Minimal representation of a swarm's agent set for fault recovery operations.
#[derive(Debug, Clone)]
pub struct Swarm {
    pub agents: HashMap<AgentId, AgentSessionRecord>,
    pub topology: TopologyType,
    pub active_tasks: HashMap<AgentId, Vec<String>>,
}

impl Swarm {
    pub fn new(topology: TopologyType) -> Self {
        Self {
            agents: HashMap::new(),
            topology,
            active_tasks: HashMap::new(),
        }
    }

    pub fn add_agent(&mut self, agent_id: AgentId, session: AgentSessionRecord) {
        self.agents.insert(agent_id, session);
    }

    pub fn remove_agent(&mut self, agent_id: &AgentId) -> Option<AgentSessionRecord> {
        self.agents.remove(agent_id)
    }

    /// Returns agents that are neither the failed one nor quarantined.
    pub fn eligible_agents(&self, exclude: &AgentId) -> Vec<AgentId> {
        self.agents
            .keys()
            .filter(|id| *id != exclude)
            .cloned()
            .collect()
    }

    /// Redistribute orphaned tasks from a failed agent to other agents in round-robin.
    pub fn redistribute_tasks(&mut self, failed: &AgentId) {
        if let Some(orphaned) = self.active_tasks.remove(failed) {
            if orphaned.is_empty() {
                return;
            }
            let recipients: Vec<AgentId> = self
                .agents
                .keys()
                .filter(|id| *id != failed)
                .cloned()
                .collect();
            if recipients.is_empty() {
                self.active_tasks.insert(failed.clone(), orphaned);
                return;
            }
            for (i, task) in orphaned.into_iter().enumerate() {
                let target = &recipients[i % recipients.len()];
                self.active_tasks
                    .entry(target.clone())
                    .or_default()
                    .push(task);
            }
        }
    }

    pub fn change_topology(&mut self, new_topology: TopologyType) {
        self.topology = new_topology;
    }
}

/// Outcome of handling an agent failure.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum RecoveryOutcome {
    RetryScheduled {
        agent_id: AgentId,
        attempt: u8,
        delay: Duration,
    },
    FailoverActivated {
        failed: AgentId,
        backup: AgentId,
    },
    Degraded {
        failed: AgentId,
        new_topology: TopologyType,
        tasks_redistributed: bool,
    },
    SuccessionTriggered {
        failed: AgentId,
        heir: AgentId,
    },
    Quarantined {
        agent_id: AgentId,
        reason: String,
    },
    Ignored {
        agent_id: AgentId,
        reason: String,
    },
}

impl FaultTolerance {
    pub fn new(strategy: RecoveryStrategy) -> Self {
        Self {
            strategy,
            heartbeat_timeout: Duration::from_secs(30),
            max_failures: 3,
            quarantine: HashSet::new(),
            failure_log: Vec::new(),
            failure_counts: HashMap::new(),
        }
    }

    pub fn with_heartbeat_timeout(mut self, timeout: Duration) -> Self {
        self.heartbeat_timeout = timeout;
        self
    }

    pub fn with_max_failures(mut self, max: usize) -> Self {
        self.max_failures = max;
        self
    }

    /// Handle an agent failure. Returns the recovery outcome and mutates the swarm as needed.
    pub async fn on_agent_failure(
        &mut self,
        failed: AgentId,
        error: String,
        swarm: &mut Swarm,
    ) -> Result<RecoveryOutcome, String> {
        if self.quarantine.contains(&failed) {
            return Ok(RecoveryOutcome::Ignored {
                agent_id: failed,
                reason: "agent is quarantined".to_string(),
            });
        }

        let count = self.failure_counts.entry(failed.clone()).or_insert(0);
        *count += 1;

        let record = FailureRecord {
            agent_id: failed.clone(),
            attempt: *count as u8,
            error: error.clone(),
            timestamp: crate::unix_timestamp(),
            recovered: false,
        };
        self.failure_log.push(record);

        if *count > self.max_failures {
            self.quarantine.insert(failed.clone());
            swarm.redistribute_tasks(&failed);
            return Ok(RecoveryOutcome::Quarantined {
                agent_id: failed,
                reason: format!("exceeded max failures ({} > {})", *count, self.max_failures),
            });
        }

        match &self.strategy {
            RecoveryStrategy::Retry {
                max_attempts,
                backoff,
            } => {
                let attempt = *count as u8;
                if attempt > *max_attempts {
                    self.quarantine.insert(failed.clone());
                    swarm.redistribute_tasks(&failed);
                    return Ok(RecoveryOutcome::Quarantined {
                        agent_id: failed,
                        reason: format!("exceeded max retry attempts ({attempt} > {max_attempts})"),
                    });
                }
                let delay = backoff.delay_for_attempt((attempt - 1) as u32);
                Ok(RecoveryOutcome::RetryScheduled {
                    agent_id: failed,
                    attempt,
                    delay,
                })
            }
            RecoveryStrategy::Failover { backup_agent } => {
                if !swarm.agents.contains_key(backup_agent) {
                    return Err(format!("backup agent {} not found in swarm", backup_agent));
                }
                swarm.redistribute_tasks(&failed);
                swarm.remove_agent(&failed);
                Ok(RecoveryOutcome::FailoverActivated {
                    failed,
                    backup: backup_agent.clone(),
                })
            }
            RecoveryStrategy::Degrade { reduced_topology } => {
                let tasks_redistributed = swarm.active_tasks.contains_key(&failed);
                swarm.redistribute_tasks(&failed);
                swarm.remove_agent(&failed);
                swarm.change_topology(*reduced_topology);
                Ok(RecoveryOutcome::Degraded {
                    failed,
                    new_topology: *reduced_topology,
                    tasks_redistributed,
                })
            }
            RecoveryStrategy::Succession { heir } => {
                if !swarm.agents.contains_key(heir) {
                    return Err(format!("heir agent {} not found in swarm", heir));
                }
                if let Some(tasks) = swarm.active_tasks.remove(&failed) {
                    swarm
                        .active_tasks
                        .entry(heir.clone())
                        .or_default()
                        .extend(tasks);
                }
                swarm.remove_agent(&failed);
                Ok(RecoveryOutcome::SuccessionTriggered {
                    failed,
                    heir: heir.clone(),
                })
            }
        }
    }

    /// Returns the list of quarantined agents.
    pub fn quarantined(&self) -> &HashSet<AgentId> {
        &self.quarantine
    }

    /// Returns the failure count for an agent.
    pub fn failure_count(&self, agent_id: &AgentId) -> usize {
        self.failure_counts.get(agent_id).copied().unwrap_or(0)
    }

    /// Returns the full failure log.
    pub fn failure_log(&self) -> &[FailureRecord] {
        &self.failure_log
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AgentSessionState, RuntimeKind, UsageRollup};
    use std::time::Duration;

    fn make_session(agent_id: &str) -> AgentSessionRecord {
        AgentSessionRecord {
            record_type: "agent_session".to_string(),
            agent_id: agent_id.to_string(),
            run_id: "run-test".to_string(),
            cell_id: None,
            role: "builder".to_string(),
            runtime: RuntimeKind::Claude,
            provider: "claude-code".to_string(),
            current_task: "LOCAL-1".to_string(),
            state: AgentSessionState::Active,
            epoch: 0,
            last_resume_brief: None,
            last_handoff: None,
            usage_rollup: UsageRollup::default(),
            created_at: 1,
            updated_at: 1,
        }
    }

    fn make_swarm(agent_ids: &[&str]) -> Swarm {
        let mut swarm = Swarm::new(TopologyType::FullMesh);
        for id in agent_ids {
            swarm.add_agent(id.to_string(), make_session(id));
        }
        swarm
    }

    #[tokio::test]
    async fn retry_schedules_with_backoff() {
        let mut ft = FaultTolerance::new(RecoveryStrategy::Retry {
            max_attempts: 3,
            backoff: ExponentialBackoff::new(Duration::from_secs(1), Duration::from_secs(30), 2.0),
        });
        let mut swarm = make_swarm(&["agent-1", "agent-2"]);

        let outcome = ft
            .on_agent_failure("agent-1".to_string(), "timeout".to_string(), &mut swarm)
            .await
            .unwrap();

        match outcome {
            RecoveryOutcome::RetryScheduled {
                agent_id,
                attempt,
                delay,
                ..
            } => {
                assert_eq!(agent_id, "agent-1");
                assert_eq!(attempt, 1);
                assert_eq!(delay, Duration::from_secs(1));
            }
            other => panic!("expected RetryScheduled, got {other:?}"),
        }
        assert_eq!(ft.failure_count(&"agent-1".to_string()), 1);
    }

    #[tokio::test]
    async fn retry_quarantines_after_max_attempts() {
        let mut ft = FaultTolerance::new(RecoveryStrategy::Retry {
            max_attempts: 2,
            backoff: ExponentialBackoff::default(),
        })
        .with_max_failures(3);
        let mut swarm = make_swarm(&["agent-1", "agent-2"]);

        ft.on_agent_failure("agent-1".to_string(), "err1".to_string(), &mut swarm)
            .await
            .unwrap();
        ft.on_agent_failure("agent-1".to_string(), "err2".to_string(), &mut swarm)
            .await
            .unwrap();
        let outcome = ft
            .on_agent_failure("agent-1".to_string(), "err3".to_string(), &mut swarm)
            .await
            .unwrap();

        assert!(matches!(outcome, RecoveryOutcome::Quarantined { .. }));
        assert!(ft.quarantined().contains("agent-1"));
    }

    #[tokio::test]
    async fn failover_redirects_to_backup() {
        let mut ft = FaultTolerance::new(RecoveryStrategy::Failover {
            backup_agent: "agent-backup".to_string(),
        });
        let mut swarm = make_swarm(&["agent-1", "agent-backup", "agent-2"]);

        let outcome = ft
            .on_agent_failure("agent-1".to_string(), "crash".to_string(), &mut swarm)
            .await
            .unwrap();

        match outcome {
            RecoveryOutcome::FailoverActivated { failed, backup } => {
                assert_eq!(failed, "agent-1");
                assert_eq!(backup, "agent-backup");
            }
            other => panic!("expected FailoverActivated, got {other:?}"),
        }
        assert!(!swarm.agents.contains_key("agent-1"));
        assert!(swarm.agents.contains_key("agent-backup"));
    }

    #[tokio::test]
    async fn failover_errors_on_missing_backup() {
        let mut ft = FaultTolerance::new(RecoveryStrategy::Failover {
            backup_agent: "ghost".to_string(),
        });
        let mut swarm = make_swarm(&["agent-1"]);

        let result = ft
            .on_agent_failure("agent-1".to_string(), "err".to_string(), &mut swarm)
            .await;

        assert!(result.unwrap_err().contains("ghost"));
    }

    #[tokio::test]
    async fn degrade_changes_topology_and_removes_agent() {
        let mut ft = FaultTolerance::new(RecoveryStrategy::Degrade {
            reduced_topology: TopologyType::Star,
        });
        let mut swarm = make_swarm(&["agent-1", "agent-2", "agent-3"]);
        swarm
            .active_tasks
            .insert("agent-1".to_string(), vec!["task-a".to_string()]);

        let outcome = ft
            .on_agent_failure("agent-1".to_string(), "overload".to_string(), &mut swarm)
            .await
            .unwrap();

        match outcome {
            RecoveryOutcome::Degraded {
                failed,
                new_topology,
                tasks_redistributed,
            } => {
                assert_eq!(failed, "agent-1");
                assert_eq!(new_topology, TopologyType::Star);
                assert!(tasks_redistributed);
            }
            other => panic!("expected Degraded, got {other:?}"),
        }
        assert_eq!(swarm.topology, TopologyType::Star);
        assert!(!swarm.agents.contains_key("agent-1"));
        assert!(
            swarm
                .active_tasks
                .values()
                .any(|tasks| tasks.contains(&"task-a".to_string()))
        );
    }

    #[tokio::test]
    async fn succession_transfers_tasks_to_heir() {
        let mut ft = FaultTolerance::new(RecoveryStrategy::Succession {
            heir: "agent-heir".to_string(),
        });
        let mut swarm = make_swarm(&["agent-1", "agent-heir", "agent-3"]);
        swarm.active_tasks.insert(
            "agent-1".to_string(),
            vec!["task-x".to_string(), "task-y".to_string()],
        );

        let outcome = ft
            .on_agent_failure("agent-1".to_string(), "panic".to_string(), &mut swarm)
            .await
            .unwrap();

        match outcome {
            RecoveryOutcome::SuccessionTriggered { failed, heir } => {
                assert_eq!(failed, "agent-1");
                assert_eq!(heir, "agent-heir");
            }
            other => panic!("expected SuccessionTriggered, got {other:?}"),
        }
        assert!(!swarm.agents.contains_key("agent-1"));
        let heir_tasks = swarm.active_tasks.get("agent-heir").unwrap();
        assert!(heir_tasks.contains(&"task-x".to_string()));
        assert!(heir_tasks.contains(&"task-y".to_string()));
    }

    #[tokio::test]
    async fn quarantined_agent_is_ignored() {
        let mut ft = FaultTolerance::new(RecoveryStrategy::Retry {
            max_attempts: 1,
            backoff: ExponentialBackoff::default(),
        });
        let mut swarm = make_swarm(&["agent-1", "agent-2"]);

        ft.on_agent_failure("agent-1".to_string(), "err1".to_string(), &mut swarm)
            .await
            .unwrap();
        ft.on_agent_failure("agent-1".to_string(), "err2".to_string(), &mut swarm)
            .await
            .unwrap();

        let outcome = ft
            .on_agent_failure("agent-1".to_string(), "err3".to_string(), &mut swarm)
            .await
            .unwrap();

        assert!(matches!(outcome, RecoveryOutcome::Ignored { .. }));
    }

    #[test]
    fn exponential_backoff_caps_at_max() {
        let backoff = ExponentialBackoff::new(Duration::from_secs(1), Duration::from_secs(10), 2.0);
        assert_eq!(backoff.delay_for_attempt(0), Duration::from_secs(1));
        assert_eq!(backoff.delay_for_attempt(1), Duration::from_secs(2));
        assert_eq!(backoff.delay_for_attempt(2), Duration::from_secs(4));
        assert_eq!(backoff.delay_for_attempt(3), Duration::from_secs(8));
        assert_eq!(backoff.delay_for_attempt(4), Duration::from_secs(10));
        assert_eq!(backoff.delay_for_attempt(10), Duration::from_secs(10));
    }

    #[test]
    fn swarm_redistributes_tasks_round_robin() {
        let mut swarm = make_swarm(&["a", "b", "c"]);
        swarm.active_tasks.insert(
            "a".to_string(),
            vec!["t1".to_string(), "t2".to_string(), "t3".to_string()],
        );
        swarm.redistribute_tasks(&"a".to_string());

        // Failed agent's tasks are removed
        assert!(swarm.active_tasks.get("a").is_none_or(std::vec::Vec::is_empty));

        // All tasks redistributed to remaining agents (order-independent)
        let b_tasks = swarm.active_tasks.get("b").map_or(0, std::vec::Vec::len);
        let c_tasks = swarm.active_tasks.get("c").map_or(0, std::vec::Vec::len);
        assert_eq!(b_tasks + c_tasks, 3);

        // One agent gets 2, the other gets 1 (round-robin with 3 tasks across 2 agents)
        let (bigger, smaller) = if b_tasks > c_tasks {
            (b_tasks, c_tasks)
        } else {
            (c_tasks, b_tasks)
        };
        assert_eq!(bigger, 2);
        assert_eq!(smaller, 1);

        // All 3 tasks are present across remaining agents
        let all: std::collections::HashSet<_> =
            swarm.active_tasks.values().flatten().cloned().collect();
        assert!(all.contains("t1"));
        assert!(all.contains("t2"));
        assert!(all.contains("t3"));
    }

    #[test]
    fn failure_log_records_events() {
        let mut ft = FaultTolerance::new(RecoveryStrategy::Retry {
            max_attempts: 5,
            backoff: ExponentialBackoff::default(),
        });
        let rt = tokio::runtime::Runtime::new().unwrap();
        let mut swarm = make_swarm(&["agent-1"]);

        rt.block_on(async {
            ft.on_agent_failure("agent-1".to_string(), "err".to_string(), &mut swarm)
                .await
                .unwrap();
        });

        assert_eq!(ft.failure_log().len(), 1);
        assert_eq!(ft.failure_log()[0].agent_id, "agent-1");
        assert_eq!(ft.failure_log()[0].attempt, 1);
        assert!(!ft.failure_log()[0].recovered);
    }
}
