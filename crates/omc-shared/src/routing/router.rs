//! Model Router
//!
//! Main routing engine that determines which model tier to use for a given task.
//! Combines signal extraction, scoring, and rules evaluation.

use super::rules::{default_routing_rules, evaluate_rules};
use super::scorer::{calculate_complexity_score, calculate_confidence, score_to_tier};
use super::signals::extract_all_signals;
use super::types::*;

/// Route a task to the appropriate model tier
pub fn route_task(ctx: &RoutingContext, config: &RoutingConfig) -> RoutingDecision {
    // If forceInherit is enabled, bypass all routing
    if config.force_inherit {
        return RoutingDecision {
            model: "inherit".into(),
            model_type: ModelType::Inherit,
            tier: ComplexityTier::Medium,
            confidence: 1.0,
            reasons: vec!["forceInherit enabled: agents inherit parent model".into()],
            adapted_prompt: None,
            escalated: false,
            original_tier: None,
        };
    }

    // If routing is disabled, use default tier
    if !config.enabled {
        return create_decision(
            config.default_tier,
            config,
            vec!["Routing disabled, using default tier".into()],
            false,
            None,
        );
    }

    // If explicit model is specified, respect it
    if let Some(explicit) = &ctx.explicit_model {
        let explicit_tier = match explicit {
            ModelType::Opus => ComplexityTier::High,
            ModelType::Haiku => ComplexityTier::Low,
            _ => ComplexityTier::Medium,
        };
        return create_decision(
            explicit_tier,
            config,
            vec!["Explicit model specified by user".into()],
            false,
            Some(explicit_tier),
        );
    }

    // Check for agent-specific overrides
    if let Some(agent_type) = &ctx.agent_type {
        if let Some((tier, reason)) = config.agent_overrides.get(agent_type) {
            return create_decision(*tier, config, vec![reason.clone()], false, Some(*tier));
        }
        // Fallback to quick tier lookup for known agent types
        if let Some(tier) = quick_tier_for_agent(agent_type) {
            return create_decision(
                tier,
                config,
                vec![format!("Agent type '{}' quick tier", agent_type)],
                false,
                Some(tier),
            );
        }
    }

    // Extract signals from the task
    let signals = extract_all_signals(&ctx.task_prompt, ctx);

    // Evaluate routing rules
    let rules = default_routing_rules();
    let rule_result = evaluate_rules(ctx, &signals, &rules);

    let rule_tier = rule_result.tier.unwrap_or(ComplexityTier::Medium);

    // Calculate score for confidence and logging
    let score = calculate_complexity_score(&signals);
    let score_tier = score_to_tier(score);
    let mut confidence = calculate_confidence(score, rule_tier);

    let mut final_tier = rule_tier;
    let rule_idx = rule_tier.index();
    let score_idx = score_tier.index();

    let divergence = rule_idx.abs_diff(score_idx);
    let mut reasons = vec![
        rule_result.reason,
        format!("Rule: {}", rule_result.rule_name),
        format!("Score: {} ({} tier by score)", score, score_tier),
    ];

    // When scorer and rules diverge by more than 1 level, reduce confidence
    if divergence > 1 {
        confidence = confidence.min(0.5);
        final_tier = ComplexityTier::from_index(rule_idx.max(score_idx));
        reasons.push(format!(
            "Scorer/rules divergence ({} levels): confidence reduced, preferred higher tier",
            divergence
        ));
    }

    // Enforce minTier if configured
    if let Some(min_tier) = config.min_tier
        && final_tier.index() < min_tier.index()
    {
        reasons.push(format!("Min tier enforced: {} -> {}", final_tier, min_tier));
        final_tier = min_tier;
    }

    RoutingDecision {
        model: config
            .tier_models
            .get(&final_tier)
            .cloned()
            .unwrap_or_default(),
        model_type: tier_to_model_type(final_tier),
        tier: final_tier,
        confidence,
        reasons,
        adapted_prompt: None,
        escalated: false,
        original_tier: None,
    }
}

fn create_decision(
    tier: ComplexityTier,
    config: &RoutingConfig,
    reasons: Vec<String>,
    escalated: bool,
    original_tier: Option<ComplexityTier>,
) -> RoutingDecision {
    RoutingDecision {
        model: config.tier_models.get(&tier).cloned().unwrap_or_default(),
        model_type: tier_to_model_type(tier),
        tier,
        confidence: if escalated { 0.9 } else { 0.7 },
        reasons,
        adapted_prompt: None,
        escalated,
        original_tier,
    }
}

/// Escalate to a higher tier after failure
pub fn escalate_model(current: ComplexityTier) -> ComplexityTier {
    match current {
        ComplexityTier::Low => ComplexityTier::Medium,
        ComplexityTier::Medium => ComplexityTier::High,
        ComplexityTier::High => ComplexityTier::High,
    }
}

/// Check if we can escalate further
pub fn can_escalate(current: ComplexityTier) -> bool {
    current != ComplexityTier::High
}

/// Get routing recommendation for orchestrator
pub fn get_routing_recommendation(ctx: &RoutingContext, config: &RoutingConfig) -> RoutingDecision {
    route_task(ctx, config)
}

/// Get recommended model for an agent based on task complexity
pub fn get_model_for_task(
    agent_type: &str,
    task_prompt: &str,
    config: &RoutingConfig,
) -> (ModelType, ComplexityTier, String) {
    let ctx = RoutingContext {
        task_prompt: task_prompt.to_string(),
        agent_type: Some(agent_type.to_string()),
        ..Default::default()
    };
    let decision = route_task(&ctx, config);

    let reason = decision
        .reasons
        .first()
        .cloned()
        .unwrap_or_else(|| "Complexity analysis".into());

    (decision.model_type, decision.tier, reason)
}

/// Quick tier lookup for known agent types
pub fn quick_tier_for_agent(agent_type: &str) -> Option<ComplexityTier> {
    match agent_type {
        "architect" | "planner" | "critic" | "analyst" => Some(ComplexityTier::High),
        "explore" | "writer" => Some(ComplexityTier::Low),
        "document-specialist"
        | "researcher"
        | "test-engineer"
        | "tdd-guide"
        | "executor"
        | "designer"
        | "vision" => Some(ComplexityTier::Medium),
        _ => None,
    }
}

/// Generate a complexity analysis summary for the orchestrator
pub fn analyze_task_complexity(
    task_prompt: &str,
    agent_type: Option<&str>,
) -> (ComplexityTier, String, String) {
    let ctx = RoutingContext {
        task_prompt: task_prompt.to_string(),
        agent_type: agent_type.map(String::from),
        ..Default::default()
    };
    let config = RoutingConfig::default();
    let signals = extract_all_signals(task_prompt, &ctx);
    let decision = route_task(&ctx, &config);

    let mut analysis = format!(
        "**Tier: {}** -> {}\n\n**Why:**",
        decision.tier, decision.model
    );
    for r in &decision.reasons {
        analysis.push_str(&format!("\n- {}", r));
    }
    analysis.push_str("\n\n**Signals detected:**");
    if signals.lexical.has_architecture_keywords {
        analysis.push_str("\n- Architecture keywords (refactor, redesign, etc.)");
    }
    if signals.lexical.has_risk_keywords {
        analysis.push_str("\n- Risk keywords (migration, production, critical)");
    }
    if signals.lexical.has_debugging_keywords {
        analysis.push_str("\n- Debugging keywords (root cause, investigate)");
    }
    if signals.structural.cross_file_dependencies {
        analysis.push_str("\n- Cross-file dependencies");
    }
    if signals.structural.impact_scope == ImpactScope::SystemWide {
        analysis.push_str("\n- System-wide impact");
    }
    if signals.structural.reversibility == Reversibility::Difficult {
        analysis.push_str("\n- Difficult to reverse");
    }

    (decision.tier, decision.model, analysis)
}

/// Explain routing decision for debugging/logging
pub fn explain_routing(ctx: &RoutingContext, config: &RoutingConfig) -> String {
    let decision = route_task(ctx, config);
    let signals = extract_all_signals(&ctx.task_prompt, ctx);

    let task_preview = if ctx.task_prompt.len() > 100 {
        format!("{}...", &ctx.task_prompt[..100])
    } else {
        ctx.task_prompt.clone()
    };

    let agent = ctx.agent_type.as_deref().unwrap_or("unspecified");

    format!(
        "=== Model Routing Decision ===\n\
         Task: {task}\n\
         Agent: {agent}\n\n\
         --- Signals ---\n\
         Word count: {wc}\n\
         File paths: {fp}\n\
         Architecture keywords: {ak}\n\
         Debugging keywords: {dk}\n\
         Simple keywords: {sk}\n\
         Risk keywords: {rk}\n\
         Question depth: {qd:?}\n\
         Estimated subtasks: {st}\n\
         Cross-file: {cf}\n\
         Impact scope: {is:?}\n\
         Reversibility: {rv:?}\n\
         Previous failures: {pf}\n\n\
         --- Decision ---\n\
         Tier: {tier}\n\
         Model: {model}\n\
         Confidence: {conf}\n\
         Escalated: {esc}\n\n\
         --- Reasons ---\n\
         {reasons}",
        task = task_preview,
        wc = signals.lexical.word_count,
        fp = signals.lexical.file_path_count,
        ak = signals.lexical.has_architecture_keywords,
        dk = signals.lexical.has_debugging_keywords,
        sk = signals.lexical.has_simple_keywords,
        rk = signals.lexical.has_risk_keywords,
        qd = signals.lexical.question_depth,
        st = signals.structural.estimated_subtasks,
        cf = signals.structural.cross_file_dependencies,
        is = signals.structural.impact_scope,
        rv = signals.structural.reversibility,
        pf = signals.context.previous_failures,
        tier = decision.tier,
        model = decision.model,
        conf = decision.confidence,
        esc = decision.escalated,
        reasons = decision
            .reasons
            .iter()
            .map(|r| format!("  - {}", r))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}
