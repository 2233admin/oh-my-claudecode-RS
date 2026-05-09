#![allow(clippy::too_many_arguments)]
//! Tiered Prompt Adaptations
//!
//! Provides model-specific prompt adaptations for Opus, Sonnet, and Haiku.
//! Each tier has prompts optimized for that model's capabilities.

use super::types::*;

// ============ Haiku Prompts ============

const HAIKU_PROMPT_PREFIX: &str = "TASK: ";
const HAIKU_PROMPT_SUFFIX: &str = "\n\nReturn results directly. No preamble.";

fn condense_prompt(prompt: &str) -> String {
    let replacements = [
        ("please ", ""),
        ("Please ", ""),
        ("could you ", ""),
        ("Could you ", ""),
        ("i would like you to ", ""),
        ("I would like you to ", ""),
        ("i need you to ", ""),
        ("I need you to ", ""),
        ("can you ", ""),
        ("Can you ", ""),
        ("would you ", ""),
        ("Would you ", ""),
        ("i want you to ", ""),
        ("I want you to ", ""),
        ("make sure to ", ""),
        ("Make sure to ", ""),
        ("be sure to ", ""),
        ("Be sure to ", ""),
        ("don't forget to ", ""),
        ("Don't forget to ", ""),
    ];
    let mut result = prompt.to_string();
    for (from, to) in &replacements {
        result = result.replace(from, to);
    }
    result.trim().to_string()
}

/// Adapt a base prompt for Haiku execution
pub fn adapt_prompt_for_haiku(base_prompt: &str) -> String {
    let condensed = condense_prompt(base_prompt);
    format!(
        "{}{}{}",
        HAIKU_PROMPT_PREFIX, condensed, HAIKU_PROMPT_SUFFIX
    )
}

// ============ Sonnet Prompts ============

const SONNET_PROMPT_PREFIX: &str =
    "## Task Execution Mode\n\nExecute this task efficiently with clear deliverables:\n\n";
const SONNET_PROMPT_SUFFIX: &str =
    "\n\n---\nFocus on delivering the requested outcome. Be thorough but efficient.";

/// Adapt a base prompt for Sonnet execution
pub fn adapt_prompt_for_sonnet(base_prompt: &str) -> String {
    format!(
        "{}{}{}",
        SONNET_PROMPT_PREFIX, base_prompt, SONNET_PROMPT_SUFFIX
    )
}

// ============ Opus Prompts ============

const OPUS_PROMPT_PREFIX: &str = "\
<thinking_mode>deep</thinking_mode>

You are operating at the highest capability tier. Apply sophisticated reasoning:

## Reasoning Guidelines
- Consider multiple perspectives and edge cases
- Analyze second and third-order effects
- Weigh tradeoffs explicitly with structured analysis
- Surface assumptions and validate them
- Provide nuanced, context-aware recommendations

## Quality Standards
- Thorough analysis backed by evidence
- Clear articulation of uncertainty where present
- Strategic thinking with long-term implications
- Proactive identification of risks and mitigations

";

const OPUS_PROMPT_SUFFIX: &str = "

## Before Concluding
- Have you considered edge cases?
- Are there second-order effects you haven't addressed?
- Have you validated your assumptions?
- Is your recommendation backed by the evidence gathered?
";

/// Adapt a base prompt for Opus execution
pub fn adapt_prompt_for_opus(base_prompt: &str) -> String {
    format!(
        "{}{}{}",
        OPUS_PROMPT_PREFIX, base_prompt, OPUS_PROMPT_SUFFIX
    )
}

// ============ Unified Interface ============

/// Adapt a prompt for a specific complexity tier
pub fn adapt_prompt_for_tier(prompt: &str, tier: ComplexityTier) -> String {
    match tier {
        ComplexityTier::High => adapt_prompt_for_opus(prompt),
        ComplexityTier::Medium => adapt_prompt_for_sonnet(prompt),
        ComplexityTier::Low => adapt_prompt_for_haiku(prompt),
    }
}

/// Get the prompt strategy for a tier
pub fn get_prompt_strategy(tier: ComplexityTier) -> PromptAdaptationStrategy {
    tier_prompt_strategy(tier)
}

/// Get prompt prefix for a tier
pub fn get_prompt_prefix(tier: ComplexityTier) -> &'static str {
    match tier {
        ComplexityTier::High => OPUS_PROMPT_PREFIX,
        ComplexityTier::Medium => SONNET_PROMPT_PREFIX,
        ComplexityTier::Low => HAIKU_PROMPT_PREFIX,
    }
}

/// Get prompt suffix for a tier
pub fn get_prompt_suffix(tier: ComplexityTier) -> &'static str {
    match tier {
        ComplexityTier::High => OPUS_PROMPT_SUFFIX,
        ComplexityTier::Medium => SONNET_PROMPT_SUFFIX,
        ComplexityTier::Low => HAIKU_PROMPT_SUFFIX,
    }
}

/// Create a delegation prompt with tier-appropriate framing
pub fn create_delegation_prompt(
    tier: ComplexityTier,
    task: &str,
    deliverables: Option<&str>,
    success_criteria: Option<&str>,
    context: Option<&str>,
    must_do: &[&str],
    must_not_do: &[&str],
    required_skills: &[&str],
    required_tools: &[&str],
) -> String {
    let prefix = get_prompt_prefix(tier);
    let suffix = get_prompt_suffix(tier);

    let mut body = format!("### Task\n{}\n", task);

    if let Some(d) = deliverables {
        body.push_str(&format!("\n### Deliverables\n{}\n", d));
    }
    if let Some(sc) = success_criteria {
        body.push_str(&format!("\n### Success Criteria\n{}\n", sc));
    }
    if let Some(c) = context {
        body.push_str(&format!("\n### Context\n{}\n", c));
    }
    if !must_do.is_empty() {
        body.push_str("\n### MUST DO\n");
        for item in must_do {
            body.push_str(&format!("- {}\n", item));
        }
    }
    if !must_not_do.is_empty() {
        body.push_str("\n### MUST NOT DO\n");
        for item in must_not_do {
            body.push_str(&format!("- {}\n", item));
        }
    }
    if !required_skills.is_empty() {
        body.push_str("\n### REQUIRED SKILLS\n");
        for item in required_skills {
            body.push_str(&format!("- {}\n", item));
        }
    }
    if !required_tools.is_empty() {
        body.push_str("\n### REQUIRED TOOLS\n");
        for item in required_tools {
            body.push_str(&format!("- {}\n", item));
        }
    }

    format!("{}{}{}", prefix, body, suffix)
}

/// Tier-specific instructions for common task types
pub fn get_task_instructions(tier: ComplexityTier, task_type: &str) -> &'static str {
    match tier {
        ComplexityTier::High => match task_type {
            "search" => "Perform thorough multi-angle search with analysis of findings.",
            "implement" => "Design solution with tradeoff analysis before implementing.",
            "debug" => "Deep root cause analysis with hypothesis testing.",
            "review" => "Comprehensive evaluation against multiple criteria.",
            "plan" => "Strategic planning with risk analysis and alternatives.",
            _ => "Design solution with tradeoff analysis before implementing.",
        },
        ComplexityTier::Medium => match task_type {
            "search" => "Search efficiently, return structured results.",
            "implement" => "Follow existing patterns, implement cleanly.",
            "debug" => "Systematic debugging, fix the issue.",
            "review" => "Check against criteria, provide feedback.",
            "plan" => "Create actionable plan with clear steps.",
            _ => "Follow existing patterns, implement cleanly.",
        },
        ComplexityTier::Low => match task_type {
            "search" => "Find and return paths.",
            "implement" => "Make the change.",
            "debug" => "Fix the bug.",
            "review" => "Check it.",
            "plan" => "List steps.",
            _ => "Make the change.",
        },
    }
}
