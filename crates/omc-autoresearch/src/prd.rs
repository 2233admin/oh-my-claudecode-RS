//! PRD (Product Requirements Document) management.
//!
//! This module is a skeleton for loading, parsing, and validating
//! mission.md / sandbox.md files that define an autoresearch mission.

use crate::types::{
    AutoresearchError, KeepPolicy, MissionContract, ParsedSandboxContract, Result,
    SETUP_CONFIDENCE_THRESHOLD, SandboxEvaluator, SetupHandoff,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Mission slug
// ---------------------------------------------------------------------------

/// Convert a directory name into a URL-safe mission slug.
pub fn slugify(name: &str) -> String {
    let slug: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let slug = slug
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let slug = slug.trim_matches('-');
    if slug.len() > 48 {
        slug[..48].to_string()
    } else if slug.is_empty() {
        "mission".to_string()
    } else {
        slug.to_string()
    }
}

// ---------------------------------------------------------------------------
// Sandbox parsing
// ---------------------------------------------------------------------------

/// Extract YAML frontmatter and body from a markdown file.
///
/// Expects the file to start with `---` delimiters.
fn extract_frontmatter(content: &str) -> Result<(String, String)> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return Err(AutoresearchError::Contract(
            "sandbox.md must start with YAML frontmatter".into(),
        ));
    }

    let after_first = &trimmed[3..];
    let end = after_first
        .find("---")
        .ok_or_else(|| AutoresearchError::Contract("unterminated YAML frontmatter".into()))?;

    let frontmatter = after_first[..end].trim().to_string();
    let body = after_first[end + 3..].trim().to_string();

    Ok((frontmatter, body))
}

/// Parse simple YAML frontmatter into a flat key-value map.
///
/// This is intentionally minimal -- handles `key: value` lines and
/// one level of nesting for the `evaluator:` block.
fn parse_yaml_frontmatter(raw: &str) -> Result<HashMap<String, serde_json::Value>> {
    let mut result = HashMap::new();
    let mut _current_section: Option<String> = None;

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Section header (e.g. `evaluator:`)
        if trimmed.ends_with(':') && !trimmed.contains(char::is_whitespace) && trimmed.len() > 1 {
            let key = trimmed.trim_end_matches(':').to_string();
            _current_section = Some(key.clone());
            result.insert(key, serde_json::Value::Object(serde_json::Map::new()));
            continue;
        }

        // key: value line
        if let Some((key, value)) = trimmed.split_once(':') {
            let key = key.trim().to_string();
            let value = value.trim().trim_matches(|c| c == '\'' || c == '"');
            result.insert(key, serde_json::Value::String(value.to_string()));
        }
    }

    Ok(result)
}

/// Parse a sandbox.md file content into a `ParsedSandboxContract`.
pub fn parse_sandbox_contract(content: &str) -> Result<ParsedSandboxContract> {
    let (frontmatter_raw, body) = extract_frontmatter(content)?;
    let frontmatter = parse_yaml_frontmatter(&frontmatter_raw)?;

    // Extract evaluator block -- for now, support flat keys
    // `evaluator_command` and `evaluator_format`, or nested via serde_yaml in the future.
    let evaluator_map: HashMap<String, String> = frontmatter
        .iter()
        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
        .collect();

    // Try to find evaluator fields from a nested object or flat keys
    let command = evaluator_map
        .get("command")
        .cloned()
        .or_else(|| evaluator_map.get("evaluator_command").cloned())
        .ok_or_else(|| {
            AutoresearchError::Contract("sandbox.md evaluator.command is required".into())
        })?;

    let format = evaluator_map
        .get("format")
        .cloned()
        .or_else(|| evaluator_map.get("evaluator_format").cloned())
        .unwrap_or_else(|| "json".to_string());

    if format != "json" {
        return Err(AutoresearchError::Contract(
            "sandbox.md evaluator.format must be json in autoresearch v1".into(),
        ));
    }

    let keep_policy_raw = evaluator_map
        .get("keep_policy")
        .or_else(|| evaluator_map.get("evaluator_keep_policy"));

    let keep_policy = match keep_policy_raw.map(|s| s.as_str()) {
        Some("pass_only") => Some(KeepPolicy::PassOnly),
        Some("score_improvement") => Some(KeepPolicy::ScoreImprovement),
        Some(other) => {
            return Err(AutoresearchError::Contract(format!(
                "invalid keep_policy: {other}"
            )));
        }
        None => None,
    };

    Ok(ParsedSandboxContract {
        frontmatter,
        evaluator: SandboxEvaluator {
            command,
            format: "json".to_string(),
            keep_policy,
        },
        body,
    })
}

// ---------------------------------------------------------------------------
// Mission contract loading
// ---------------------------------------------------------------------------

/// Load a mission contract from a directory containing mission.md and sandbox.md.
pub fn load_mission_contract(mission_dir: &Path) -> Result<MissionContract> {
    let mission_dir = mission_dir
        .canonicalize()
        .map_err(|e| AutoresearchError::Contract(format!("cannot resolve mission dir: {e}")))?;

    if !mission_dir.is_dir() {
        return Err(AutoresearchError::Contract(format!(
            "mission-dir does not exist: {}",
            mission_dir.display()
        )));
    }

    let mission_file = mission_dir.join("mission.md");
    let sandbox_file = mission_dir.join("sandbox.md");

    if !mission_file.exists() {
        return Err(AutoresearchError::Contract(format!(
            "mission.md is required: {}",
            mission_file.display()
        )));
    }
    if !sandbox_file.exists() {
        return Err(AutoresearchError::Contract(format!(
            "sandbox.md is required: {}",
            sandbox_file.display()
        )));
    }

    let mission_content = std::fs::read_to_string(&mission_file)?;
    let sandbox_content = std::fs::read_to_string(&sandbox_file)?;
    let sandbox = parse_sandbox_contract(&sandbox_content)?;

    // Resolve repo root via git
    let repo_root = resolve_git_root(&mission_dir)?;
    let mission_relative_dir = mission_dir
        .strip_prefix(&repo_root)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| {
            mission_dir
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        });

    let mission_slug = slugify(&mission_relative_dir);

    Ok(MissionContract {
        mission_dir,
        repo_root,
        mission_file,
        sandbox_file,
        mission_relative_dir,
        mission_content,
        sandbox_content,
        sandbox,
        mission_slug,
    })
}

/// Resolve the git repository root for a given path.
fn resolve_git_root(path: &Path) -> Result<PathBuf> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(path)
        .output()
        .map_err(|e| AutoresearchError::Runtime(format!("git not available: {e}")))?;

    if !output.status.success() {
        return Err(AutoresearchError::Contract(
            "mission-dir must be inside a git repository".into(),
        ));
    }

    let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(PathBuf::from(root))
}

// ---------------------------------------------------------------------------
// Setup handoff validation
// ---------------------------------------------------------------------------

/// Validate a `SetupHandoff` payload.
pub fn validate_setup_handoff(handoff: &SetupHandoff) -> Result<()> {
    if handoff.mission_text.trim().is_empty() {
        return Err(AutoresearchError::Contract(
            "setup handoff missionText is required".into(),
        ));
    }
    if handoff.evaluator_command.trim().is_empty() {
        return Err(AutoresearchError::Contract(
            "setup handoff evaluatorCommand is required".into(),
        ));
    }
    if !(0.0..=1.0).contains(&handoff.confidence) {
        return Err(AutoresearchError::Contract(
            "setup handoff confidence must be between 0 and 1".into(),
        ));
    }

    if handoff.evaluator_source == crate::types::EvaluatorSource::Inferred
        && handoff.confidence < SETUP_CONFIDENCE_THRESHOLD
        && handoff.ready_to_launch
    {
        return Err(AutoresearchError::Contract(
            "low-confidence inferred evaluators cannot be marked readyToLaunch".into(),
        ));
    }

    if !handoff.ready_to_launch && handoff.clarification_question.is_none() {
        return Err(AutoresearchError::Contract(
            "setup handoff must include clarificationQuestion when launch is blocked".into(),
        ));
    }

    // Verify the evaluator command parses as a valid sandbox contract
    let sandbox_content =
        build_setup_sandbox_content(&handoff.evaluator_command, handoff.keep_policy);
    parse_sandbox_contract(&sandbox_content)?;

    Ok(())
}

/// Build a minimal sandbox.md content from an evaluator command and optional keep policy.
pub fn build_setup_sandbox_content(
    evaluator_command: &str,
    keep_policy: Option<KeepPolicy>,
) -> String {
    let safe_cmd = evaluator_command.replace(['\r', '\n'], " ");
    let keep_policy_line = match keep_policy {
        Some(KeepPolicy::PassOnly) => "\n  keep_policy: pass_only",
        Some(KeepPolicy::ScoreImprovement) => "\n  keep_policy: score_improvement",
        None => "",
    };
    format!("---\nevaluator:\n  command: {safe_cmd}\n  format: json{keep_policy_line}\n---\n")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime;
    use crate::types::{CandidateArtifact, CandidateStatus, DecisionStatus};

    #[test]
    fn slugify_basic() {
        assert_eq!(slugify("My Mission"), "my-mission");
        assert_eq!(slugify("hello_world"), "hello-world");
        assert_eq!(slugify("---"), "mission");
    }

    #[test]
    fn slugify_truncates_at_48() {
        let long = "a".repeat(100);
        assert!(slugify(&long).len() <= 48);
    }

    #[test]
    fn parse_sandbox_valid() {
        let content = "---\nevaluator:\n  command: pytest\n  format: json\n---\nRun tests.\n";
        let parsed = parse_sandbox_contract(content).unwrap();
        assert_eq!(parsed.evaluator.command, "pytest");
        assert_eq!(parsed.evaluator.format, "json");
        assert_eq!(parsed.body, "Run tests.");
    }

    #[test]
    fn parse_sandbox_with_keep_policy() {
        let content =
            "---\nevaluator:\n  command: pytest\n  format: json\n  keep_policy: pass_only\n---\n";
        let parsed = parse_sandbox_contract(content).unwrap();
        assert_eq!(parsed.evaluator.keep_policy, Some(KeepPolicy::PassOnly));
    }

    #[test]
    fn parse_sandbox_missing_command() {
        let content = "---\nevaluator:\n  format: json\n---\n";
        assert!(parse_sandbox_contract(content).is_err());
    }

    #[test]
    fn parse_evaluator_result_valid() {
        let result = runtime::parse_evaluator_result(r#"{"pass": true, "score": 0.95}"#).unwrap();
        assert!(result.pass);
        assert_eq!(result.score, Some(0.95));
    }

    #[test]
    fn parse_evaluator_result_no_score() {
        let result = runtime::parse_evaluator_result(r#"{"pass": false}"#).unwrap();
        assert!(!result.pass);
        assert_eq!(result.score, None);
    }

    #[test]
    fn parse_evaluator_result_invalid() {
        assert!(runtime::parse_evaluator_result(r#"{"score": 1.0}"#).is_err());
        assert!(runtime::parse_evaluator_result("not json").is_err());
    }

    #[test]
    fn decide_abort() {
        use crate::runtime::decide_outcome;

        let candidate = CandidateArtifact {
            status: CandidateStatus::Abort,
            candidate_commit: None,
            base_commit: "abc".into(),
            description: "test".into(),
            notes: vec![],
            created_at: "2026-01-01T00:00:00Z".into(),
        };
        let decision = decide_outcome(KeepPolicy::default(), None, &candidate, None);
        assert_eq!(decision.decision, DecisionStatus::Abort);
        assert!(!decision.keep);
    }

    #[test]
    fn build_sandbox_content_roundtrip() {
        let content = build_setup_sandbox_content("pytest --tb=short", Some(KeepPolicy::PassOnly));
        let parsed = parse_sandbox_contract(&content).unwrap();
        assert_eq!(parsed.evaluator.command, "pytest --tb=short");
        assert_eq!(parsed.evaluator.keep_policy, Some(KeepPolicy::PassOnly));
    }
}
