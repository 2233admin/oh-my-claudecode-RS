use std::collections::HashMap;
use std::fmt::Write as FmtWrite;

use serde::{Deserialize, Serialize};

use crate::phase_controller::TeamPhase;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
}

impl TokenUsage {
    pub fn total(&self) -> u64 {
        self.input_tokens + self.output_tokens + self.cache_read_tokens + self.cache_write_tokens
    }

    pub fn merge(&mut self, other: &TokenUsage) {
        self.input_tokens += other.input_tokens;
        self.output_tokens += other.output_tokens;
        self.cache_read_tokens += other.cache_read_tokens;
        self.cache_write_tokens += other.cache_write_tokens;
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CostBreakdown {
    pub input_cost: f64,
    pub output_cost: f64,
    pub cache_read_cost: f64,
    pub cache_write_cost: f64,
    pub total_cost: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerUsage {
    pub worker_id: String,
    pub model: String,
    pub tokens: TokenUsage,
    pub cost: CostBreakdown,
    pub tasks_completed: u32,
    pub tasks_failed: u32,
    pub total_duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamUsageSummary {
    pub team_name: String,
    pub phase: TeamPhase,
    pub workers: Vec<WorkerUsage>,
    pub total_tokens: TokenUsage,
    pub total_cost: CostBreakdown,
    pub total_tasks: u32,
    pub completed_tasks: u32,
    pub failed_tasks: u32,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub duration_ms: u64,
}

pub struct UsageTracker {
    workers: HashMap<String, WorkerUsage>,
    started_at: String,
}

impl UsageTracker {
    pub fn new() -> Self {
        Self {
            workers: HashMap::default(),
            started_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub fn record_tokens(&mut self, worker_id: &str, model: &str, tokens: TokenUsage) {
        let pricing = default_pricing(model);
        let cost = calculate_cost(&tokens, &pricing);
        let entry = self
            .workers
            .entry(worker_id.to_string())
            .or_insert_with(|| WorkerUsage {
                worker_id: worker_id.to_string(),
                model: model.to_string(),
                tokens: TokenUsage::default(),
                cost: CostBreakdown::default(),
                tasks_completed: 0,
                tasks_failed: 0,
                total_duration_ms: 0,
            });
        entry.tokens.merge(&tokens);
        entry.cost.input_cost += cost.input_cost;
        entry.cost.output_cost += cost.output_cost;
        entry.cost.cache_read_cost += cost.cache_read_cost;
        entry.cost.cache_write_cost += cost.cache_write_cost;
        entry.cost.total_cost += cost.total_cost;
    }

    pub fn record_task_completion(&mut self, worker_id: &str, success: bool, duration_ms: u64) {
        let entry = self
            .workers
            .entry(worker_id.to_string())
            .or_insert_with(|| WorkerUsage {
                worker_id: worker_id.to_string(),
                model: String::default(),
                tokens: TokenUsage::default(),
                cost: CostBreakdown::default(),
                tasks_completed: 0,
                tasks_failed: 0,
                total_duration_ms: 0,
            });
        if success {
            entry.tasks_completed += 1;
        } else {
            entry.tasks_failed += 1;
        }
        entry.total_duration_ms += duration_ms;
    }

    pub fn get_worker_usage(&self, worker_id: &str) -> Option<&WorkerUsage> {
        self.workers.get(worker_id)
    }

    pub fn summary(&self, team_name: &str, phase: TeamPhase) -> TeamUsageSummary {
        let workers: Vec<WorkerUsage> = self.workers.values().cloned().collect();
        let mut total_tokens = TokenUsage::default();
        let mut total_cost = CostBreakdown::default();
        let mut total_tasks: u32 = 0;
        let mut completed_tasks: u32 = 0;
        let mut failed_tasks: u32 = 0;
        let mut max_duration: u64 = 0;

        for w in &workers {
            total_tokens.merge(&w.tokens);
            total_cost.input_cost += w.cost.input_cost;
            total_cost.output_cost += w.cost.output_cost;
            total_cost.cache_read_cost += w.cost.cache_read_cost;
            total_cost.cache_write_cost += w.cost.cache_write_cost;
            total_cost.total_cost += w.cost.total_cost;
            completed_tasks += w.tasks_completed;
            failed_tasks += w.tasks_failed;
            total_tasks += w.tasks_completed + w.tasks_failed;
            if w.total_duration_ms > max_duration {
                max_duration = w.total_duration_ms;
            }
        }

        TeamUsageSummary {
            team_name: team_name.to_string(),
            phase,
            workers,
            total_tokens,
            total_cost,
            total_tasks,
            completed_tasks,
            failed_tasks,
            started_at: self.started_at.clone(),
            ended_at: None,
            duration_ms: max_duration,
        }
    }

    pub fn total_cost(&self) -> f64 {
        self.workers.values().map(|w| w.cost.total_cost).sum()
    }
}

impl Default for UsageTracker {
    fn default() -> Self {
        Self {
            workers: HashMap::default(),
            started_at: chrono::Utc::now().to_rfc3339(),
        }
    }
}

/// Model pricing per 1M tokens (USD).
pub struct ModelPricing {
    pub input_per_mtok: f64,
    pub output_per_mtok: f64,
    pub cache_read_per_mtok: f64,
    pub cache_write_per_mtok: f64,
}

pub fn default_pricing(model: &str) -> ModelPricing {
    let lower = model.to_lowercase();
    if lower.contains("opus") {
        ModelPricing {
            input_per_mtok: 15.0,
            output_per_mtok: 75.0,
            cache_read_per_mtok: 1.5,
            cache_write_per_mtok: 18.75,
        }
    } else if lower.contains("haiku") {
        ModelPricing {
            input_per_mtok: 0.25,
            output_per_mtok: 1.25,
            cache_read_per_mtok: 0.03,
            cache_write_per_mtok: 0.3,
        }
    } else {
        // Default to sonnet pricing
        ModelPricing {
            input_per_mtok: 3.0,
            output_per_mtok: 15.0,
            cache_read_per_mtok: 0.3,
            cache_write_per_mtok: 3.75,
        }
    }
}

pub fn calculate_cost(tokens: &TokenUsage, pricing: &ModelPricing) -> CostBreakdown {
    let input_cost = (tokens.input_tokens as f64) * pricing.input_per_mtok / 1_000_000.0;
    let output_cost = (tokens.output_tokens as f64) * pricing.output_per_mtok / 1_000_000.0;
    let cache_read_cost =
        (tokens.cache_read_tokens as f64) * pricing.cache_read_per_mtok / 1_000_000.0;
    let cache_write_cost =
        (tokens.cache_write_tokens as f64) * pricing.cache_write_per_mtok / 1_000_000.0;
    CostBreakdown {
        input_cost,
        output_cost,
        cache_read_cost,
        cache_write_cost,
        total_cost: input_cost + output_cost + cache_read_cost + cache_write_cost,
    }
}

pub fn render_summary_report(summary: &TeamUsageSummary) -> String {
    let mut out = String::default();
    writeln!(out, "# Team Usage: {}\n", summary.team_name).unwrap();
    writeln!(out, "**Phase:** {}", summary.phase.as_str()).unwrap();
    writeln!(out, "**Started:** {}", summary.started_at).unwrap();
    if let Some(ref ended) = summary.ended_at {
        writeln!(out, "**Ended:** {ended}").unwrap();
    }
    writeln!(out, "**Duration:** {} ms\n", summary.duration_ms).unwrap();

    out.push_str("## Tasks\n\n");
    writeln!(
        out,
        "| Metric | Count |\n|--------|-------|\n| Total | {} |\n| Completed | {} |\n| Failed | {} |\n",
        summary.total_tasks, summary.completed_tasks, summary.failed_tasks,
    )
    .unwrap();

    if !summary.workers.is_empty() {
        out.push_str("## Per-Worker Breakdown\n\n");
        out.push_str(
            "| Worker | Model | Input | Output | Cache Read | Cache Write | Total Tokens | Cost ($) |\n",
        );
        out.push_str(
            "|--------|-------|-------|--------|------------|-------------|--------------|----------|\n",
        );
        for w in &summary.workers {
            writeln!(
                out,
                "| {} | {} | {} | {} | {} | {} | {} | {:.6} |",
                w.worker_id,
                w.model,
                w.tokens.input_tokens,
                w.tokens.output_tokens,
                w.tokens.cache_read_tokens,
                w.tokens.cache_write_tokens,
                w.tokens.total(),
                w.cost.total_cost,
            )
            .unwrap();
        }
        out.push('\n');
    }

    out.push_str("## Totals\n\n");
    writeln!(
        out,
        "- **Input tokens:** {}",
        summary.total_tokens.input_tokens
    )
    .unwrap();
    writeln!(
        out,
        "- **Output tokens:** {}",
        summary.total_tokens.output_tokens
    )
    .unwrap();
    writeln!(
        out,
        "- **Cache read tokens:** {}",
        summary.total_tokens.cache_read_tokens
    )
    .unwrap();
    writeln!(
        out,
        "- **Cache write tokens:** {}",
        summary.total_tokens.cache_write_tokens
    )
    .unwrap();
    writeln!(out, "- **Total tokens:** {}", summary.total_tokens.total()).unwrap();
    writeln!(
        out,
        "- **Cache write cost:** ${:.6}",
        summary.total_cost.cache_write_cost
    )
    .unwrap();
    writeln!(
        out,
        "- **Total cost:** ${:.6}",
        summary.total_cost.total_cost
    )
    .unwrap();

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_usage_merge() {
        let mut a = TokenUsage {
            input_tokens: 100,
            output_tokens: 200,
            cache_read_tokens: 50,
            cache_write_tokens: 10,
        };
        let b = TokenUsage {
            input_tokens: 150,
            output_tokens: 250,
            cache_read_tokens: 60,
            cache_write_tokens: 20,
        };
        a.merge(&b);
        assert_eq!(a.input_tokens, 250);
        assert_eq!(a.output_tokens, 450);
        assert_eq!(a.cache_read_tokens, 110);
        assert_eq!(a.cache_write_tokens, 30);
    }

    #[test]
    fn token_usage_total() {
        let t = TokenUsage {
            input_tokens: 100,
            output_tokens: 200,
            cache_read_tokens: 50,
            cache_write_tokens: 10,
        };
        assert_eq!(t.total(), 360);
    }

    #[test]
    fn cost_opus_pricing() {
        let pricing = default_pricing("claude-opus-4-7");
        let tokens = TokenUsage {
            input_tokens: 1_000_000,
            output_tokens: 1_000_000,
            cache_read_tokens: 1_000_000,
            cache_write_tokens: 0,
        };
        let cost = calculate_cost(&tokens, &pricing);
        assert!((cost.input_cost - 15.0).abs() < f64::EPSILON);
        assert!((cost.output_cost - 75.0).abs() < f64::EPSILON);
        assert!((cost.cache_read_cost - 1.5).abs() < f64::EPSILON);
        assert!((cost.total_cost - 91.5).abs() < f64::EPSILON);
    }

    #[test]
    fn cost_sonnet_pricing() {
        let pricing = default_pricing("claude-sonnet-4-6");
        let tokens = TokenUsage {
            input_tokens: 1_000_000,
            output_tokens: 1_000_000,
            cache_read_tokens: 1_000_000,
            cache_write_tokens: 0,
        };
        let cost = calculate_cost(&tokens, &pricing);
        assert!((cost.input_cost - 3.0).abs() < f64::EPSILON);
        assert!((cost.output_cost - 15.0).abs() < f64::EPSILON);
        assert!((cost.cache_read_cost - 0.3).abs() < f64::EPSILON);
        assert!((cost.total_cost - 18.3).abs() < f64::EPSILON);
    }

    #[test]
    fn cost_haiku_pricing() {
        let pricing = default_pricing("claude-haiku-4-5-20251001");
        let tokens = TokenUsage {
            input_tokens: 1_000_000,
            output_tokens: 1_000_000,
            cache_read_tokens: 1_000_000,
            cache_write_tokens: 0,
        };
        let cost = calculate_cost(&tokens, &pricing);
        assert!((cost.input_cost - 0.25).abs() < f64::EPSILON);
        assert!((cost.output_cost - 1.25).abs() < f64::EPSILON);
        assert!((cost.cache_read_cost - 0.03).abs() < f64::EPSILON);
        assert!((cost.total_cost - 1.53).abs() < f64::EPSILON);
    }

    #[test]
    fn cost_unknown_model_defaults_to_sonnet() {
        let pricing = default_pricing("some-unknown-model");
        let tokens = TokenUsage {
            input_tokens: 1_000_000,
            output_tokens: 0,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
        };
        let cost = calculate_cost(&tokens, &pricing);
        assert!((cost.input_cost - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn cost_cache_write_pricing() {
        let pricing = default_pricing("claude-opus-4-7");
        let tokens = TokenUsage {
            input_tokens: 0,
            output_tokens: 0,
            cache_read_tokens: 0,
            cache_write_tokens: 1_000_000,
        };
        let cost = calculate_cost(&tokens, &pricing);
        assert!((cost.cache_write_cost - 18.75).abs() < f64::EPSILON);
        assert!((cost.total_cost - 18.75).abs() < f64::EPSILON);
    }

    #[test]
    fn cost_cache_write_sonnet_pricing() {
        let pricing = default_pricing("claude-sonnet-4-6");
        let tokens = TokenUsage {
            input_tokens: 0,
            output_tokens: 0,
            cache_read_tokens: 0,
            cache_write_tokens: 1_000_000,
        };
        let cost = calculate_cost(&tokens, &pricing);
        assert!((cost.cache_write_cost - 3.75).abs() < f64::EPSILON);
    }

    #[test]
    fn cost_cache_write_haiku_pricing() {
        let pricing = default_pricing("claude-haiku-4-5-20251001");
        let tokens = TokenUsage {
            input_tokens: 0,
            output_tokens: 0,
            cache_read_tokens: 0,
            cache_write_tokens: 1_000_000,
        };
        let cost = calculate_cost(&tokens, &pricing);
        assert!((cost.cache_write_cost - 0.3).abs() < f64::EPSILON);
    }

    #[test]
    fn tracker_records_tokens_and_tasks() {
        let mut tracker = UsageTracker::default();
        tracker.record_tokens(
            "worker-1",
            "claude-sonnet-4-6",
            TokenUsage {
                input_tokens: 1000,
                output_tokens: 500,
                cache_read_tokens: 200,
                cache_write_tokens: 0,
            },
        );
        tracker.record_task_completion("worker-1", true, 1500);
        tracker.record_task_completion("worker-1", true, 2000);
        tracker.record_task_completion("worker-1", false, 500);

        let w = tracker.get_worker_usage("worker-1").unwrap();
        assert_eq!(w.tokens.input_tokens, 1000);
        assert_eq!(w.tokens.output_tokens, 500);
        assert_eq!(w.tasks_completed, 2);
        assert_eq!(w.tasks_failed, 1);
        assert_eq!(w.total_duration_ms, 4000);
    }

    #[test]
    fn tracker_summary() {
        let mut tracker = UsageTracker::default();
        tracker.record_tokens(
            "w1",
            "claude-opus-4-7",
            TokenUsage {
                input_tokens: 100,
                output_tokens: 200,
                cache_read_tokens: 50,
                cache_write_tokens: 0,
            },
        );
        tracker.record_task_completion("w1", true, 1000);
        tracker.record_tokens(
            "w2",
            "claude-haiku-4-5-20251001",
            TokenUsage {
                input_tokens: 50,
                output_tokens: 100,
                cache_read_tokens: 25,
                cache_write_tokens: 0,
            },
        );
        tracker.record_task_completion("w2", false, 500);

        let summary = tracker.summary("test-team", TeamPhase::Completed);
        assert_eq!(summary.team_name, "test-team");
        assert_eq!(summary.phase, TeamPhase::Completed);
        assert_eq!(summary.workers.len(), 2);
        assert_eq!(summary.total_tasks, 2);
        assert_eq!(summary.completed_tasks, 1);
        assert_eq!(summary.failed_tasks, 1);
        assert_eq!(summary.total_tokens.input_tokens, 150);
        assert_eq!(summary.total_tokens.output_tokens, 300);
        assert_eq!(summary.duration_ms, 1000);
        assert!(summary.total_cost.total_cost > 0.0);
    }

    #[test]
    fn tracker_total_cost() {
        let mut tracker = UsageTracker::default();
        tracker.record_tokens(
            "w1",
            "claude-sonnet-4-6",
            TokenUsage {
                input_tokens: 1_000_000,
                output_tokens: 0,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
            },
        );
        assert!((tracker.total_cost() - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn summary_report_rendering() {
        let mut tracker = UsageTracker::default();
        tracker.record_tokens(
            "worker-alpha",
            "claude-opus-4-7",
            TokenUsage {
                input_tokens: 5000,
                output_tokens: 3000,
                cache_read_tokens: 1000,
                cache_write_tokens: 500,
            },
        );
        tracker.record_task_completion("worker-alpha", true, 2500);
        let summary = tracker.summary("demo-team", TeamPhase::Completed);
        let report = render_summary_report(&summary);
        assert!(report.contains("# Team Usage: demo-team"));
        assert!(report.contains("**Phase:** completed"));
        assert!(report.contains("worker-alpha"));
        assert!(report.contains("claude-opus-4-7"));
        assert!(report.contains("## Totals"));
        assert!(report.contains("Cache Write"));
        assert!(report.contains("Cache write cost"));
    }
}
