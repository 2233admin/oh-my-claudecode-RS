use serde::{Deserialize, Serialize};

/// Keyword-based role router.
/// Maps keywords/phrases in user input to recommended agent roles.
pub struct RoleRouter {
    rules: Vec<RoleRule>,
}

struct RoleRule {
    keywords: Vec<String>,
    role: String,
    priority: u8,
}

impl RoleRouter {
    pub fn new() -> Self {
        Self {
            rules: Vec::default(),
        }
    }

    /// Add a routing rule.
    pub fn add_rule(&mut self, keywords: Vec<&str>, role: &str, priority: u8) {
        self.rules.push(RoleRule {
            keywords: keywords.into_iter().map(str::to_lowercase).collect(),
            role: role.to_string(),
            priority,
        });
    }

    /// Route user input to the best matching role.
    pub fn route(&self, input: &str) -> Option<&str> {
        let lower = input.to_lowercase();
        self.rules
            .iter()
            .filter(|rule| rule.keywords.iter().any(|kw| lower.contains(kw.as_str())))
            .max_by_key(|rule| rule.priority)
            .map(|rule| rule.role.as_str())
    }

    /// Get all matching roles sorted by priority (highest first).
    pub fn route_all(&self, input: &str) -> Vec<&str> {
        let lower = input.to_lowercase();
        let mut matches: Vec<(&str, u8)> = self
            .rules
            .iter()
            .filter(|rule| rule.keywords.iter().any(|kw| lower.contains(kw.as_str())))
            .map(|rule| (rule.role.as_str(), rule.priority))
            .collect();
        matches.sort_by_key(|b| std::cmp::Reverse(b.1));
        matches.dedup_by(|a, b| a.0 == b.0);
        matches.into_iter().map(|(role, _)| role).collect()
    }

    /// Create a router with default OMC rules.
    pub fn with_defaults() -> Self {
        let mut router = Self::new();
        router.add_rule(
            vec!["bug", "error", "broken", "failing", "crash"],
            "diagnose",
            10,
        );
        router.add_rule(vec!["test", "tdd", "red-green", "refactor"], "tdd", 8);
        router.add_rule(
            vec!["architecture", "refactor", "deepen", "module"],
            "improve-codebase-architecture",
            7,
        );
        router.add_rule(vec!["prototype", "mockup", "ui", "design"], "prototype", 6);
        router.add_rule(
            vec!["issue", "triage", "bug report", "feature request"],
            "triage",
            5,
        );
        router.add_rule(vec!["plan", "ralplan", "consensus"], "ralplan", 4);
        router
    }
}

impl Default for RoleRouter {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LaneIntent {
    Implementation,
    Verification,
    Review,
    Debug,
    Design,
    Docs,
    BuildFix,
    Cleanup,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleRouterResult {
    pub role: String,
    pub confidence: Confidence,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Confidence {
    High,
    Medium,
    Low,
}

type RoleKeywords = (&'static str, &'static [&'static str]);

const ROLE_KEYWORDS: &[RoleKeywords] = &[
    (
        "omc-executor",
        &[
            "implement",
            "build",
            "create",
            "add",
            "write",
            "develop",
            "feature",
            "code",
            "port",
            "migrate",
            "integrate",
            "refactor",
        ],
    ),
    (
        "omc-reviewer",
        &[
            "review", "audit", "check", "validate", "assess", "evaluate", "inspect", "verify",
            "quality", "feedback",
        ],
    ),
    (
        "omc-security-auditor",
        &[
            "security",
            "vulnerability",
            "auth",
            "permission",
            "encrypt",
            "secret",
            "token",
            "injection",
            "xss",
            "csrf",
            "sanitiz",
            "unsafe",
        ],
    ),
    (
        "omc-planner",
        &[
            "plan",
            "design",
            "architect",
            "spec",
            "rfc",
            "proposal",
            "strategy",
            "breakdown",
            "decompose",
            "split",
        ],
    ),
    (
        "debugger",
        &[
            "debug",
            "fix",
            "bug",
            "error",
            "crash",
            "fail",
            "broken",
            "regression",
            "panic",
            "trace",
            "diagnos",
        ],
    ),
    (
        "documenter",
        &[
            "document",
            "doc",
            "readme",
            "changelog",
            "comment",
            "javadoc",
            "docstring",
            "guide",
            "tutorial",
            "explain",
        ],
    ),
    (
        "test-writer",
        &[
            "test",
            "spec",
            "coverage",
            "unit test",
            "integration test",
            "e2e",
            "assertion",
            "mock",
            "fixture",
            "benchmark",
        ],
    ),
    (
        "devops",
        &[
            "ci",
            "cd",
            "deploy",
            "docker",
            "kubernetes",
            "pipeline",
            "build",
            "release",
            "infra",
            "terraform",
            "workflow",
            "github action",
        ],
    ),
];

/// Route a task to a role based on keyword matching.
pub fn route_task_to_role(subject: &str, description: &str, fallback: &str) -> RoleRouterResult {
    let combined = format!("{subject} {description}").to_lowercase();

    let mut best_score: u32 = 0;
    let mut best_role = fallback;
    let mut matched_keyword = "";

    for &(role, patterns) in ROLE_KEYWORDS {
        let mut score: u32 = 0;
        let mut matched = "";
        for &pattern in patterns {
            if combined.contains(pattern) {
                score += 1;
                matched = pattern;
            }
        }
        if score > best_score {
            best_score = score;
            best_role = role;
            matched_keyword = matched;
        }
    }

    if best_score == 0 {
        RoleRouterResult {
            role: fallback.to_string(),
            confidence: Confidence::Low,
            reason: "No keyword matches found; using fallback role".to_string(),
        }
    } else {
        let confidence = match best_score {
            1 => Confidence::Low,
            2 => Confidence::Medium,
            _ => Confidence::High,
        };
        RoleRouterResult {
            role: best_role.to_string(),
            confidence,
            reason: format!(
                "Matched keyword '{}' ({} total matches)",
                matched_keyword, best_score
            ),
        }
    }
}

/// Infer lane intent from task text.
pub fn infer_lane_intent(text: &str) -> LaneIntent {
    let lower = text.to_lowercase();

    if contains_any(
        &lower,
        &["test", "spec", "coverage", "e2e", "assertion", "mock"],
    ) {
        return LaneIntent::Verification;
    }
    if contains_any(
        &lower,
        &["review", "audit", "check", "assess", "evaluate", "inspect"],
    ) {
        return LaneIntent::Review;
    }
    if contains_any(
        &lower,
        &["debug", "fix bug", "crash", "error", "regression", "trace"],
    ) {
        return LaneIntent::Debug;
    }
    if contains_any(
        &lower,
        &["design", "architect", "spec", "rfc", "proposal", "plan"],
    ) {
        return LaneIntent::Design;
    }
    if contains_any(
        &lower,
        &["doc", "readme", "changelog", "guide", "tutorial", "comment"],
    ) {
        return LaneIntent::Docs;
    }
    if contains_any(
        &lower,
        &[
            "ci",
            "build fix",
            "pipeline",
            "deploy",
            "release",
            "workflow",
        ],
    ) {
        return LaneIntent::BuildFix;
    }
    if contains_any(
        &lower,
        &["cleanup", "refactor", "dead code", "unused", "tidy", "lint"],
    ) {
        return LaneIntent::Cleanup;
    }
    if contains_any(
        &lower,
        &[
            "implement",
            "build",
            "create",
            "add",
            "feature",
            "port",
            "migrate",
            "develop",
            "write",
        ],
    ) {
        return LaneIntent::Implementation;
    }

    LaneIntent::Unknown
}

fn contains_any(text: &str, keywords: &[&str]) -> bool {
    keywords.iter().any(|kw| text.contains(kw))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_implementation_task() {
        let result =
            route_task_to_role("Implement auth module", "Build the login flow", "fallback");
        assert_eq!(result.role, "omc-executor");
        assert_eq!(result.confidence, Confidence::Medium);
    }

    #[test]
    fn routes_security_task() {
        let result = route_task_to_role(
            "Fix XSS vulnerability in user input",
            "Sanitize and escape all user-provided content",
            "fallback",
        );
        assert_eq!(result.role, "omc-security-auditor");
        assert!(result.confidence == Confidence::Medium || result.confidence == Confidence::High);
    }

    #[test]
    fn routes_debug_task() {
        let result = route_task_to_role(
            "Fix crash in parser",
            "Debug the error that occurs on empty input",
            "fallback",
        );
        assert_eq!(result.role, "debugger");
        assert!(result.confidence == Confidence::Medium || result.confidence == Confidence::High);
    }

    #[test]
    fn routes_review_task() {
        let result = route_task_to_role(
            "Review PR #42",
            "Validate the changes and check quality",
            "fallback",
        );
        assert_eq!(result.role, "omc-reviewer");
        assert_eq!(result.confidence, Confidence::High);
    }

    #[test]
    fn falls_back_when_no_match() {
        let result = route_task_to_role("Random stuff", "Nothing relevant here", "default-role");
        assert_eq!(result.role, "default-role");
        assert_eq!(result.confidence, Confidence::Low);
    }

    #[test]
    fn routes_test_task() {
        let result = route_task_to_role(
            "Add unit tests for service layer",
            "Write test coverage for edge cases",
            "fallback",
        );
        assert_eq!(result.role, "test-writer");
    }

    #[test]
    fn infer_lane_implementation() {
        assert_eq!(
            infer_lane_intent("Implement the new auth feature"),
            LaneIntent::Implementation
        );
    }

    #[test]
    fn infer_lane_verification() {
        assert_eq!(
            infer_lane_intent("Write tests for the parser"),
            LaneIntent::Verification
        );
    }

    #[test]
    fn infer_lane_review() {
        assert_eq!(
            infer_lane_intent("Review the PR for correctness"),
            LaneIntent::Review
        );
    }

    #[test]
    fn infer_lane_debug() {
        assert_eq!(
            infer_lane_intent("Fix crash in the scheduler"),
            LaneIntent::Debug
        );
    }

    #[test]
    fn infer_lane_design() {
        assert_eq!(
            infer_lane_intent("Design the new API architecture"),
            LaneIntent::Design
        );
    }

    #[test]
    fn infer_lane_docs() {
        assert_eq!(
            infer_lane_intent("Write README and update the guide"),
            LaneIntent::Docs
        );
    }

    #[test]
    fn infer_lane_cleanup() {
        assert_eq!(
            infer_lane_intent("Refactor and remove dead code"),
            LaneIntent::Cleanup
        );
    }

    #[test]
    fn infer_lane_unknown() {
        assert_eq!(infer_lane_intent("Something random"), LaneIntent::Unknown);
    }

    #[test]
    fn role_router_keyword_matching() {
        let router = RoleRouter::with_defaults();
        assert_eq!(
            router.route("there is a bug in the parser"),
            Some("diagnose")
        );
        assert_eq!(router.route("write more tests"), Some("tdd"));
        assert_eq!(
            router.route("improve architecture of this module"),
            Some("improve-codebase-architecture")
        );
        assert_eq!(router.route("build a quick prototype"), Some("prototype"));
        assert_eq!(router.route("file an issue"), Some("triage"));
        assert_eq!(router.route("make a plan"), Some("ralplan"));
    }

    #[test]
    fn role_router_priority_ordering() {
        let mut router = RoleRouter::new();
        router.add_rule(vec!["fix"], "low-priority", 2);
        router.add_rule(vec!["fix", "crash"], "high-priority", 10);
        // "crash" matches both, but high-priority wins
        assert_eq!(router.route("fix the crash"), Some("high-priority"));
    }

    #[test]
    fn role_router_no_match_returns_none() {
        let router = RoleRouter::new();
        assert_eq!(router.route("hello world"), None);
    }

    #[test]
    fn role_router_multi_keyword() {
        let mut router = RoleRouter::new();
        router.add_rule(vec!["alpha", "beta"], "role-a", 5);
        router.add_rule(vec!["gamma"], "role-b", 3);
        let all = router.route_all("alpha and gamma");
        assert_eq!(all, vec!["role-a", "role-b"]);
    }

    #[test]
    fn role_router_case_insensitive() {
        let router = RoleRouter::with_defaults();
        assert_eq!(router.route("THERE IS A BUG"), Some("diagnose"));
        assert_eq!(router.route("Write More Tests"), Some("tdd"));
    }

    #[test]
    fn role_router_default_impl() {
        let router = RoleRouter::default();
        assert!(router.route("something random").is_none());
        assert_eq!(router.route("bug"), Some("diagnose"));
    }
}
