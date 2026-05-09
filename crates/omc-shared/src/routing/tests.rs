#[cfg(test)]
mod routing_tests {
    use super::super::*;

    #[test]
    fn test_simple_search_routes_to_low() {
        let ctx = RoutingContext {
            task_prompt: "Find where authentication is implemented".into(),
            agent_type: Some("explore".into()),
            ..Default::default()
        };
        let config = RoutingConfig::default();
        let decision = route_task(&ctx, &config);
        assert_eq!(decision.tier, ComplexityTier::Low);
        assert_eq!(decision.model_type, ModelType::Haiku);
    }

    #[test]
    fn test_architecture_system_wide_routes_to_high() {
        let ctx = RoutingContext {
            task_prompt:
                "Redesign the entire authentication system architecture across the whole project"
                    .into(),
            agent_type: Some("architect".into()),
            ..Default::default()
        };
        let config = RoutingConfig::default();
        let decision = route_task(&ctx, &config);
        assert_eq!(decision.tier, ComplexityTier::High);
        assert_eq!(decision.model_type, ModelType::Opus);
    }

    #[test]
    fn test_security_domain_routes_to_high() {
        let ctx = RoutingContext {
            task_prompt: "Fix the OAuth JWT token validation vulnerability in the auth middleware"
                .into(),
            ..Default::default()
        };
        let config = RoutingConfig::default();
        let decision = route_task(&ctx, &config);
        assert_eq!(decision.tier, ComplexityTier::High);
    }

    #[test]
    fn test_force_inherit_bypasses_routing() {
        let ctx = RoutingContext {
            task_prompt: "Refactor the entire production system".into(),
            ..Default::default()
        };
        let config = RoutingConfig {
            force_inherit: true,
            ..Default::default()
        };
        let decision = route_task(&ctx, &config);
        assert_eq!(decision.model_type, ModelType::Inherit);
        assert_eq!(decision.confidence, 1.0);
    }

    #[test]
    fn test_disabled_routing_uses_default() {
        let ctx = RoutingContext {
            task_prompt: "Some task".into(),
            ..Default::default()
        };
        let config = RoutingConfig {
            enabled: false,
            default_tier: ComplexityTier::Medium,
            ..Default::default()
        };
        let decision = route_task(&ctx, &config);
        assert_eq!(decision.tier, ComplexityTier::Medium);
    }

    #[test]
    fn test_escalate_model() {
        assert_eq!(escalate_model(ComplexityTier::Low), ComplexityTier::Medium);
        assert_eq!(escalate_model(ComplexityTier::Medium), ComplexityTier::High);
        assert_eq!(escalate_model(ComplexityTier::High), ComplexityTier::High);
    }

    #[test]
    fn test_can_escalate() {
        assert!(can_escalate(ComplexityTier::Low));
        assert!(can_escalate(ComplexityTier::Medium));
        assert!(!can_escalate(ComplexityTier::High));
    }

    #[test]
    fn test_quick_tier_for_agent() {
        assert_eq!(
            quick_tier_for_agent("architect"),
            Some(ComplexityTier::High)
        );
        assert_eq!(quick_tier_for_agent("explore"), Some(ComplexityTier::Low));
        assert_eq!(
            quick_tier_for_agent("executor"),
            Some(ComplexityTier::Medium)
        );
        assert_eq!(quick_tier_for_agent("unknown-agent"), None);
    }

    #[test]
    fn test_score_to_tier_thresholds() {
        assert_eq!(score_to_tier(2.0), ComplexityTier::Low);
        assert_eq!(score_to_tier(4.0), ComplexityTier::Medium);
        assert_eq!(score_to_tier(8.0), ComplexityTier::High);
        assert_eq!(score_to_tier(15.0), ComplexityTier::High);
    }

    #[test]
    fn test_adapt_prompt_for_tier() {
        let base = "Fix the bug in auth.rs";

        let haiku = adapt_prompt_for_tier(base, ComplexityTier::Low);
        assert!(haiku.starts_with("TASK:"));
        assert!(haiku.contains("Return results directly"));

        let sonnet = adapt_prompt_for_tier(base, ComplexityTier::Medium);
        assert!(sonnet.contains("Task Execution Mode"));

        let opus = adapt_prompt_for_tier(base, ComplexityTier::High);
        assert!(opus.contains("thinking_mode"));
        assert!(opus.contains("Before Concluding"));
    }

    #[test]
    fn test_lexical_signal_extraction() {
        let signals = extract_lexical_signals("Find the main.rs file in src/ directory");
        assert!(signals.has_simple_keywords);
        assert!(!signals.has_architecture_keywords);
        assert!(signals.file_path_count > 0);
    }

    #[test]
    fn test_structural_signals_risk() {
        let signals =
            extract_structural_signals("Migrate the production database, this is irreversible");
        assert_eq!(signals.reversibility, Reversibility::Difficult);
    }

    #[test]
    fn test_route_and_adapt_convenience() {
        let (decision, adapted) =
            route_and_adapt_task("Find where auth is implemented", Some("explore"), None);
        assert_eq!(decision.tier, ComplexityTier::Low);
        assert!(adapted.contains("TASK:"));
    }

    #[test]
    fn test_confidence_near_threshold() {
        // Score right at threshold should have lower confidence
        let c_at = calculate_confidence(4.0, ComplexityTier::Medium);
        // Score far from threshold should have higher confidence
        let c_far = calculate_confidence(1.0, ComplexityTier::Low);
        assert!(c_far > c_at);
    }

    #[test]
    fn test_min_tier_enforcement() {
        let ctx = RoutingContext {
            task_prompt: "Find auth".into(),
            ..Default::default()
        };
        let config = RoutingConfig {
            min_tier: Some(ComplexityTier::Medium),
            ..Default::default()
        };
        let decision = route_task(&ctx, &config);
        assert!(decision.tier.index() >= ComplexityTier::Medium.index());
    }
}
