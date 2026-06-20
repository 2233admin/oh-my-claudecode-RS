//! Routing Rules
//!
//! Defines the rules engine for model routing decisions.
//! Rules are evaluated in priority order, and the first matching rule wins.

use super::types::*;

/// A rule evaluation result
pub struct RuleResult {
    pub tier: Option<ComplexityTier>,
    pub reason: String,
    pub rule_name: String,
}

/// Build the default routing rules
pub fn default_routing_rules() -> Vec<RoutingRule> {
    vec![
        // ============ Override Rules (Highest Priority) ============
        RoutingRule {
            name: "architect-complex-debugging".into(),
            condition: Box::new(|ctx, signals| {
                ctx.agent_type.as_deref() == Some("architect")
                    && (signals.lexical.has_debugging_keywords
                        || signals.lexical.has_architecture_keywords
                        || signals.lexical.has_risk_keywords)
            }),
            tier: Some(ComplexityTier::High),
            reason: "Architect: Complex debugging/architecture decision".into(),
            priority: 85,
        },
        RoutingRule {
            name: "architect-simple-lookup".into(),
            condition: Box::new(|ctx, signals| {
                ctx.agent_type.as_deref() == Some("architect")
                    && signals.lexical.has_simple_keywords
                    && !signals.lexical.has_debugging_keywords
                    && !signals.lexical.has_architecture_keywords
                    && !signals.lexical.has_risk_keywords
            }),
            tier: Some(ComplexityTier::Low),
            reason: "Architect: Simple lookup query".into(),
            priority: 80,
        },
        RoutingRule {
            name: "planner-simple-breakdown".into(),
            condition: Box::new(|ctx, signals| {
                ctx.agent_type.as_deref() == Some("planner")
                    && signals.structural.estimated_subtasks <= 3
                    && !signals.lexical.has_risk_keywords
                    && signals.structural.impact_scope == ImpactScope::Local
            }),
            tier: Some(ComplexityTier::Low),
            reason: "Planner: Simple task breakdown".into(),
            priority: 75,
        },
        RoutingRule {
            name: "planner-strategic-planning".into(),
            condition: Box::new(|ctx, signals| {
                ctx.agent_type.as_deref() == Some("planner")
                    && (signals.structural.impact_scope == ImpactScope::SystemWide
                        || signals.lexical.has_architecture_keywords
                        || signals.structural.estimated_subtasks > 10)
            }),
            tier: Some(ComplexityTier::High),
            reason: "Planner: Cross-domain strategic planning".into(),
            priority: 75,
        },
        RoutingRule {
            name: "critic-checklist-review".into(),
            condition: Box::new(|ctx, signals| {
                ctx.agent_type.as_deref() == Some("critic")
                    && signals.lexical.word_count < 30
                    && !signals.lexical.has_risk_keywords
            }),
            tier: Some(ComplexityTier::Low),
            reason: "Critic: Checklist verification".into(),
            priority: 75,
        },
        RoutingRule {
            name: "critic-adversarial-review".into(),
            condition: Box::new(|ctx, signals| {
                ctx.agent_type.as_deref() == Some("critic")
                    && (signals.lexical.has_risk_keywords
                        || signals.structural.impact_scope == ImpactScope::SystemWide)
            }),
            tier: Some(ComplexityTier::High),
            reason: "Critic: Adversarial review for critical system".into(),
            priority: 75,
        },
        RoutingRule {
            name: "analyst-simple-impact".into(),
            condition: Box::new(|ctx, signals| {
                ctx.agent_type.as_deref() == Some("analyst")
                    && signals.structural.impact_scope == ImpactScope::Local
                    && !signals.lexical.has_risk_keywords
            }),
            tier: Some(ComplexityTier::Low),
            reason: "Analyst: Simple impact analysis".into(),
            priority: 75,
        },
        RoutingRule {
            name: "analyst-risk-analysis".into(),
            condition: Box::new(|ctx, signals| {
                ctx.agent_type.as_deref() == Some("analyst")
                    && (signals.lexical.has_risk_keywords
                        || signals.structural.impact_scope == ImpactScope::SystemWide)
            }),
            tier: Some(ComplexityTier::High),
            reason: "Analyst: Risk analysis and unknown-unknowns detection".into(),
            priority: 75,
        },
        // ============ Task-Based Rules ============
        RoutingRule {
            name: "architecture-system-wide".into(),
            condition: Box::new(|_ctx, signals| {
                signals.lexical.has_architecture_keywords
                    && signals.structural.impact_scope == ImpactScope::SystemWide
            }),
            tier: Some(ComplexityTier::High),
            reason: "Architectural decisions with system-wide impact".into(),
            priority: 70,
        },
        RoutingRule {
            name: "security-domain".into(),
            condition: Box::new(|_ctx, signals| {
                signals.structural.domain_specificity == Domain::Security
            }),
            tier: Some(ComplexityTier::High),
            reason: "Security-related tasks require careful reasoning".into(),
            priority: 70,
        },
        RoutingRule {
            name: "difficult-reversibility-risk".into(),
            condition: Box::new(|_ctx, signals| {
                signals.structural.reversibility == Reversibility::Difficult
                    && signals.lexical.has_risk_keywords
            }),
            tier: Some(ComplexityTier::High),
            reason: "High-risk, difficult-to-reverse changes".into(),
            priority: 70,
        },
        RoutingRule {
            name: "deep-debugging".into(),
            condition: Box::new(|_ctx, signals| {
                signals.lexical.has_debugging_keywords
                    && signals.lexical.question_depth == QuestionDepth::Why
            }),
            tier: Some(ComplexityTier::High),
            reason: "Root cause analysis requires deep reasoning".into(),
            priority: 65,
        },
        RoutingRule {
            name: "complex-multi-step".into(),
            condition: Box::new(|_ctx, signals| {
                signals.structural.estimated_subtasks > 5
                    && signals.structural.cross_file_dependencies
            }),
            tier: Some(ComplexityTier::High),
            reason: "Complex multi-step task with cross-file changes".into(),
            priority: 60,
        },
        RoutingRule {
            name: "simple-search-query".into(),
            condition: Box::new(|_ctx, signals| {
                signals.lexical.has_simple_keywords
                    && signals.structural.estimated_subtasks <= 1
                    && signals.structural.impact_scope == ImpactScope::Local
                    && !signals.lexical.has_architecture_keywords
                    && !signals.lexical.has_debugging_keywords
            }),
            tier: Some(ComplexityTier::Low),
            reason: "Simple search or lookup task".into(),
            priority: 60,
        },
        RoutingRule {
            name: "short-local-change".into(),
            condition: Box::new(|_ctx, signals| {
                signals.lexical.word_count < 50
                    && signals.structural.impact_scope == ImpactScope::Local
                    && signals.structural.reversibility == Reversibility::Easy
                    && !signals.lexical.has_risk_keywords
            }),
            tier: Some(ComplexityTier::Low),
            reason: "Short, local, easily reversible change".into(),
            priority: 55,
        },
        RoutingRule {
            name: "moderate-complexity".into(),
            condition: Box::new(|_ctx, signals| {
                signals.structural.estimated_subtasks > 1
                    && signals.structural.estimated_subtasks <= 5
            }),
            tier: Some(ComplexityTier::Medium),
            reason: "Moderate complexity with multiple subtasks".into(),
            priority: 50,
        },
        RoutingRule {
            name: "module-level-work".into(),
            condition: Box::new(|_ctx, signals| {
                signals.structural.impact_scope == ImpactScope::Module
            }),
            tier: Some(ComplexityTier::Medium),
            reason: "Module-level changes".into(),
            priority: 45,
        },
        // ============ Default Rule ============
        RoutingRule {
            name: "default-medium".into(),
            condition: Box::new(|_ctx, _signals| true),
            tier: Some(ComplexityTier::Medium),
            reason: "Default tier for unclassified tasks".into(),
            priority: 0,
        },
    ]
}

/// Evaluate routing rules and return the first matching rule's action
pub fn evaluate_rules(
    ctx: &RoutingContext,
    signals: &ComplexitySignals,
    rules: &[RoutingRule],
) -> RuleResult {
    let mut sorted: Vec<&RoutingRule> = rules.iter().collect();
    sorted.sort_by_key(|r| std::cmp::Reverse(r.priority));

    for rule in &sorted {
        if (rule.condition)(ctx, signals) {
            return RuleResult {
                tier: rule.tier,
                reason: rule.reason.clone(),
                rule_name: rule.name.clone(),
            };
        }
    }

    RuleResult {
        tier: Some(ComplexityTier::Medium),
        reason: "Fallback to medium tier".into(),
        rule_name: "fallback".into(),
    }
}

/// Get all rules that would match for a given context (for debugging)
pub fn get_matching_rules<'a>(
    ctx: &RoutingContext,
    signals: &ComplexitySignals,
    rules: &'a [RoutingRule],
) -> Vec<&'a RoutingRule> {
    rules
        .iter()
        .filter(|rule| (rule.condition)(ctx, signals))
        .collect()
}
