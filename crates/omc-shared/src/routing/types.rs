//! Model Routing Types
//!
//! Type definitions for the intelligent model routing system that routes
//! sub-agent tasks to appropriate models (Opus/Sonnet/Haiku) based on
//! task complexity.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Complexity tier for task routing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ComplexityTier {
    Low,
    Medium,
    High,
}

impl ComplexityTier {
    /// Numeric index for ordering (Low=0, Medium=1, High=2)
    pub fn index(self) -> usize {
        match self {
            Self::Low => 0,
            Self::Medium => 1,
            Self::High => 2,
        }
    }

    /// Tier from index
    pub fn from_index(i: usize) -> Self {
        match i {
            0 => Self::Low,
            1 => Self::Medium,
            _ => Self::High,
        }
    }
}

impl std::fmt::Display for ComplexityTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Low => write!(f, "LOW"),
            Self::Medium => write!(f, "MEDIUM"),
            Self::High => write!(f, "HIGH"),
        }
    }
}

/// Simple model type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelType {
    Haiku,
    Sonnet,
    Opus,
    Inherit,
}

impl std::fmt::Display for ModelType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Haiku => write!(f, "haiku"),
            Self::Sonnet => write!(f, "sonnet"),
            Self::Opus => write!(f, "opus"),
            Self::Inherit => write!(f, "inherit"),
        }
    }
}

/// Map tier to model type
pub fn tier_to_model_type(tier: ComplexityTier) -> ModelType {
    match tier {
        ComplexityTier::Low => ModelType::Haiku,
        ComplexityTier::Medium => ModelType::Sonnet,
        ComplexityTier::High => ModelType::Opus,
    }
}

/// Map model type string to tier
pub fn model_type_to_tier(model_type: &str) -> ComplexityTier {
    match model_type {
        "opus" => ComplexityTier::High,
        "haiku" => ComplexityTier::Low,
        _ => ComplexityTier::Medium,
    }
}

/// Default tier model IDs (configurable via env vars)
pub fn default_tier_models() -> HashMap<ComplexityTier, String> {
    let mut m = HashMap::new();
    m.insert(
        ComplexityTier::Low,
        std::env::var("OMC_MODEL_LOW").unwrap_or_else(|_| "claude-haiku-4-5-20251001".to_string()),
    );
    m.insert(
        ComplexityTier::Medium,
        std::env::var("OMC_MODEL_MEDIUM").unwrap_or_else(|_| "claude-sonnet-4-6".to_string()),
    );
    m.insert(
        ComplexityTier::High,
        std::env::var("OMC_MODEL_HIGH").unwrap_or_else(|_| "claude-opus-4-7".to_string()),
    );
    m
}

/// Lexical/syntactic signals extractable without model calls
#[derive(Debug, Clone, Default)]
pub struct LexicalSignals {
    /// Word count of the task prompt
    pub word_count: usize,
    /// Number of file paths mentioned
    pub file_path_count: usize,
    /// Number of code blocks in the prompt
    pub code_block_count: usize,
    /// Contains architecture-related keywords
    pub has_architecture_keywords: bool,
    /// Contains debugging-related keywords
    pub has_debugging_keywords: bool,
    /// Contains simple search keywords
    pub has_simple_keywords: bool,
    /// Contains risk/critical keywords
    pub has_risk_keywords: bool,
    /// Question depth
    pub question_depth: QuestionDepth,
    /// Has implicit requirements (vague statements without clear deliverables)
    pub has_implicit_requirements: bool,
}

/// Question depth hierarchy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QuestionDepth {
    Why,
    How,
    What,
    Where,
    #[default]
    None,
}

/// Structural signals that require parsing
#[derive(Debug, Clone, Default)]
pub struct StructuralSignals {
    /// Estimated number of subtasks
    pub estimated_subtasks: usize,
    /// Whether changes span multiple files
    pub cross_file_dependencies: bool,
    /// Whether tests are required
    pub has_test_requirements: bool,
    /// Domain specificity of the task
    pub domain_specificity: Domain,
    /// Whether external knowledge is needed
    pub requires_external_knowledge: bool,
    /// How reversible the changes are
    pub reversibility: Reversibility,
    /// Scope of impact
    pub impact_scope: ImpactScope,
}

/// Domain classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Domain {
    #[default]
    Generic,
    Frontend,
    Backend,
    Infrastructure,
    Security,
}

/// Reversibility classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Reversibility {
    #[default]
    Easy,
    Moderate,
    Difficult,
}

/// Impact scope classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImpactScope {
    #[default]
    Local,
    Module,
    SystemWide,
}

/// Context signals from session state
#[derive(Debug, Clone, Default)]
pub struct ContextSignals {
    /// Number of previous failures on this task
    pub previous_failures: usize,
    /// Number of conversation turns
    pub conversation_turns: usize,
    /// Complexity of the active plan (number of tasks)
    pub plan_complexity: usize,
    /// Number of remaining tasks in plan
    pub remaining_tasks: usize,
    /// Depth of agent delegation chain
    pub agent_chain_depth: usize,
}

/// Combined complexity signals
#[derive(Debug, Clone, Default)]
pub struct ComplexitySignals {
    pub lexical: LexicalSignals,
    pub structural: StructuralSignals,
    pub context: ContextSignals,
}

/// Routing decision result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingDecision {
    /// Selected model ID
    pub model: String,
    /// Selected model type
    pub model_type: ModelType,
    /// Complexity tier
    pub tier: ComplexityTier,
    /// Confidence score (0-1)
    pub confidence: f64,
    /// Reasons for the decision
    pub reasons: Vec<String>,
    /// Adapted prompt for the tier (optional)
    pub adapted_prompt: Option<String>,
    /// Whether escalation was triggered
    pub escalated: bool,
    /// Original tier before escalation (if escalated)
    pub original_tier: Option<ComplexityTier>,
}

/// Context for making routing decisions
#[derive(Debug, Clone, Default)]
pub struct RoutingContext {
    /// The task prompt to route
    pub task_prompt: String,
    /// Target agent type (if specified)
    pub agent_type: Option<String>,
    /// Parent session ID for context
    pub parent_session: Option<String>,
    /// Number of previous failures
    pub previous_failures: Option<usize>,
    /// Current conversation turn count
    pub conversation_turns: Option<usize>,
    /// Active plan tasks count
    pub plan_tasks: Option<usize>,
    /// Remaining plan tasks
    pub remaining_tasks: Option<usize>,
    /// Current agent chain depth
    pub agent_chain_depth: Option<usize>,
    /// Explicit model override (bypasses routing)
    pub explicit_model: Option<ModelType>,
}

/// Condition function type for routing rules.
pub type RuleCondition = Box<dyn Fn(&RoutingContext, &ComplexitySignals) -> bool>;

/// Routing rule definition
pub struct RoutingRule {
    /// Rule name for logging/debugging
    pub name: String,
    /// Condition function to check if rule applies
    pub condition: RuleCondition,
    /// Target tier if condition matches
    pub tier: Option<ComplexityTier>,
    /// Reason for the decision
    pub reason: String,
    /// Priority (higher = evaluated first)
    pub priority: i32,
}

impl std::fmt::Debug for RoutingRule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RoutingRule")
            .field("name", &self.name)
            .field("tier", &self.tier)
            .field("reason", &self.reason)
            .field("priority", &self.priority)
            .finish()
    }
}

/// Prompt adaptation strategy per tier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptAdaptationStrategy {
    Full,
    Balanced,
    Concise,
}

/// Map tier to prompt strategy
pub fn tier_prompt_strategy(tier: ComplexityTier) -> PromptAdaptationStrategy {
    match tier {
        ComplexityTier::High => PromptAdaptationStrategy::Full,
        ComplexityTier::Medium => PromptAdaptationStrategy::Balanced,
        ComplexityTier::Low => PromptAdaptationStrategy::Concise,
    }
}

/// Keywords for complexity detection
pub struct ComplexityKeywords;

impl ComplexityKeywords {
    pub const ARCHITECTURE: &'static [&'static str] = &[
        "architecture",
        "refactor",
        "redesign",
        "restructure",
        "reorganize",
        "decouple",
        "modularize",
        "abstract",
        "pattern",
        "design",
    ];

    pub const DEBUGGING: &'static [&'static str] = &[
        "debug",
        "diagnose",
        "root cause",
        "investigate",
        "trace",
        "analyze",
        "why is",
        "figure out",
        "understand why",
        "not working",
    ];

    pub const SIMPLE: &'static [&'static str] = &[
        "find", "search", "locate", "list", "show", "where is", "what is", "get", "fetch",
        "display", "print",
    ];

    pub const RISK: &'static [&'static str] = &[
        "critical",
        "production",
        "urgent",
        "security",
        "breaking",
        "dangerous",
        "irreversible",
        "data loss",
        "migration",
        "deploy",
    ];
}

/// Agent categories and their default complexity tiers
pub fn agent_category_tiers() -> HashMap<String, ComplexityTier> {
    let mut m = HashMap::new();
    m.insert("exploration".to_string(), ComplexityTier::Low);
    m.insert("utility".to_string(), ComplexityTier::Low);
    m.insert("specialist".to_string(), ComplexityTier::Medium);
    m.insert("orchestration".to_string(), ComplexityTier::Medium);
    m.insert("advisor".to_string(), ComplexityTier::High);
    m.insert("planner".to_string(), ComplexityTier::High);
    m.insert("reviewer".to_string(), ComplexityTier::High);
    m
}

/// Routing configuration
#[derive(Debug, Clone)]
pub struct RoutingConfig {
    /// Whether routing is enabled
    pub enabled: bool,
    /// Default tier when no rules match
    pub default_tier: ComplexityTier,
    /// Force all agents to inherit the parent model
    pub force_inherit: bool,
    /// Minimum tier to allow
    pub min_tier: Option<ComplexityTier>,
    /// Whether automatic escalation is enabled
    pub escalation_enabled: bool,
    /// Maximum escalation attempts
    pub max_escalations: usize,
    /// Model mapping per tier
    pub tier_models: HashMap<ComplexityTier, String>,
    /// Agent-specific overrides
    pub agent_overrides: HashMap<String, (ComplexityTier, String)>,
}

impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_tier: ComplexityTier::Medium,
            force_inherit: false,
            min_tier: None,
            escalation_enabled: false,
            max_escalations: 0,
            tier_models: default_tier_models(),
            agent_overrides: HashMap::new(),
        }
    }
}
