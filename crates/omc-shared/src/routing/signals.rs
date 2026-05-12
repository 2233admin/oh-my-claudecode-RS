//! Complexity Signal Extraction
//!
//! Extracts complexity signals from task prompts to inform routing decisions.
//! Signals are categorized into lexical, structural, and context types.

use once_cell::sync::Lazy;
use regex::Regex;

use super::types::*;

/// Extract lexical signals from task prompt (fast, regex-based)
pub fn extract_lexical_signals(prompt: &str) -> LexicalSignals {
    let lower = prompt.to_lowercase();
    let word_count = prompt.split_whitespace().count();

    LexicalSignals {
        word_count,
        file_path_count: count_file_paths(prompt),
        code_block_count: count_code_blocks(prompt),
        has_architecture_keywords: has_keywords(&lower, ComplexityKeywords::ARCHITECTURE),
        has_debugging_keywords: has_keywords(&lower, ComplexityKeywords::DEBUGGING),
        has_simple_keywords: has_keywords(&lower, ComplexityKeywords::SIMPLE),
        has_risk_keywords: has_keywords(&lower, ComplexityKeywords::RISK),
        question_depth: detect_question_depth(&lower),
        has_implicit_requirements: detect_implicit_requirements(&lower),
    }
}

/// Extract structural signals from task prompt
pub fn extract_structural_signals(prompt: &str) -> StructuralSignals {
    let lower = prompt.to_lowercase();

    StructuralSignals {
        estimated_subtasks: estimate_subtasks(prompt),
        cross_file_dependencies: detect_cross_file_dependencies(prompt),
        has_test_requirements: detect_test_requirements(&lower),
        domain_specificity: detect_domain(&lower),
        requires_external_knowledge: detect_external_knowledge(&lower),
        reversibility: assess_reversibility(&lower),
        impact_scope: assess_impact_scope(prompt),
    }
}

/// Extract context signals from routing context
pub fn extract_context_signals(ctx: &RoutingContext) -> ContextSignals {
    ContextSignals {
        previous_failures: ctx.previous_failures.unwrap_or(0),
        conversation_turns: ctx.conversation_turns.unwrap_or(0),
        plan_complexity: ctx.plan_tasks.unwrap_or(0),
        remaining_tasks: ctx.remaining_tasks.unwrap_or(0),
        agent_chain_depth: ctx.agent_chain_depth.unwrap_or(0),
    }
}

/// Extract all complexity signals
pub fn extract_all_signals(prompt: &str, ctx: &RoutingContext) -> ComplexitySignals {
    ComplexitySignals {
        lexical: extract_lexical_signals(prompt),
        structural: extract_structural_signals(prompt),
        context: extract_context_signals(ctx),
    }
}

// ============ Helper Functions ============

fn has_keywords(text: &str, keywords: &[&str]) -> bool {
    keywords.iter().any(|kw| text.contains(kw))
}

static FILE_PATH_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?m)(?:^|\s)[.~/]?[\w-]+(?:/[\w-]+)*[\w.-]+\.\w+"#).unwrap());
static BACKTICK_FILE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r#"`[^`]+\.\w+`"#).unwrap());
static QUOTED_FILE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"'[^']+\.\w+'|"[^"]+\.\w+""#).unwrap());

fn count_file_paths(prompt: &str) -> usize {
    let mut count = 0usize;
    count += FILE_PATH_RE.find_iter(prompt).count();
    count += BACKTICK_FILE_RE.find_iter(prompt).count();
    count += QUOTED_FILE_RE.find_iter(prompt).count();
    count.min(20)
}

static FENCED_BLOCK_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r#"```[\s\S]*?```"#).unwrap());
static INDENTED_BLOCK_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?m)(?:^|\n)(?: {4}|\t)[^\n]+(?:\n(?: {4}|\t)[^\n]+)*"#).unwrap());

fn count_code_blocks(prompt: &str) -> usize {
    let fenced = FENCED_BLOCK_RE.find_iter(prompt).count();
    let indented = INDENTED_BLOCK_RE.find_iter(prompt).count();
    fenced + indented / 2
}

static WHY_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?i)\bwhy\b.*\?|\bwhy\s+(is|are|does|do|did|would|should|can)"#).unwrap()
});
static HOW_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?i)\bhow\b.*\?|\bhow\s+(do|does|can|should|would|to)"#).unwrap());
static WHAT_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?i)\bwhat\b.*\?|\bwhat\s+(is|are|does|do)"#).unwrap());
static WHERE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?i)\bwhere\b.*\?|\bwhere\s+(is|are|does|do|can)"#).unwrap());

fn detect_question_depth(prompt: &str) -> QuestionDepth {
    if WHY_RE.is_match(prompt) {
        QuestionDepth::Why
    } else if HOW_RE.is_match(prompt) {
        QuestionDepth::How
    } else if WHAT_RE.is_match(prompt) {
        QuestionDepth::What
    } else if WHERE_RE.is_match(prompt) {
        QuestionDepth::Where
    } else {
        QuestionDepth::None
    }
}

static VAGUE_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        Regex::new(r#"(?i)\bmake it better\b"#).unwrap(),
        Regex::new(r#"(?i)\bimprove\b"#).unwrap(),
        Regex::new(r#"(?i)\bfix\b"#).unwrap(),
        Regex::new(r#"(?i)\boptimize\b"#).unwrap(),
        Regex::new(r#"(?i)\bclean up\b"#).unwrap(),
        Regex::new(r#"(?i)\brefactor\b"#).unwrap(),
    ]
});

fn detect_implicit_requirements(prompt: &str) -> bool {
    VAGUE_PATTERNS.iter().any(|p| p.is_match(prompt))
}

static BULLET_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?m)^\s*[-*•]\s"#).unwrap());
static NUMBERED_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?m)^\s*\d+[.)]\s"#).unwrap());

fn estimate_subtasks(prompt: &str) -> usize {
    let mut count: usize = 1;
    count += BULLET_RE.find_iter(prompt).count();
    count += NUMBERED_RE.find_iter(prompt).count();
    let and_word_count = prompt
        .split_whitespace()
        .filter(|w| w.eq_ignore_ascii_case("and"))
        .count();
    count += and_word_count / 2;
    let then_count = prompt
        .split_whitespace()
        .filter(|w| w.eq_ignore_ascii_case("then"))
        .count();
    count += then_count;
    count.min(10)
}

static CROSS_FILE_RE: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        Regex::new(r#"(?i)multiple files"#).unwrap(),
        Regex::new(r#"(?i)across.*files"#).unwrap(),
        Regex::new(r#"(?i)several.*files"#).unwrap(),
        Regex::new(r#"(?i)all.*files"#).unwrap(),
        Regex::new(r#"(?i)throughout.*codebase"#).unwrap(),
        Regex::new(r#"(?i)entire.*project"#).unwrap(),
        Regex::new(r#"(?i)whole.*system"#).unwrap(),
    ]
});

fn detect_cross_file_dependencies(prompt: &str) -> bool {
    if count_file_paths(prompt) >= 2 {
        return true;
    }
    CROSS_FILE_RE.iter().any(|p| p.is_match(prompt))
}

static TEST_RE: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        Regex::new(r#"(?i)\btests?\b"#).unwrap(),
        Regex::new(r#"(?i)\bspec\b"#).unwrap(),
        Regex::new(r#"(?i)make sure.*work"#).unwrap(),
        Regex::new(r#"(?i)verify"#).unwrap(),
        Regex::new(r#"(?i)ensure.*pass"#).unwrap(),
        Regex::new(r#"\bTDD\b"#).unwrap(),
        Regex::new(r#"(?i)unit test"#).unwrap(),
        Regex::new(r#"(?i)integration test"#).unwrap(),
    ]
});

fn detect_test_requirements(prompt: &str) -> bool {
    TEST_RE.iter().any(|p| p.is_match(prompt))
}

fn detect_domain(prompt: &str) -> Domain {
    static FRONTEND_RE: Lazy<Vec<Regex>> = Lazy::new(|| {
        vec![
            Regex::new(r#"(?i)\b(react|vue|angular|svelte|css|html|jsx|tsx|component|ui|ux|styling|tailwind|sass|scss)\b"#).unwrap(),
            Regex::new(r#"(?i)\b(button|modal|form|input|layout|responsive|animation)\b"#).unwrap(),
        ]
    });
    static BACKEND_RE: Lazy<Vec<Regex>> = Lazy::new(|| {
        vec![
            Regex::new(
                r#"(?i)\b(api|endpoint|database|query|sql|graphql|rest|server|auth|middleware)\b"#,
            )
            .unwrap(),
            Regex::new(r#"(?i)\b(node|express|fastify|nest|django|flask|rails)\b"#).unwrap(),
        ]
    });
    static INFRA_RE: Lazy<Vec<Regex>> = Lazy::new(|| {
        vec![
            Regex::new(
                r#"(?i)\b(docker|kubernetes|k8s|terraform|aws|gcp|azure|ci|cd|deploy|container)\b"#,
            )
            .unwrap(),
            Regex::new(r#"(?i)\b(nginx|load.?balancer|scaling|monitoring|logging)\b"#).unwrap(),
        ]
    });
    static SECURITY_RE: Lazy<Vec<Regex>> = Lazy::new(|| {
        vec![
            Regex::new(
                r#"(?i)\b(security|auth|oauth|jwt|encryption|vulnerability|xss|csrf|injection)\b"#,
            )
            .unwrap(),
            Regex::new(r#"(?i)\b(password|credential|secret|token|permission)\b"#).unwrap(),
        ]
    });

    if SECURITY_RE.iter().any(|p| p.is_match(prompt)) {
        return Domain::Security;
    }
    if INFRA_RE.iter().any(|p| p.is_match(prompt)) {
        return Domain::Infrastructure;
    }
    if BACKEND_RE.iter().any(|p| p.is_match(prompt)) {
        return Domain::Backend;
    }
    if FRONTEND_RE.iter().any(|p| p.is_match(prompt)) {
        return Domain::Frontend;
    }
    Domain::Generic
}

static EXTERNAL_KNOWLEDGE_RE: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        Regex::new(r#"(?i)\bdocs?\b"#).unwrap(),
        Regex::new(r#"(?i)\bdocumentation\b"#).unwrap(),
        Regex::new(r#"(?i)\bofficial\b"#).unwrap(),
        Regex::new(r#"(?i)\blibrary\b"#).unwrap(),
        Regex::new(r#"(?i)\bpackage\b"#).unwrap(),
        Regex::new(r#"(?i)\bframework\b"#).unwrap(),
        Regex::new(r#"(?i)\bhow does.*work\b"#).unwrap(),
        Regex::new(r#"(?i)\bbest practice"#).unwrap(),
    ]
});

fn detect_external_knowledge(prompt: &str) -> bool {
    EXTERNAL_KNOWLEDGE_RE.iter().any(|p| p.is_match(prompt))
}

static DIFFICULT_RE: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        Regex::new(r#"(?i)\bmigrat"#).unwrap(),
        Regex::new(r#"(?i)\bproduction\b"#).unwrap(),
        Regex::new(r#"(?i)\bdata.*loss"#).unwrap(),
        Regex::new(r#"(?i)\bdelete.*all"#).unwrap(),
        Regex::new(r#"(?i)\bdrop.*table"#).unwrap(),
        Regex::new(r#"(?i)\birreversible"#).unwrap(),
        Regex::new(r#"(?i)\bpermanent"#).unwrap(),
    ]
});
static MODERATE_RE: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        Regex::new(r#"(?i)\brefactor"#).unwrap(),
        Regex::new(r#"(?i)\brestructure"#).unwrap(),
        Regex::new(r#"(?i)\brename.*across"#).unwrap(),
        Regex::new(r#"(?i)\bmove.*files"#).unwrap(),
        Regex::new(r#"(?i)\bchange.*schema"#).unwrap(),
    ]
});

fn assess_reversibility(prompt: &str) -> Reversibility {
    if DIFFICULT_RE.iter().any(|p| p.is_match(prompt)) {
        return Reversibility::Difficult;
    }
    if MODERATE_RE.iter().any(|p| p.is_match(prompt)) {
        return Reversibility::Moderate;
    }
    Reversibility::Easy
}

static SYSTEM_WIDE_RE: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        Regex::new(r#"(?i)\bentire\b"#).unwrap(),
        Regex::new(r#"(?i)\ball\s+(?:files|components|modules)"#).unwrap(),
        Regex::new(r#"(?i)\bwhole\s+(?:project|codebase|system)"#).unwrap(),
        Regex::new(r#"(?i)\bsystem.?wide"#).unwrap(),
        Regex::new(r#"(?i)\bglobal"#).unwrap(),
        Regex::new(r#"(?i)\beverywhere"#).unwrap(),
        Regex::new(r#"(?i)\bthroughout"#).unwrap(),
    ]
});
static MODULE_RE: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        Regex::new(r#"(?i)\bmodule"#).unwrap(),
        Regex::new(r#"(?i)\bpackage"#).unwrap(),
        Regex::new(r#"(?i)\bservice"#).unwrap(),
        Regex::new(r#"(?i)\bfeature"#).unwrap(),
        Regex::new(r#"(?i)\bcomponent"#).unwrap(),
        Regex::new(r#"(?i)\blayer"#).unwrap(),
    ]
});

fn assess_impact_scope(prompt: &str) -> ImpactScope {
    if SYSTEM_WIDE_RE.iter().any(|p| p.is_match(prompt)) {
        return ImpactScope::SystemWide;
    }
    if count_file_paths(prompt) >= 3 {
        return ImpactScope::Module;
    }
    if MODULE_RE.iter().any(|p| p.is_match(prompt)) {
        return ImpactScope::Module;
    }
    ImpactScope::Local
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_prompt() {
        let signals = extract_lexical_signals("");
        assert_eq!(signals.word_count, 0);
        assert_eq!(signals.file_path_count, 0);
        assert_eq!(signals.question_depth, QuestionDepth::None);
    }

    #[test]
    fn word_count_basic() {
        let signals = extract_lexical_signals("one two three four five");
        assert_eq!(signals.word_count, 5);
    }

    #[test]
    fn architecture_keywords_detected() {
        let signals = extract_lexical_signals("refactor the entire architecture of this system");
        assert!(signals.has_architecture_keywords);
    }

    #[test]
    fn debugging_keywords_detected() {
        let signals = extract_lexical_signals("find the root cause of this bug");
        assert!(signals.has_debugging_keywords);
    }

    #[test]
    fn simple_keywords_detected() {
        let signals = extract_lexical_signals("fix the typo in readme");
        assert!(signals.has_simple_keywords);
    }

    #[test]
    fn risk_keywords_detected() {
        let signals = extract_lexical_signals("run the production migration");
        assert!(signals.has_risk_keywords);
    }

    #[test]
    fn question_depth_why() {
        let signals = extract_lexical_signals("why does the cache fail?");
        assert_eq!(signals.question_depth, QuestionDepth::Why);
    }

    #[test]
    fn question_depth_how() {
        let signals = extract_lexical_signals("how do I configure the router?");
        assert_eq!(signals.question_depth, QuestionDepth::How);
    }

    #[test]
    fn question_depth_none() {
        let signals = extract_lexical_signals("add a new button to the UI");
        assert_eq!(signals.question_depth, QuestionDepth::None);
    }

    #[test]
    fn file_path_counting() {
        let prompt = "Edit src/main.rs and src/lib.rs and Cargo.toml";
        let signals = extract_lexical_signals(prompt);
        assert!(
            signals.file_path_count >= 3,
            "expected >= 3 file paths, got {}",
            signals.file_path_count
        );
    }

    #[test]
    fn code_block_counting() {
        let prompt = "Use this code:\n```\nfn main() {}\n```\nAnd also:\n```\nlet x = 1;\n```";
        let signals = extract_lexical_signals(prompt);
        assert!(
            signals.code_block_count >= 2,
            "expected >= 2 code blocks, got {}",
            signals.code_block_count
        );
    }

    #[test]
    fn structural_cross_file_detection() {
        let signals = extract_structural_signals("update multiple files across the codebase");
        assert!(signals.cross_file_dependencies);
    }

    #[test]
    fn structural_test_requirements() {
        let signals = extract_structural_signals("add unit tests for this module");
        assert!(signals.has_test_requirements);
    }

    #[test]
    fn domain_security() {
        let signals = extract_structural_signals("add OAuth2 authentication with JWT tokens");
        assert_eq!(signals.domain_specificity, Domain::Security);
    }

    #[test]
    fn domain_infrastructure() {
        let signals = extract_structural_signals("create a Docker container for deployment");
        assert_eq!(signals.domain_specificity, Domain::Infrastructure);
    }

    #[test]
    fn domain_generic() {
        let signals = extract_structural_signals("sort the list alphabetically");
        assert_eq!(signals.domain_specificity, Domain::Generic);
    }

    #[test]
    fn reversibility_difficult() {
        let signals = extract_structural_signals("migrate the database schema in production");
        assert_eq!(signals.reversibility, Reversibility::Difficult);
    }

    #[test]
    fn reversibility_moderate() {
        let signals = extract_structural_signals("refactor the auth module");
        assert_eq!(signals.reversibility, Reversibility::Moderate);
    }

    #[test]
    fn impact_scope_system_wide() {
        let signals = extract_structural_signals("update the entire codebase with new logging");
        assert_eq!(signals.impact_scope, ImpactScope::SystemWide);
    }

    #[test]
    fn impact_scope_module() {
        let signals = extract_structural_signals("change the auth service module");
        assert_eq!(signals.impact_scope, ImpactScope::Module);
    }

    #[test]
    fn context_signals_from_context() {
        let ctx = RoutingContext {
            task_prompt: String::new(),
            previous_failures: Some(3),
            conversation_turns: Some(10),
            plan_tasks: Some(5),
            remaining_tasks: Some(2),
            agent_chain_depth: Some(4),
            ..Default::default()
        };
        let signals = extract_context_signals(&ctx);
        assert_eq!(signals.previous_failures, 3);
        assert_eq!(signals.conversation_turns, 10);
        assert_eq!(signals.plan_complexity, 5);
        assert_eq!(signals.agent_chain_depth, 4);
    }

    #[test]
    fn context_signals_defaults_to_zero() {
        let ctx = RoutingContext::default();
        let signals = extract_context_signals(&ctx);
        assert_eq!(signals.previous_failures, 0);
        assert_eq!(signals.conversation_turns, 0);
    }

    #[test]
    fn subtask_estimation_with_bullets() {
        let prompt = "Do these things:\n- step one\n- step two\n- step three";
        let count = estimate_subtasks(prompt);
        assert!(
            count >= 4,
            "expected >= 4 (1 base + 3 bullets), got {}",
            count
        );
    }

    #[test]
    fn subtask_estimation_capped_at_10() {
        let bullets = (0..20)
            .map(|i| format!("- step {}", i))
            .collect::<Vec<_>>()
            .join("\n");
        let count = estimate_subtasks(&bullets);
        assert_eq!(count, 10);
    }

    #[test]
    fn implicit_requirements_refactor() {
        assert!(detect_implicit_requirements("refactor this module"));
    }

    #[test]
    fn implicit_requirements_none() {
        assert!(!detect_implicit_requirements("add two numbers"));
    }
}
