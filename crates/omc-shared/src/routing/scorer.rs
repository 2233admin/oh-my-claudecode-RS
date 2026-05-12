//! Complexity Scorer
//!
//! Calculates complexity tier based on extracted signals.
//! Uses weighted scoring to determine LOW/MEDIUM/HIGH tier.

use super::types::*;

/// Score thresholds for tier classification
const TIER_THRESHOLD_HIGH: f64 = 8.0;
const TIER_THRESHOLD_MEDIUM: f64 = 4.0;

// Weight constants
mod w {
    // Lexical weights
    pub const WORD_COUNT_HIGH: f64 = 2.0;
    pub const WORD_COUNT_VERY_HIGH: f64 = 1.0;
    pub const FILE_PATHS_MULTIPLE: f64 = 1.0;
    pub const CODE_BLOCKS_PRESENT: f64 = 1.0;
    pub const ARCHITECTURE_KEYWORDS: f64 = 3.0;
    pub const DEBUGGING_KEYWORDS: f64 = 2.0;
    pub const SIMPLE_KEYWORDS: f64 = -2.0;
    pub const RISK_KEYWORDS: f64 = 2.0;
    pub const QUESTION_DEPTH_WHY: f64 = 2.0;
    pub const QUESTION_DEPTH_HOW: f64 = 1.0;
    pub const IMPLICIT_REQUIREMENTS: f64 = 1.0;

    // Structural weights
    pub const SUBTASKS_MANY: f64 = 3.0;
    pub const SUBTASKS_SOME: f64 = 1.0;
    pub const CROSS_FILE: f64 = 2.0;
    pub const TEST_REQUIRED: f64 = 1.0;
    pub const SECURITY_DOMAIN: f64 = 2.0;
    pub const INFRA_DOMAIN: f64 = 1.0;
    pub const EXTERNAL_KNOWLEDGE: f64 = 1.0;
    pub const REVERSIBILITY_DIFFICULT: f64 = 2.0;
    pub const REVERSIBILITY_MODERATE: f64 = 1.0;
    pub const IMPACT_SYSTEM_WIDE: f64 = 3.0;
    pub const IMPACT_MODULE: f64 = 1.0;

    // Context weights
    pub const PREVIOUS_FAILURE: f64 = 2.0;
    pub const PREVIOUS_FAILURE_MAX: f64 = 4.0;
    pub const DEEP_CHAIN: f64 = 2.0;
    pub const COMPLEX_PLAN: f64 = 1.0;
}

fn score_lexical(s: &LexicalSignals) -> f64 {
    let mut score = 0.0;

    if s.word_count > 200 {
        score += w::WORD_COUNT_HIGH;
        if s.word_count > 500 {
            score += w::WORD_COUNT_VERY_HIGH;
        }
    }

    if s.file_path_count >= 2 {
        score += w::FILE_PATHS_MULTIPLE;
    }

    if s.code_block_count > 0 {
        score += w::CODE_BLOCKS_PRESENT;
    }

    if s.has_architecture_keywords {
        score += w::ARCHITECTURE_KEYWORDS;
    }
    if s.has_debugging_keywords {
        score += w::DEBUGGING_KEYWORDS;
    }
    if s.has_simple_keywords {
        score += w::SIMPLE_KEYWORDS;
    }
    if s.has_risk_keywords {
        score += w::RISK_KEYWORDS;
    }

    match s.question_depth {
        QuestionDepth::Why => score += w::QUESTION_DEPTH_WHY,
        QuestionDepth::How => score += w::QUESTION_DEPTH_HOW,
        _ => {}
    }

    if s.has_implicit_requirements {
        score += w::IMPLICIT_REQUIREMENTS;
    }

    score
}

fn score_structural(s: &StructuralSignals) -> f64 {
    let mut score = 0.0;

    if s.estimated_subtasks > 3 {
        score += w::SUBTASKS_MANY;
    } else if s.estimated_subtasks > 1 {
        score += w::SUBTASKS_SOME;
    }

    if s.cross_file_dependencies {
        score += w::CROSS_FILE;
    }

    if s.has_test_requirements {
        score += w::TEST_REQUIRED;
    }

    match s.domain_specificity {
        Domain::Security => score += w::SECURITY_DOMAIN,
        Domain::Infrastructure => score += w::INFRA_DOMAIN,
        _ => {}
    }

    if s.requires_external_knowledge {
        score += w::EXTERNAL_KNOWLEDGE;
    }

    match s.reversibility {
        Reversibility::Difficult => score += w::REVERSIBILITY_DIFFICULT,
        Reversibility::Moderate => score += w::REVERSIBILITY_MODERATE,
        _ => {}
    }

    match s.impact_scope {
        ImpactScope::SystemWide => score += w::IMPACT_SYSTEM_WIDE,
        ImpactScope::Module => score += w::IMPACT_MODULE,
        _ => {}
    }

    score
}

fn score_context(s: &ContextSignals) -> f64 {
    let mut score = 0.0;

    let failure_score =
        (s.previous_failures as f64 * w::PREVIOUS_FAILURE).min(w::PREVIOUS_FAILURE_MAX);
    score += failure_score;

    if s.agent_chain_depth >= 3 {
        score += w::DEEP_CHAIN;
    }

    if s.plan_complexity >= 5 {
        score += w::COMPLEX_PLAN;
    }

    score
}

/// Calculate total complexity score
pub fn calculate_complexity_score(signals: &ComplexitySignals) -> f64 {
    score_lexical(&signals.lexical)
        + score_structural(&signals.structural)
        + score_context(&signals.context)
}

/// Determine complexity tier from score
pub fn score_to_tier(score: f64) -> ComplexityTier {
    if score >= TIER_THRESHOLD_HIGH {
        ComplexityTier::High
    } else if score >= TIER_THRESHOLD_MEDIUM {
        ComplexityTier::Medium
    } else {
        ComplexityTier::Low
    }
}

/// Calculate complexity tier from signals
pub fn calculate_complexity_tier(signals: &ComplexitySignals) -> ComplexityTier {
    score_to_tier(calculate_complexity_score(signals))
}

/// Score breakdown for debugging/logging
pub struct ScoreBreakdown {
    pub lexical: f64,
    pub structural: f64,
    pub context: f64,
    pub total: f64,
    pub tier: ComplexityTier,
}

/// Get detailed score breakdown
pub fn get_score_breakdown(signals: &ComplexitySignals) -> ScoreBreakdown {
    let lexical = score_lexical(&signals.lexical);
    let structural = score_structural(&signals.structural);
    let context = score_context(&signals.context);
    let total = lexical + structural + context;

    ScoreBreakdown {
        lexical,
        structural,
        context,
        total,
        tier: score_to_tier(total),
    }
}

/// Calculate confidence in the tier assignment.
/// Higher confidence when score is far from thresholds.
pub fn calculate_confidence(score: f64, tier: ComplexityTier) -> f64 {
    let min_distance = match tier {
        ComplexityTier::Low => TIER_THRESHOLD_MEDIUM - score,
        ComplexityTier::Medium => {
            let d_low = (score - TIER_THRESHOLD_MEDIUM).abs();
            let d_high = (score - TIER_THRESHOLD_HIGH).abs();
            d_low.min(d_high)
        }
        ComplexityTier::High => score - TIER_THRESHOLD_HIGH,
    };

    let confidence = 0.5 + (min_distance.min(4.0) / 4.0) * 0.4;
    (confidence * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_signals() -> ComplexitySignals {
        ComplexitySignals {
            lexical: LexicalSignals {
                word_count: 10,
                file_path_count: 0,
                code_block_count: 0,
                has_architecture_keywords: false,
                has_debugging_keywords: false,
                has_simple_keywords: false,
                has_risk_keywords: false,
                question_depth: QuestionDepth::None,
                has_implicit_requirements: false,
            },
            structural: StructuralSignals {
                estimated_subtasks: 1,
                cross_file_dependencies: false,
                has_test_requirements: false,
                domain_specificity: Domain::Generic,
                requires_external_knowledge: false,
                reversibility: Reversibility::Easy,
                impact_scope: ImpactScope::Local,
            },
            context: ContextSignals {
                previous_failures: 0,
                conversation_turns: 0,
                plan_complexity: 0,
                remaining_tasks: 0,
                agent_chain_depth: 0,
            },
        }
    }

    #[test]
    fn empty_signals_score_low() {
        let signals = default_signals();
        let score = calculate_complexity_score(&signals);
        assert_eq!(score, 0.0);
        assert_eq!(score_to_tier(score), ComplexityTier::Low);
    }

    #[test]
    fn high_complexity_signals() {
        let mut signals = default_signals();
        signals.lexical.word_count = 600;
        signals.lexical.has_architecture_keywords = true;
        signals.structural.estimated_subtasks = 5;
        signals.structural.cross_file_dependencies = true;
        signals.structural.impact_scope = ImpactScope::SystemWide;
        signals.structural.reversibility = Reversibility::Difficult;
        let score = calculate_complexity_score(&signals);
        assert!(score >= 8.0, "expected High tier, score was {}", score);
        assert_eq!(score_to_tier(score), ComplexityTier::High);
    }

    #[test]
    fn simple_keywords_reduce_score() {
        let mut signals = default_signals();
        signals.lexical.has_simple_keywords = true;
        let score = calculate_complexity_score(&signals);
        assert!(score < 0.0, "simple keywords should make score negative");
    }

    #[test]
    fn score_to_tier_thresholds() {
        assert_eq!(score_to_tier(0.0), ComplexityTier::Low);
        assert_eq!(score_to_tier(3.99), ComplexityTier::Low);
        assert_eq!(score_to_tier(4.0), ComplexityTier::Medium);
        assert_eq!(score_to_tier(7.99), ComplexityTier::Medium);
        assert_eq!(score_to_tier(8.0), ComplexityTier::High);
        assert_eq!(score_to_tier(20.0), ComplexityTier::High);
    }

    #[test]
    fn previous_failures_capped_at_4() {
        let mut signals = default_signals();
        signals.context.previous_failures = 100;
        let score = calculate_complexity_score(&signals);
        assert_eq!(score, 4.0);
    }

    #[test]
    fn calculate_confidence_high_when_far_from_threshold() {
        let confidence = calculate_confidence(0.0, ComplexityTier::Low);
        assert_eq!(confidence, 0.9);
    }

    #[test]
    fn calculate_confidence_at_threshold_boundary() {
        let confidence = calculate_confidence(4.0, ComplexityTier::Medium);
        assert_eq!(confidence, 0.5);
    }

    #[test]
    fn get_score_breakdown_structure() {
        let mut signals = default_signals();
        signals.lexical.has_architecture_keywords = true;
        signals.structural.cross_file_dependencies = true;
        let breakdown = get_score_breakdown(&signals);
        assert_eq!(breakdown.lexical, 3.0);
        assert_eq!(breakdown.structural, 2.0);
        assert_eq!(breakdown.context, 0.0);
        assert_eq!(breakdown.total, 5.0);
        assert_eq!(breakdown.tier, ComplexityTier::Medium);
    }
}
