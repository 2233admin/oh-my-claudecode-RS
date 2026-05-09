//! Wiki Types and Operations
//!
//! Core types for the LLM Wiki knowledge layer and the three main operations:
//! ingest, query, and lint.
//!
//! Inspired by Karpathy's LLM Wiki concept -- persistent, self-maintained
//! markdown knowledge base that compounds over time.

use serde::{Deserialize, Serialize};

// ============================================================================
// Constants
// ============================================================================

/// Current schema version for wiki pages.
pub const WIKI_SCHEMA_VERSION: u32 = 1;

/// Supported wiki categories.
pub const WIKI_CATEGORIES: &[&str] = &[
    "architecture",
    "decision",
    "pattern",
    "debugging",
    "environment",
    "session-log",
    "reference",
    "convention",
];

// ============================================================================
// Page Schema
// ============================================================================

/// YAML frontmatter for a wiki page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiPageFrontmatter {
    /// Page title (human-readable).
    pub title: String,
    /// Searchable tags.
    pub tags: Vec<String>,
    /// ISO timestamp of page creation.
    pub created: String,
    /// ISO timestamp of last update.
    pub updated: String,
    /// Session IDs or sources that contributed to this page.
    pub sources: Vec<String>,
    /// Filenames of linked pages (cross-references).
    pub links: Vec<String>,
    /// Page category.
    pub category: String,
    /// Confidence level of the knowledge (high / medium / low).
    pub confidence: String,
    /// Schema version for future migration support.
    pub schema_version: u32,
}

/// A wiki page: frontmatter + markdown content + filename.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiPage {
    /// Filename without path (e.g., "auth-architecture.md").
    pub filename: String,
    /// Parsed YAML frontmatter.
    pub frontmatter: WikiPageFrontmatter,
    /// Markdown content (everything after the frontmatter).
    pub content: String,
}

// ============================================================================
// Operations
// ============================================================================

/// Log entry for wiki operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiLogEntry {
    /// ISO timestamp.
    pub timestamp: String,
    /// Type of operation.
    pub operation: String,
    /// Filenames of pages affected.
    pub pages_affected: Vec<String>,
    /// Human-readable summary.
    pub summary: String,
}

/// Input for the ingest operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiIngestInput {
    pub title: String,
    pub content: String,
    pub tags: Vec<String>,
    pub category: String,
    pub sources: Vec<String>,
    pub confidence: Option<String>,
}

/// Result of an ingest operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiIngestResult {
    /// Pages that were created.
    pub created: Vec<String>,
    /// Pages that were updated (merged).
    pub updated: Vec<String>,
    /// Total pages affected.
    pub total_affected: usize,
}

/// Options for wiki query.
#[derive(Debug, Clone, Default)]
pub struct WikiQueryOptions {
    /// Filter by tags (OR match).
    pub tags: Option<Vec<String>>,
    /// Filter by category.
    pub category: Option<String>,
    /// Maximum results to return.
    pub limit: Option<usize>,
}

/// A single query match.
#[derive(Debug, Clone)]
pub struct WikiQueryMatch {
    /// The matched page.
    pub page: WikiPage,
    /// Relevance snippet.
    pub snippet: String,
    /// Match score (higher = more relevant).
    pub score: i64,
}

// ============================================================================
// Lint
// ============================================================================

/// Wiki lint severity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WikiLintSeverity {
    Error,
    Warning,
    Info,
}

/// Types of lint issues.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WikiLintIssueType {
    Orphan,
    Stale,
    BrokenRef,
    LowConfidence,
    Oversized,
    StructuralContradiction,
}

/// A single lint issue.
#[derive(Debug, Clone)]
pub struct WikiLintIssue {
    pub page: String,
    pub severity: WikiLintSeverity,
    pub issue_type: WikiLintIssueType,
    pub message: String,
}

/// Summary statistics for a lint report.
#[derive(Debug, Clone, Default)]
pub struct WikiLintStats {
    pub total_pages: usize,
    pub orphan_count: usize,
    pub stale_count: usize,
    pub broken_ref_count: usize,
    pub low_confidence_count: usize,
    pub oversized_count: usize,
    pub contradiction_count: usize,
}

/// Full lint report.
#[derive(Debug, Clone)]
pub struct WikiLintReport {
    pub issues: Vec<WikiLintIssue>,
    pub stats: WikiLintStats,
}

// ============================================================================
// Wiki Configuration
// ============================================================================

/// Wiki configuration.
#[derive(Debug, Clone)]
pub struct WikiConfig {
    /// Whether auto-capture is enabled at session end.
    pub auto_capture: bool,
    /// Days after which a page is considered stale.
    pub stale_days: u32,
    /// Maximum page content size in bytes before lint warns.
    pub max_page_size: usize,
}

impl Default for WikiConfig {
    fn default() -> Self {
        Self {
            auto_capture: true,
            stale_days: 30,
            max_page_size: 10_240, // 10KB
        }
    }
}

// ============================================================================
// Operations (stubs — implement in future iterations)
// ============================================================================

/// Ingest knowledge into the wiki.
///
/// If a page with the same slug exists, merges content (append strategy).
pub fn ingest_knowledge(root: &std::path::Path, input: &WikiIngestInput) -> WikiIngestResult {
    use crate::storage;

    let slug = storage::title_to_slug(&input.title);
    let now = chrono::Utc::now().to_rfc3339();
    let mut result = WikiIngestResult {
        created: vec![],
        updated: vec![],
        total_affected: 0,
    };

    let existing = storage::read_page(root, &slug).ok().flatten();

    if let Some(mut page) = existing {
        // Merge into existing page (append strategy)
        let merged_tags: Vec<String> = {
            let mut tags: Vec<String> = page.frontmatter.tags.to_vec();
            for t in &input.tags {
                if !tags.contains(t) {
                    tags.push(t.clone());
                }
            }
            tags
        };
        let mut merged_sources: Vec<String> = page.frontmatter.sources.to_vec();
        for s in &input.sources {
            if !merged_sources.contains(s) {
                merged_sources.push(s.clone());
            }
        }
        page.frontmatter.tags = merged_tags;
        page.frontmatter.sources = merged_sources;
        page.frontmatter.updated.clone_from(&now);

        let default_conf = "medium".to_string();
        let new_conf = input.confidence.as_ref().unwrap_or(&default_conf);
        let rank = |c: &str| match c {
            "high" => 3,
            "medium" => 2,
            "low" => 1,
            _ => 2,
        };
        if rank(new_conf) >= rank(&page.frontmatter.confidence) {
            page.frontmatter.confidence.clone_from(new_conf);
        }

        page.content = format!(
            "{}\n\n---\n\n## Update ({})\n\n{}",
            page.content.trim_end(),
            now,
            input.content
        );

        let _ = storage::write_page(root, &page);
        result.updated.push(slug);
    } else {
        // Create new page
        let frontmatter = WikiPageFrontmatter {
            title: input.title.clone(),
            tags: {
                let mut t = input.tags.clone();
                t.dedup();
                t
            },
            created: now.clone(),
            updated: now.clone(),
            sources: input.sources.clone(),
            links: extract_wiki_links(&input.content),
            category: input.category.clone(),
            confidence: input
                .confidence
                .clone()
                .unwrap_or_else(|| "medium".to_string()),
            schema_version: WIKI_SCHEMA_VERSION,
        };
        let page = WikiPage {
            filename: slug.clone(),
            frontmatter,
            content: format!("\n# {}\n\n{}", input.title, input.content),
        };
        let _ = storage::write_page(root, &page);
        result.created.push(slug);
    }

    result.total_affected = result.created.len() + result.updated.len();

    let _ = storage::append_log(
        root,
        &WikiLogEntry {
            timestamp: now,
            operation: "ingest".to_string(),
            pages_affected: result
                .created
                .iter()
                .chain(result.updated.iter())
                .cloned()
                .collect(),
            summary: if result.updated.is_empty() {
                format!("Created new page \"{}\"", input.title)
            } else {
                format!("Updated \"{}\" with new content", input.title)
            },
        },
    );

    result
}

/// Search wiki pages by keyword and/or tags.
pub fn query_wiki(
    root: &std::path::Path,
    query_text: &str,
    options: &WikiQueryOptions,
) -> Vec<WikiQueryMatch> {
    use crate::storage;

    let pages = match storage::read_all_pages(root) {
        Ok(p) => p,
        Err(_) => return vec![],
    };
    let limit = options.limit.unwrap_or(20);
    let query_lower = query_text.to_lowercase();
    let query_terms = tokenize(query_text);

    let mut matches: Vec<WikiQueryMatch> = Vec::default();

    for page in &pages {
        // Category filter
        if let Some(ref cat) = options.category
            && page.frontmatter.category != *cat
        {
            continue;
        }

        let mut score: i64 = 0;
        let mut snippet = String::default();

        // Tag matching (weight: 3 per matching tag)
        if let Some(ref filter_tags) = options.tags {
            let overlap = filter_tags
                .iter()
                .filter(|t| {
                    page.frontmatter
                        .tags
                        .iter()
                        .any(|pt| pt.eq_ignore_ascii_case(t))
                })
                .count();
            score += overlap as i64 * 3;
        }

        // Match query terms against page tags
        for term in &query_terms {
            if page
                .frontmatter
                .tags
                .iter()
                .any(|t| t.to_lowercase().contains(term.as_str()))
            {
                score += 2;
            }
        }

        // Title matching (weight: 5)
        let title_lower = page.frontmatter.title.to_lowercase();
        if title_lower.contains(&query_lower) {
            score += 5;
        } else {
            for term in &query_terms {
                if title_lower.contains(term.as_str()) {
                    score += 2;
                }
            }
        }

        // Content matching (weight: 1 per unique term)
        let content_lower = page.content.to_lowercase();
        for term in &query_terms {
            if let Some(idx) = content_lower.find(term.as_str()) {
                score += 1;
                if snippet.is_empty() {
                    let start = idx.saturating_sub(40);
                    let end = std::cmp::min(content_lower.len(), idx + term.len() + 80);
                    let raw = page.content[start..end].replace('\n', " ");
                    snippet = format!(
                        "{}{}{}",
                        if start > 0 { "..." } else { "" },
                        raw,
                        if end < content_lower.len() { "..." } else { "" }
                    );
                }
            }
        }

        if score > 0 {
            if snippet.is_empty() {
                snippet = page
                    .content
                    .lines()
                    .find(|l| !l.trim().is_empty())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if snippet.len() > 120 {
                    snippet.truncate(117);
                    snippet.push_str("...");
                }
            }
            matches.push(WikiQueryMatch {
                page: page.clone(),
                snippet,
                score,
            });
        }
    }

    matches.sort_by_key(|m| std::cmp::Reverse(m.score));
    matches.truncate(limit);

    let _ = storage::append_log(
        root,
        &WikiLogEntry {
            timestamp: chrono::Utc::now().to_rfc3339(),
            operation: "query".to_string(),
            pages_affected: matches.iter().map(|m| m.page.filename.clone()).collect(),
            summary: format!("Query \"{}\" -> {} results", query_text, matches.len()),
        },
    );

    matches
}

/// Run health checks on the wiki.
pub fn lint_wiki(root: &std::path::Path) -> WikiLintReport {
    use crate::storage;

    let config = WikiConfig::default();
    let pages = match storage::read_all_pages(root) {
        Ok(p) => p,
        Err(_) => {
            return WikiLintReport {
                issues: vec![],
                stats: WikiLintStats::default(),
            };
        }
    };

    let mut issues: Vec<WikiLintIssue> = Vec::new();
    let page_filenames: std::collections::HashSet<String> =
        pages.iter().map(|p| p.filename.clone()).collect();

    // Build incoming link map
    let mut incoming_links: std::collections::HashMap<String, std::collections::HashSet<String>> =
        std::collections::HashMap::new();
    for page in &pages {
        for link in &page.frontmatter.links {
            incoming_links
                .entry(link.clone())
                .or_default()
                .insert(page.filename.clone());
        }
    }

    let now = chrono::Utc::now().timestamp_millis();
    let stale_threshold_ms = config.stale_days as i64 * 24 * 60 * 60 * 1000;

    for page in &pages {
        // 1. Orphan detection
        let incoming = incoming_links.get(&page.filename);
        if incoming.is_none_or(|s| s.is_empty()) {
            issues.push(WikiLintIssue {
                page: page.filename.clone(),
                severity: WikiLintSeverity::Info,
                issue_type: WikiLintIssueType::Orphan,
                message: format!("No other pages link to \"{}\"", page.frontmatter.title),
            });
        }

        // 2. Stale detection
        if let Ok(updated_at) = chrono::DateTime::parse_from_rfc3339(&page.frontmatter.updated) {
            let updated_ms = updated_at.timestamp_millis();
            let diff = now - updated_ms;
            if diff > stale_threshold_ms {
                let days_since = diff / (24 * 60 * 60 * 1000);
                issues.push(WikiLintIssue {
                    page: page.filename.clone(),
                    severity: WikiLintSeverity::Warning,
                    issue_type: WikiLintIssueType::Stale,
                    message: format!(
                        "\"{}\" not updated in {} days",
                        page.frontmatter.title, days_since
                    ),
                });
            }
        }

        // 3. Broken cross-references
        for link in &page.frontmatter.links {
            if !page_filenames.contains(link) {
                issues.push(WikiLintIssue {
                    page: page.filename.clone(),
                    severity: WikiLintSeverity::Error,
                    issue_type: WikiLintIssueType::BrokenRef,
                    message: format!(
                        "Broken link to \"{}\" from \"{}\"",
                        link, page.frontmatter.title
                    ),
                });
            }
        }

        // 4. Low confidence
        if page.frontmatter.confidence == "low" {
            issues.push(WikiLintIssue {
                page: page.filename.clone(),
                severity: WikiLintSeverity::Info,
                issue_type: WikiLintIssueType::LowConfidence,
                message: format!(
                    "\"{}\" has low confidence -- consider verifying or removing",
                    page.frontmatter.title
                ),
            });
        }

        // 5. Oversized pages
        let content_size = page.content.len();
        if content_size > config.max_page_size {
            let size_kb = (content_size as f64) / 1024.0;
            issues.push(WikiLintIssue {
                page: page.filename.clone(),
                severity: WikiLintSeverity::Warning,
                issue_type: WikiLintIssueType::Oversized,
                message: format!(
                    "\"{}\" is {:.1}KB -- consider splitting into smaller pages",
                    page.frontmatter.title, size_kb
                ),
            });
        }
    }

    // 6. Structural contradictions
    detect_structural_contradictions(&pages, &mut issues);

    let stats = WikiLintStats {
        total_pages: pages.len(),
        orphan_count: issues
            .iter()
            .filter(|i| i.issue_type == WikiLintIssueType::Orphan)
            .count(),
        stale_count: issues
            .iter()
            .filter(|i| i.issue_type == WikiLintIssueType::Stale)
            .count(),
        broken_ref_count: issues
            .iter()
            .filter(|i| i.issue_type == WikiLintIssueType::BrokenRef)
            .count(),
        low_confidence_count: issues
            .iter()
            .filter(|i| i.issue_type == WikiLintIssueType::LowConfidence)
            .count(),
        oversized_count: issues
            .iter()
            .filter(|i| i.issue_type == WikiLintIssueType::Oversized)
            .count(),
        contradiction_count: issues
            .iter()
            .filter(|i| i.issue_type == WikiLintIssueType::StructuralContradiction)
            .count(),
    };

    let _ = storage::append_log(
        root,
        &WikiLogEntry {
            timestamp: chrono::Utc::now().to_rfc3339(),
            operation: "lint".to_string(),
            pages_affected: issues.iter().map(|i| i.page.clone()).collect(),
            summary: format!(
                "Lint: {} issues ({} orphan, {} stale, {} broken, {} contradictions)",
                issues.len(),
                stats.orphan_count,
                stats.stale_count,
                stats.broken_ref_count,
                stats.contradiction_count,
            ),
        },
    );

    WikiLintReport { issues, stats }
}

/// Detect structural contradictions: overlapping tags with different categories,
/// same slug prefix with conflicting confidence.
fn detect_structural_contradictions(pages: &[WikiPage], issues: &mut Vec<WikiLintIssue>) {
    let mut slug_groups: std::collections::HashMap<String, Vec<&WikiPage>> =
        std::collections::HashMap::new();
    for page in pages {
        let parts: Vec<&str> = page.filename.split('-').collect();
        let prefix = if parts.len() >= 2 {
            format!("{}-{}", parts[0], parts[1])
        } else {
            page.filename.clone()
        };
        slug_groups.entry(prefix).or_default().push(page);
    }

    for group in slug_groups.values() {
        if group.len() < 2 {
            continue;
        }

        // Check for conflicting confidence on same topic
        let mut confidences: std::collections::HashSet<&str> = group
            .iter()
            .map(|p| p.frontmatter.confidence.as_str())
            .collect();
        if confidences.len() > 1
            && confidences.contains("high")
            && confidences.take("low").is_some()
        {
            let titles: Vec<String> = group
                .iter()
                .map(|p| format!("\"{}\"", p.frontmatter.title))
                .collect();
            issues.push(WikiLintIssue {
                page: group[0].filename.clone(),
                severity: WikiLintSeverity::Warning,
                issue_type: WikiLintIssueType::StructuralContradiction,
                message: format!(
                    "Conflicting confidence levels for related pages: {}",
                    titles.join(", ")
                ),
            });
        }

        // Check for overlapping tags with different categories
        let mut tag_categories: std::collections::HashMap<&str, std::collections::HashSet<&str>> =
            std::collections::HashMap::new();
        for page in group {
            for tag in &page.frontmatter.tags {
                tag_categories
                    .entry(tag.as_str())
                    .or_default()
                    .insert(&page.frontmatter.category);
            }
        }

        for (tag, categories) in &tag_categories {
            if categories.len() > 1 {
                issues.push(WikiLintIssue {
                    page: group[0].filename.clone(),
                    severity: WikiLintSeverity::Info,
                    issue_type: WikiLintIssueType::StructuralContradiction,
                    message: format!(
                        "Tag \"{}\" appears in pages with different categories: {}",
                        tag,
                        categories.iter().cloned().collect::<Vec<_>>().join(", ")
                    ),
                });
                break; // One contradiction per group is enough
            }
        }
    }
}

/// Extract [[wiki-link]] references from content.
fn extract_wiki_links(content: &str) -> Vec<String> {
    let mut links = Vec::new();
    let mut remaining = content;
    while let Some(start) = remaining.find("[[") {
        if let Some(end) = remaining[start + 2..].find("]]") {
            let name = remaining[start + 2..start + 2 + end].trim();
            let slug = crate::storage::title_to_slug(name);
            if !links.contains(&slug) {
                links.push(slug);
            }
            remaining = &remaining[start + 2 + end + 2..];
        } else {
            break;
        }
    }
    links
}

/// Tokenize text for search, with CJK bi-gram support.
pub fn tokenize(text: &str) -> Vec<String> {
    let lower = text.to_lowercase();
    let mut tokens: Vec<String> = Vec::default();

    // Latin/numeric tokens
    let mut current_word = String::default();
    for ch in lower.chars() {
        if ch.is_ascii_alphanumeric() {
            current_word.push(ch);
        } else {
            if !current_word.is_empty() {
                tokens.push(current_word.clone());
                current_word.clear();
            }
        }
    }
    if !current_word.is_empty() {
        tokens.push(current_word);
    }

    // CJK segments: individual chars + bi-grams
    let cjk_chars: Vec<&str> = lower
        .matches(|c| {
            ('\u{3040}'..='\u{309F}').contains(&c)  // Hiragana
                || ('\u{30A0}'..='\u{30FF}').contains(&c)  // Katakana
                || ('\u{4E00}'..='\u{9FFF}').contains(&c)  // CJK Unified Ideographs
                || ('\u{AC00}'..='\u{D7AF}').contains(&c) // Hangul
        })
        .collect();

    for ch in &cjk_chars {
        tokens.push(ch.to_string());
    }
    for window in cjk_chars.windows(2) {
        tokens.push(format!("{}{}", window[0], window[1]));
    }

    tokens
}
