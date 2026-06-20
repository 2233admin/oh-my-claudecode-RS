//! Model Routing
//!
//! Intelligent model routing system that routes sub-agent tasks to appropriate
//! models (Opus/Sonnet/Haiku) based on task complexity.
//!
//! # Architecture
//!
//! The routing pipeline has four stages:
//!
//! 1. **Signal Extraction** (`signals`) - Extracts lexical, structural, and
//!    context signals from the task prompt via regex-based heuristics.
//!
//! 2. **Scoring** (`scorer`) - Applies weighted scoring to signals to produce
//!    a numeric complexity score and map it to a tier.
//!
//! 3. **Rules** (`rules`) - Evaluates priority-ordered rules that can override
//!    the score-based tier for specific agent types and task patterns.
//!
//! 4. **Router** (`router`) - Orchestrates the pipeline, handling overrides,
//!    escalation, and producing the final `RoutingDecision`.
//!
//! Prompts are adapted per tier via `prompts` (concise for Haiku, balanced for
//! Sonnet, deep-reasoning framing for Opus).
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use omc_shared::routing::{route_task, RoutingContext, RoutingConfig};
//!
//! let ctx = RoutingContext {
//!     task_prompt: "Find where authentication is implemented".into(),
//!     agent_type: Some("explore".into()),
//!     ..Default::default()
//! };
//! let config = RoutingConfig::default();
//! let decision = route_task(&ctx, &config);
//! assert_eq!(decision.tier, omc_shared::routing::ComplexityTier::Low);
//! ```

mod prompts;
mod router;
mod rules;
mod scorer;
mod signals;
#[cfg(test)]
mod tests;
mod types;

// Re-export types
pub use types::{
    ComplexityKeywords, ComplexitySignals, ComplexityTier, ContextSignals, Domain, ImpactScope,
    LexicalSignals, ModelType, PromptAdaptationStrategy, QuestionDepth, Reversibility,
    RoutingConfig, RoutingContext, RoutingDecision, RoutingRule, StructuralSignals,
    agent_category_tiers, model_type_to_tier, tier_prompt_strategy, tier_to_model_type,
};

// Re-export signal extraction
pub use signals::{
    extract_all_signals, extract_context_signals, extract_lexical_signals,
    extract_structural_signals,
};

// Re-export scoring
pub use scorer::{
    ScoreBreakdown, calculate_complexity_score, calculate_complexity_tier, calculate_confidence,
    get_score_breakdown, score_to_tier,
};

// Re-export rules
pub use rules::{RuleResult, default_routing_rules, evaluate_rules, get_matching_rules};

// Re-export router
pub use router::{
    analyze_task_complexity, can_escalate, escalate_model, explain_routing, get_model_for_task,
    get_routing_recommendation, quick_tier_for_agent, route_task,
};

// Re-export prompts
pub use prompts::{
    adapt_prompt_for_haiku, adapt_prompt_for_opus, adapt_prompt_for_sonnet, adapt_prompt_for_tier,
    create_delegation_prompt, get_prompt_prefix, get_prompt_strategy, get_prompt_suffix,
    get_task_instructions,
};

/// Convenience function to route and adapt prompt in one call
pub fn route_and_adapt_task(
    task_prompt: &str,
    agent_type: Option<&str>,
    previous_failures: Option<usize>,
) -> (RoutingDecision, String) {
    let ctx = RoutingContext {
        task_prompt: task_prompt.to_string(),
        agent_type: agent_type.map(String::from),
        previous_failures,
        ..Default::default()
    };
    let config = RoutingConfig::default();
    let mut decision = route_task(&ctx, &config);
    let adapted = adapt_prompt_for_tier(task_prompt, decision.tier);
    decision.adapted_prompt = Some(adapted.clone());
    (decision, adapted)
}
