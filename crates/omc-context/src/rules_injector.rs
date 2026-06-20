//! Rules Injector
//!
//! Discovers and injects relevant rule files when files are accessed.
//! Supports project-level (.claude/rules, .github/instructions) and
//! user-level rules under ~/.claude.
//!
//! Ported from oh-my-claudecode's hooks/rules-injector.

use dashmap::DashMap;
use std::collections::HashSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
pub enum RulesInjectorError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Parsed rule metadata from YAML frontmatter.
#[derive(Debug, Clone, Default)]
pub struct RuleMetadata {
    pub description: Option<String>,
    pub globs: Vec<String>,
    pub always_apply: bool,
}

/// A rule to be injected into output.
#[derive(Debug, Clone)]
pub struct RuleToInject {
    pub relative_path: String,
    pub match_reason: String,
    pub content: String,
    pub distance: usize,
}

/// Rule file candidate found during discovery.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct RuleFileCandidate {
    path: PathBuf,
    real_path: PathBuf,
    is_global: bool,
    distance: usize,
    is_single_file: bool,
}

/// Per-session cache for tracking injected rules.
#[derive(Debug, Default)]
struct SessionCache {
    content_hashes: HashSet<String>,
    real_paths: HashSet<PathBuf>,
}

/// Tracked tool names that trigger rule injection.
const TRACKED_TOOLS: &[&str] = &[
    "read",
    "write",
    "edit",
    "multiedit",
    "glob",
    "grep",
    "notebookedit",
];

/// Discovers and injects relevant rule files.
#[derive(Clone)]
pub struct RulesInjector {
    working_directory: PathBuf,
    session_caches: Arc<DashMap<String, SessionCache>>,
}

impl RulesInjector {
    pub fn new(working_directory: impl Into<PathBuf>) -> Self {
        Self {
            working_directory: working_directory.into(),
            session_caches: Arc::new(DashMap::new()),
        }
    }

    /// Process a tool execution and return rules to inject.
    pub async fn process_tool_execution(
        &self,
        tool_name: &str,
        file_path: &str,
        session_id: &str,
    ) -> Vec<RuleToInject> {
        if !TRACKED_TOOLS.contains(&tool_name.to_lowercase().as_str()) {
            return Vec::new();
        }

        let resolved = self.resolve_file_path(file_path);
        let Some(resolved) = resolved else {
            return Vec::new();
        };

        let project_root = self.find_project_root(&resolved);
        let candidates = self.find_rule_files(&project_root, &resolved);
        let mut cache = self
            .session_caches
            .entry(session_id.to_string())
            .or_default();

        let mut to_inject = Vec::new();

        for candidate in candidates {
            if cache.real_paths.contains(&candidate.real_path) {
                continue;
            }

            let Ok(raw_content) = std::fs::read_to_string(&candidate.path) else {
                continue;
            };

            let (metadata, body) = parse_rule_frontmatter(&raw_content);

            let match_reason = if candidate.is_single_file {
                "copilot-instructions (always apply)".to_string()
            } else {
                match should_apply_rule(&metadata, &resolved, &project_root) {
                    Some(reason) => reason,
                    None => continue,
                }
            };

            let content_hash = content_hash(&body);
            if cache.content_hashes.contains(&content_hash) {
                continue;
            }

            let relative_path = project_root.as_ref().map_or_else(
                || candidate.path.to_string_lossy().to_string(),
                |root| {
                    pathdiff::diff_paths(&candidate.path, root).map_or_else(
                        || candidate.path.to_string_lossy().to_string(),
                        |p| p.to_string_lossy().to_string(),
                    )
                },
            );

            to_inject.push(RuleToInject {
                relative_path,
                match_reason,
                content: body,
                distance: candidate.distance,
            });

            cache.real_paths.insert(candidate.real_path);
            cache.content_hashes.insert(content_hash);
        }

        to_inject.sort_by_key(|a| a.distance);
        to_inject
    }

    /// Format rules for injection into output.
    pub fn format_rules_for_injection(rules: &[RuleToInject]) -> String {
        if rules.is_empty() {
            return String::default();
        }

        let mut output = String::default();
        for rule in rules {
            let _ = write!(
                output,
                "\n\n[Rule: {}]\n[Match: {}]\n{}",
                rule.relative_path, rule.match_reason, rule.content
            );
        }
        output
    }

    /// Check if a tool triggers rule injection.
    pub fn is_tracked_tool(tool_name: &str) -> bool {
        TRACKED_TOOLS.contains(&tool_name.to_lowercase().as_str())
    }

    /// Clear session cache when session ends.
    pub async fn clear_session(&self, session_id: &str) {
        self.session_caches.remove(session_id);
    }

    fn resolve_file_path(&self, file_path: &str) -> Option<PathBuf> {
        let path = Path::new(file_path);
        if path.is_absolute() {
            Some(path.to_path_buf())
        } else {
            Some(self.working_directory.join(path))
        }
    }

    fn find_project_root(&self, file_path: &Path) -> Option<PathBuf> {
        let mut dir = if file_path.is_dir() {
            file_path.to_path_buf()
        } else {
            file_path.parent()?.to_path_buf()
        };

        loop {
            if dir.join(".git").exists() || dir.join("CLAUDE.md").exists() {
                return Some(dir);
            }
            if !dir.pop() {
                return None;
            }
        }
    }

    fn find_rule_files(
        &self,
        project_root: &Option<PathBuf>,
        target: &Path,
    ) -> Vec<RuleFileCandidate> {
        let mut candidates = Vec::new();

        // Project-level rules
        if let Some(root) = project_root {
            let rules_dir = root.join(".claude").join("rules");
            Self::collect_rules_in_dir(&rules_dir, root, target, false, &mut candidates);

            // .github/instructions
            let github_dir = root.join(".github").join("instructions");
            Self::collect_rules_in_dir(&github_dir, root, target, false, &mut candidates);

            // .github/copilot-instructions.md (single file, always apply)
            let copilot_file = root.join(".github").join("copilot-instructions.md");
            if copilot_file.exists() {
                let real_path =
                    std::fs::canonicalize(&copilot_file).unwrap_or_else(|_| copilot_file.clone());
                candidates.push(RuleFileCandidate {
                    path: copilot_file,
                    real_path,
                    is_global: false,
                    distance: 0,
                    is_single_file: true,
                });
            }
        }

        // User-level rules
        if let Some(home) = dirs::home_dir() {
            let user_rules = home.join(".claude").join("rules");
            Self::collect_rules_in_dir(&user_rules, &home, target, true, &mut candidates);
        }

        candidates
    }

    fn collect_rules_in_dir(
        dir: &Path,
        root: &Path,
        target: &Path,
        is_global: bool,
        out: &mut Vec<RuleFileCandidate>,
    ) {
        if !dir.is_dir() {
            return;
        }

        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let ext = path.extension().and_then(|e| e.to_str());
            if !matches!(ext, Some("md" | "txt" | "yml" | "yaml")) {
                continue;
            }

            let real_path = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
            let distance = Self::dir_distance(&path, target, root);

            out.push(RuleFileCandidate {
                path,
                real_path,
                is_global,
                distance,
                is_single_file: false,
            });
        }
    }

    fn dir_distance(rule_path: &Path, target: &Path, root: &Path) -> usize {
        let rule_dir = rule_path.parent().unwrap_or(root);
        let target_dir = if target.is_dir() {
            target
        } else {
            target.parent().unwrap_or(root)
        };

        let rule_rel = rule_dir.strip_prefix(root).unwrap_or(rule_dir);
        let target_rel = target_dir.strip_prefix(root).unwrap_or(target_dir);

        let rule_parts: Vec<_> = rule_rel.components().collect();
        let target_parts: Vec<_> = target_rel.components().collect();

        let common = rule_parts
            .iter()
            .zip(target_parts.iter())
            .take_while(|(a, b)| a == b)
            .count();

        (rule_parts.len() - common) + (target_parts.len() - common)
    }
}

/// Parse rule frontmatter (YAML-like) and body.
fn parse_rule_frontmatter(content: &str) -> (RuleMetadata, String) {
    let content = content.trim_start();
    if !content.starts_with("---") {
        return (RuleMetadata::default(), content.to_string());
    }

    let after_open = &content[3..];
    let Some(end_idx) = after_open.find("\n---") else {
        return (RuleMetadata::default(), content.to_string());
    };

    let frontmatter = &after_open[..end_idx];
    let body = after_open[end_idx + 4..].trim();

    let mut metadata = RuleMetadata::default();

    for line in frontmatter.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("description:") {
            metadata.description = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("globs:") {
            metadata.globs = rest
                .trim()
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        } else if let Some(rest) = line.strip_prefix("alwaysApply:") {
            metadata.always_apply = rest.trim().to_lowercase() == "true";
        }
    }

    (metadata, body.to_string())
}

/// Check if a rule should apply to the target file.
fn should_apply_rule(
    metadata: &RuleMetadata,
    target: &Path,
    project_root: &Option<PathBuf>,
) -> Option<String> {
    if metadata.always_apply {
        return Some("alwaysApply".to_string());
    }

    if metadata.globs.is_empty() {
        return None;
    }

    let target_str = project_root
        .as_ref()
        .and_then(|root| {
            pathdiff::diff_paths(target, root).map(|p| p.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| target.to_string_lossy().to_string());

    let normalized = target_str.replace('\\', "/");

    for glob in &metadata.globs {
        let normalized_glob = glob.replace('\\', "/");
        if match_glob(&normalized_glob, &normalized) {
            return Some(format!("glob: {glob}"));
        }
    }

    None
}

/// Simple glob matching supporting * and **
fn match_glob(pattern: &str, text: &str) -> bool {
    glob_match::glob_match(pattern, text)
}

/// Simple content hash for deduplication.
fn content_hash(content: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::default();
    content.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Standalone utility: get rules for a file path.
pub async fn get_rules_for_path(
    file_path: &str,
    working_directory: Option<&str>,
) -> Vec<RuleToInject> {
    let cwd = working_directory.map_or_else(
        || std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        PathBuf::from,
    );

    let injector = RulesInjector::new(cwd);
    injector
        .process_tool_execution("read", file_path, "standalone")
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_frontmatter() {
        let content = "---\ndescription: Test rule\nglobs: *.ts, *.tsx\nalwaysApply: false\n---\nRule body here";
        let (metadata, body) = parse_rule_frontmatter(content);
        assert_eq!(metadata.description.as_deref(), Some("Test rule"));
        assert_eq!(metadata.globs, vec!["*.ts", "*.tsx"]);
        assert!(!metadata.always_apply);
        assert_eq!(body, "Rule body here");
    }

    #[test]
    fn test_parse_no_frontmatter() {
        let content = "Just plain content";
        let (metadata, body) = parse_rule_frontmatter(content);
        assert!(metadata.globs.is_empty());
        assert_eq!(body, "Just plain content");
    }

    #[test]
    fn test_format_rules() {
        let rules = vec![RuleToInject {
            relative_path: ".claude/rules/test.md".into(),
            match_reason: "glob: *.ts".into(),
            content: "Test rule".into(),
            distance: 0,
        }];

        let formatted = RulesInjector::format_rules_for_injection(&rules);
        assert!(formatted.contains("[Rule: .claude/rules/test.md]"));
        assert!(formatted.contains("[Match: glob: *.ts]"));
        assert!(formatted.contains("Test rule"));
    }

    #[test]
    fn test_is_tracked_tool() {
        assert!(RulesInjector::is_tracked_tool("Read"));
        assert!(RulesInjector::is_tracked_tool("edit"));
        assert!(!RulesInjector::is_tracked_tool("bash"));
    }

    #[test]
    fn test_match_glob() {
        assert!(match_glob("*.ts", "file.ts"));
        assert!(!match_glob("*.ts", "file.rs"));
        assert!(match_glob("**/*.ts", "src/deep/file.ts"));
    }
}
