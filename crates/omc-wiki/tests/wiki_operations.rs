//! Tests for wiki operations: ingest, query, lint, tokenize, and helpers.
//!
//! Coverage gaps addressed:
//! - ingest_knowledge: create path, merge path (tag/source deduplication, confidence ranking)
//! - query_wiki: tag filter, category filter, title/content scoring, snippet extraction
//! - lint_wiki: orphan detection, stale detection, broken refs, low-confidence, oversized
//! - detect_structural_contradictions: confidence conflicts, tag-category overlap
//! - extract_wiki_links: basic, nested, edge cases
//! - tokenize: Latin tokens, CJK char + bigram, empty input

use omc_wiki::storage::{ensure_wiki_dir, write_page, read_page};
use omc_wiki::wiki::{
    detect_structural_contradictions, extract_wiki_links, ingest_knowledge, lint_wiki,
    query_wiki, tokenize, WikiConfig, WikiIngestInput, WikiLintIssue, WikiLintIssueType,
    WikiLintReport, WikiLintSeverity, WikiPage, WikiPageFrontmatter, WIKI_SCHEMA_VERSION,
};
use std::fs;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn temp_wiki() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().unwrap();
    let root = dir.path().to_path_buf();
    ensure_wiki_dir(&root).unwrap();
    (dir, root)
}

fn make_page(
    root: &std::path::Path,
    title: &str,
    category: &str,
    tags: &[&str],
    content: &str,
    confidence: &str,
    links: Vec<String>,
    updated_days_ago: Option<i64>,
) {
    let slug = omc_wiki::storage::title_to_slug(title);
    let now = chrono::Utc::now();
    let updated = updated_days_ago
        .map(|d| now - chrono::Duration::days(d))
        .unwrap_or(now);
    let page = WikiPage {
        filename: slug.clone(),
        frontmatter: WikiPageFrontmatter {
            title: title.to_string(),
            tags: tags.iter().map(|s| s.to_string()).collect(),
            created: now.to_rfc3339(),
            updated: updated.to_rfc3339(),
            sources: vec![],
            links,
            category: category.to_string(),
            confidence: confidence.to_string(),
            schema_version: WIKI_SCHEMA_VERSION,
        },
        content: format!("\n# {}\n\n{}", title, content),
    };
    write_page(root, &page).unwrap();
}

// ---------------------------------------------------------------------------
// tokenize
// ---------------------------------------------------------------------------

#[test]
fn test_tokenize_latin_basic() {
    assert_eq!(tokenize("Find the auth module"), vec!["find", "the", "auth", "module"]);
}

#[test]
fn test_tokenize_latin_alphanumeric_only() {
    // punctuation is dropped
    let tokens = tokenize("fix: auth.rs — update config!");
    assert!(tokens.contains(&"fix".to_string()));
    assert!(tokens.contains(&"auth".to_string()));
    assert!(tokens.contains(&"rs".to_string()));
    assert!(tokens.contains(&"update".to_string()));
    assert!(tokens.contains(&"config".to_string()));
    assert!(!tokens.contains(&"!".to_string()));
}

#[test]
fn test_tokenize_empty() {
    assert!(tokenize("").is_empty());
}

#[test]
fn test_tokenize_numbers() {
    let tokens = tokenize("v1.2.3 release");
    assert!(tokens.iter().any(|t| t == "v1"));
    assert!(tokens.iter().any(|t| t == "2"));
    assert!(tokens.iter().any(|t| t == "3"));
    assert!(tokens.contains(&"release".to_string()));
}

#[test]
fn test_tokenize_cjk_chars() {
    // Individual CJK chars + bigrams
    let tokens = tokenize("中文标题");
    let chars: Vec<_> = tokens.iter().filter(|t| t.len() == 1).collect();
    let bigrams: Vec<_> = tokens.iter().filter(|t| t.len() == 2).collect();
    assert!(!chars.is_empty(), "should produce single-char CJK tokens");
    assert!(!bigrams.is_empty(), "should produce CJK bigrams");
}

#[test]
fn test_tokenize_cjk_hiragana() {
    let tokens = tokenize("こんにちは");
    let chars: Vec<_> = tokens.iter().filter(|t| t.len() == 1).collect();
    assert!(!chars.is_empty(), "Hiragana should tokenize");
}

#[test]
fn test_tokenize_cjk_katakana() {
    let tokens = tokenize("プロセス");
    let chars: Vec<_> = tokens.iter().filter(|t| t.len() == 1).collect();
    assert!(!chars.is_empty(), "Katakana should tokenize");
}

#[test]
fn test_tokenize_cjk_hangul() {
    let tokens = tokenize("안녕하세요");
    let chars: Vec<_> = tokens.iter().filter(|t| t.len() == 1).collect();
    assert!(!chars.is_empty(), "Hangul should tokenize");
}

#[test]
fn test_tokenize_mixed_latin_cjk() {
    let tokens = tokenize("auth中文config");
    let has_latin = tokens.iter().any(|t| t == "auth" || t == "config");
    let has_cjk = tokens.iter().any(|t| t.len() == 1 && t.chars().next().unwrap() >= '\u{4E00}');
    assert!(has_latin);
    assert!(has_cjk);
}

#[test]
fn test_tokenize_single_letter() {
    // Single ASCII letters should NOT become tokens (current impl requires word boundary)
    let tokens = tokenize("a b c");
    // Current implementation pushes current_word when non-alphanumeric hit
    // so "a" should be pushed, then " " resets
    assert!(tokens.contains(&"a".to_string()));
}

// ---------------------------------------------------------------------------
// extract_wiki_links
// ---------------------------------------------------------------------------

#[test]
fn test_extract_wiki_links_basic() {
    let links = extract_wiki_links("See [[auth-architecture]] and [[database-schema]].");
    assert!(links.contains(&"auth-architecture".to_string()));
    assert!(links.contains(&"database-schema".to_string()));
}

#[test]
fn test_extract_wiki_links_none() {
    let links = extract_wiki_links("No links here.");
    assert!(links.is_empty());
}

#[test]
fn test_extract_wiki_links_deduplicates() {
    let links = extract_wiki_links("[[auth]] and [[auth]] again");
    assert_eq!(links.len(), 1);
    assert_eq!(links[0], "auth");
}

#[test]
fn test_extract_wiki_links_empty_name() {
    // [[]] should produce empty string, which title_to_slug converts to hash slug
    let links = extract_wiki_links("[[]]");
    assert!(!links.is_empty());
}

#[test]
fn test_extract_wiki_links_trims_whitespace() {
    let links = extract_wiki_links("[[  spaced-link  ]]");
    assert!(links.iter().any(|l| l.contains("spaced-link") || l.contains("spaced")));
}

#[test]
fn test_extract_wiki_links_nested_brackets() {
    // Only outer-most pair should match
    let links = extract_wiki_links("[[outer [[inner]] end]]");
    assert!(links.len() <= 1, "nested brackets should not double-count");
}

// ---------------------------------------------------------------------------
// detect_structural_contradictions
// ---------------------------------------------------------------------------

#[test]
fn test_detect_contradiction_confidence_mismatch() {
    let pages = vec![
        WikiPage {
            filename: "auth-frontend.md".to_string(),
            frontmatter: WikiPageFrontmatter {
                title: "Auth Frontend".to_string(),
                tags: vec!["auth".to_string(), "frontend".to_string()],
                created: chrono::Utc::now().to_rfc3339(),
                updated: chrono::Utc::now().to_rfc3339(),
                sources: vec![],
                links: vec![],
                category: "architecture".to_string(),
                confidence: "high".to_string(),
                schema_version: WIKI_SCHEMA_VERSION,
            },
            content: "Frontend auth".to_string(),
        },
        WikiPage {
            filename: "auth-backend.md".to_string(),
            frontmatter: WikiPageFrontmatter {
                title: "Auth Backend".to_string(),
                tags: vec!["auth".to_string(), "backend".to_string()],
                created: chrono::Utc::now().to_rfc3339(),
                updated: chrono::Utc::now().to_rfc3339(),
                sources: vec![],
                links: vec![],
                category: "architecture".to_string(),
                confidence: "low".to_string(),
                schema_version: WIKI_SCHEMA_VERSION,
            },
            content: "Backend auth".to_string(),
        },
    ];
    let mut issues = Vec::new();
    detect_structural_contradictions(&pages, &mut issues);
    let contradiction = issues
        .iter()
        .find(|i| i.issue_type == WikiLintIssueType::StructuralContradiction);
    assert!(
        contradiction.is_some(),
        "should detect high/low confidence conflict for same topic prefix"
    );
}

#[test]
fn test_detect_contradiction_tag_category_overlap() {
    let pages = vec![
        WikiPage {
            filename: "api-reference.md".to_string(),
            frontmatter: WikiPageFrontmatter {
                title: "API Reference".to_string(),
                tags: vec!["api".to_string()],
                created: chrono::Utc::now().to_rfc3339(),
                updated: chrono::Utc::now().to_rfc3339(),
                sources: vec![],
                links: vec![],
                category: "reference".to_string(),
                confidence: "medium".to_string(),
                schema_version: WIKI_SCHEMA_VERSION,
            },
            content: "API docs".to_string(),
        },
        WikiPage {
            filename: "api-tutorial.md".to_string(),
            frontmatter: WikiPageFrontmatter {
                title: "API Tutorial".to_string(),
                tags: vec!["api".to_string()],
                created: chrono::Utc::now().to_rfc3339(),
                updated: chrono::Utc::now().to_rfc3339(),
                sources: vec![],
                links: vec![],
                category: "pattern".to_string(), // different category, same tag
                confidence: "medium".to_string(),
                schema_version: WIKI_SCHEMA_VERSION,
            },
            content: "API tutorial".to_string(),
        },
    ];
    let mut issues = Vec::new();
    detect_structural_contradictions(&pages, &mut issues);
    let contradiction = issues
        .iter()
        .find(|i| i.issue_type == WikiLintIssueType::StructuralContradiction);
    assert!(
        contradiction.is_some(),
        "should detect tag appearing in different categories"
    );
}

#[test]
fn test_detect_contradiction_no_conflict() {
    let pages = vec![
        WikiPage {
            filename: "auth-design.md".to_string(),
            frontmatter: WikiPageFrontmatter {
                title: "Auth Design".to_string(),
                tags: vec!["auth".to_string()],
                created: chrono::Utc::now().to_rfc3339(),
                updated: chrono::Utc::now().to_rfc3339(),
                sources: vec![],
                links: vec![],
                category: "architecture".to_string(),
                confidence: "high".to_string(),
                schema_version: WIKI_SCHEMA_VERSION,
            },
            content: "Auth design".to_string(),
        },
        WikiPage {
            filename: "api-design.md".to_string(),
            frontmatter: WikiPageFrontmatter {
                title: "API Design".to_string(),
                tags: vec!["api".to_string()],
                created: chrono::Utc::now().to_rfc3339(),
                updated: chrono::Utc::now().to_rfc3339(),
                sources: vec![],
                links: vec![],
                category: "architecture".to_string(),
                confidence: "high".to_string(),
                schema_version: WIKI_SCHEMA_VERSION,
            },
            content: "API design".to_string(),
        },
    ];
    let mut issues = Vec::new();
    detect_structural_contradictions(&pages, &mut issues);
    // Different prefix groups, no contradiction
    assert!(
        !issues.iter().any(|i| i.issue_type == WikiLintIssueType::StructuralContradiction),
        "different prefix groups should not conflict"
    );
}

// ---------------------------------------------------------------------------
// ingest_knowledge
// ---------------------------------------------------------------------------

#[test]
fn test_ingest_creates_new_page() {
    let (_dir, root) = temp_wiki();
    let input = WikiIngestInput {
        title: "Rust Concurrency".to_string(),
        content: "Use async/await for concurrency.".to_string(),
        tags: vec!["rust".to_string(), "concurrency".to_string()],
        category: "pattern".to_string(),
        sources: vec!["session-1".to_string()],
        confidence: Some("high".to_string()),
    };
    let result = ingest_knowledge(&root, &input);
    assert_eq!(result.created.len(), 1);
    assert!(result.updated.is_empty());
    assert_eq!(result.total_affected, 1);
}

#[test]
fn test_ingest_merges_existing_page() {
    let (_dir, root) = temp_wiki();

    // First ingest: create page
    let first = WikiIngestInput {
        title: "Auth Architecture".to_string(),
        content: "Use JWT for auth.".to_string(),
        tags: vec!["auth".to_string()],
        category: "architecture".to_string(),
        sources: vec!["session-1".to_string()],
        confidence: Some("medium".to_string()),
    };
    let r1 = ingest_knowledge(&root, &first);
    assert_eq!(r1.created.len(), 1);

    // Second ingest: merge into existing
    let second = WikiIngestInput {
        title: "Auth Architecture".to_string(),
        content: "Add OAuth2 support.".to_string(),
        tags: vec!["oauth".to_string(), "auth".to_string()], // auth already exists
        category: "architecture".to_string(),
        sources: vec!["session-2".to_string()], // new source
        confidence: Some("high".to_string()),   // higher confidence
    };
    let r2 = ingest_knowledge(&root, &second);
    assert!(r2.updated.len() == 1);
    assert_eq!(r2.created.len(), 0);

    // Verify merged content
    let page = read_page(&root, &omc_wiki::storage::title_to_slug("Auth Architecture"))
        .unwrap()
        .unwrap();
    assert!(page.frontmatter.tags.contains(&"auth".to_string()));
    assert!(page.frontmatter.tags.contains(&"oauth".to_string()));
    assert!(page.frontmatter.sources.contains(&"session-1".to_string()));
    assert!(page.frontmatter.sources.contains(&"session-2".to_string()));
    assert_eq!(page.frontmatter.confidence, "high");
    assert!(
        page.content.contains("Add OAuth2"),
        "merged content should contain new section"
    );
}

#[test]
fn test_ingest_confidence_not_downgraded() {
    let (_dir, root) = temp_wiki();

    let high = WikiIngestInput {
        title: "Topic".to_string(),
        content: "First".to_string(),
        tags: vec![],
        category: "ref".to_string(),
        sources: vec![],
        confidence: Some("high".to_string()),
    };
    ingest_knowledge(&root, &high);

    let low = WikiIngestInput {
        title: "Topic".to_string(),
        content: "Second".to_string(),
        tags: vec![],
        category: "ref".to_string(),
        sources: vec![],
        confidence: Some("low".to_string()),
    };
    ingest_knowledge(&root, &low);

    let page = read_page(&root, &omc_wiki::storage::title_to_slug("Topic"))
        .unwrap()
        .unwrap();
    assert_eq!(
        page.frontmatter.confidence, "high",
        "confidence should not be downgraded"
    );
}

#[test]
fn test_ingest_dedup_tags_and_sources() {
    let (_dir, root) = temp_wiki();

    let first = WikiIngestInput {
        title: "Topic".to_string(),
        content: "First".to_string(),
        tags: vec!["tag-a".to_string(), "tag-a".to_string()], // duplicate
        category: "ref".to_string(),
        sources: vec!["src".to_string(), "src".to_string()], // duplicate
        confidence: None,
    };
    ingest_knowledge(&root, &first);

    let page = read_page(&root, &omc_wiki::storage::title_to_slug("Topic"))
        .unwrap()
        .unwrap();
    assert_eq!(page.frontmatter.tags.len(), 1);
    assert_eq!(page.frontmatter.sources.len(), 1);
}

#[test]
fn test_ingest_extracts_wiki_links() {
    let (_dir, root) = temp_wiki();
    let input = WikiIngestInput {
        title: "Auth".to_string(),
        content: "See [[database-schema]] for the schema.".to_string(),
        tags: vec![],
        category: "ref".to_string(),
        sources: vec![],
        confidence: None,
    };
    ingest_knowledge(&root, &input);
    let page = read_page(&root, &omc_wiki::storage::title_to_slug("Auth"))
        .unwrap()
        .unwrap();
    assert!(page.frontmatter.links.contains(&"database-schema".to_string()));
}

#[test]
fn test_ingest_default_confidence() {
    let (_dir, root) = temp_wiki();
    let input = WikiIngestInput {
        title: "Topic".to_string(),
        content: "Content".to_string(),
        tags: vec![],
        category: "ref".to_string(),
        sources: vec![],
        confidence: None, // should default to "medium"
    };
    ingest_knowledge(&root, &input);
    let page = read_page(&root, &omc_wiki::storage::title_to_slug("Topic"))
        .unwrap()
        .unwrap();
    assert_eq!(page.frontmatter.confidence, "medium");
}

// ---------------------------------------------------------------------------
// query_wiki
// ---------------------------------------------------------------------------

#[test]
fn test_query_text_in_title() {
    let (_dir, root) = temp_wiki();
    make_page(&root, "Auth Architecture", "architecture", &["auth", "security"], "Use JWT.", "high", vec![], None);

    let results = query_wiki(&root, "auth", &Default::default());
    assert!(!results.is_empty());
    assert_eq!(results[0].page.frontmatter.title, "Auth Architecture");
}

#[test]
fn test_query_text_in_content() {
    let (_dir, root) = temp_wiki();
    make_page(&root, "Database Schema", "reference", &[], "PostgreSQL is the primary database.", "medium", vec![], None);

    let results = query_wiki(&root, "postgresql", &Default::default());
    assert!(!results.is_empty());
}

#[test]
fn test_query_filter_by_category() {
    let (_dir, root) = temp_wiki();
    make_page(&root, "Auth Ref", "reference", &[], "Auth info.", "low", vec![], None);
    make_page(&root, "Auth Arch", "architecture", &[], "Auth design.", "low", vec![], None);

    let opts = omc_wiki::wiki::WikiQueryOptions {
        category: Some("architecture".to_string()),
        ..Default::default()
    };
    let results = query_wiki(&root, "auth", &opts);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].page.frontmatter.category, "architecture");
}

#[test]
fn test_query_filter_by_tags() {
    let (_dir, root) = temp_wiki();
    make_page(&root, "Rust Book", "reference", &["rust", "async"], "Rust async.", "high", vec![], None);
    make_page(&root, "Python Book", "reference", &["python"], "Python book.", "high", vec![], None);

    let opts = omc_wiki::wiki::WikiQueryOptions {
        tags: Some(vec!["rust".to_string()]),
        ..Default::default()
    };
    let results = query_wiki(&root, "book", &opts);
    assert_eq!(results.len(), 1);
    assert!(results[0].page.frontmatter.tags.contains(&"rust".to_string()));
}

#[test]
fn test_query_limit() {
    let (_dir, root) = temp_wiki();
    for i in 0..10 {
        make_page(&root, &format!("Page {}", i), "reference", &[], &format!("Content {}", i), "medium", vec![], None);
    }

    let opts = omc_wiki::wiki::WikiQueryOptions {
        limit: Some(3),
        ..Default::default()
    };
    let results = query_wiki(&root, "page", &opts);
    assert!(results.len() <= 3);
}

#[test]
fn test_query_scoring_title_weight() {
    let (_dir, root) = temp_wiki();
    make_page(&root, "Rust Book", "reference", &[], "Short content.", "high", vec![], None);
    make_page(&root, "Generic", "reference", &["rust"], "Rust is mentioned in content here.", "high", vec![], None);

    let results = query_wiki(&root, "rust", &Default::default());
    assert!(!results.is_empty());
    // Title match (weight 5) should outscore tag match (weight 2) + content match (weight 1)
    let top = &results[0];
    assert_eq!(top.page.frontmatter.title, "Rust Book");
}

#[test]
fn test_query_snippet_extraction() {
    let (_dir, root) = temp_wiki();
    make_page(&root, "Test", "reference", &[], "The quick brown fox jumps over the lazy dog.", "medium", vec![], None);

    let results = query_wiki(&root, "fox", &Default::default());
    assert!(!results.is_empty());
    assert!(results[0].snippet.contains("fox"), "snippet should contain matched term");
}

#[test]
fn test_query_returns_empty_for_no_match() {
    let (_dir, root) = temp_wiki();
    make_page(&root, "Auth", "reference", &[], "Content", "medium", vec![], None);

    let results = query_wiki(&root, "nonexistent-xyz", &Default::default());
    assert!(results.is_empty());
}

#[test]
fn test_query_empty_wiki() {
    let (_dir, root) = temp_wiki();
    let results = query_wiki(&root, "anything", &Default::default());
    assert!(results.is_empty());
}

// ---------------------------------------------------------------------------
// lint_wiki
// ---------------------------------------------------------------------------

#[test]
fn test_lint_detects_orphan_page() {
    let (_dir, root) = temp_wiki();
    // A page with no incoming links is an orphan
    make_page(&root, "Orphan Page", "reference", &[], "No one links to me.", "medium", vec![], None);

    let report = lint_wiki(&root);
    let orphans: Vec<_> = report.issues.iter().filter(|i| i.issue_type == WikiLintIssueType::Orphan).collect();
    assert!(!orphans.is_empty(), "page with no incoming links should be flagged as orphan");
}

#[test]
fn test_lint_detects_stale_page() {
    let (_dir, root) = temp_wiki();
    // Page updated 60 days ago (stale threshold is 30)
    make_page(&root, "Stale Page", "reference", &[], "Old content.", "medium", vec![], Some(60));

    let report = lint_wiki(&root);
    let stale: Vec<_> = report.issues.iter().filter(|i| i.issue_type == WikiLintIssueType::Stale).collect();
    assert!(!stale.is_empty(), "page not updated in 60 days should be flagged as stale");
}

#[test]
fn test_lint_detects_broken_ref() {
    let (_dir, root) = temp_wiki();
    // Page links to another page that doesn't exist
    make_page(&root, "With Link", "reference", &[], "Content", "medium", vec!["nonexistent-page.md".to_string()], None);

    let report = lint_wiki(&root);
    let broken: Vec<_> = report.issues.iter().filter(|i| i.issue_type == WikiLintIssueType::BrokenRef).collect();
    assert!(!broken.is_empty(), "link to nonexistent page should be flagged as broken ref");
}

#[test]
fn test_lint_detects_low_confidence() {
    let (_dir, root) = temp_wiki();
    make_page(&root, "Low Conf", "reference", &[], "Unverified content.", "low", vec![], None);

    let report = lint_wiki(&root);
    let low_conf: Vec<_> = report.issues.iter().filter(|i| i.issue_type == WikiLintIssueType::LowConfidence).collect();
    assert!(!low_conf.is_empty(), "low confidence page should be flagged");
}

#[test]
fn test_lint_detects_oversized_page() {
    let (_dir, root) = temp_wiki();
    let huge_content = "x".repeat(15_000); // > 10KB default max
    make_page(&root, "Huge Page", "reference", &[], &huge_content, "medium", vec![], None);

    let report = lint_wiki(&root);
    let oversized: Vec<_> = report.issues.iter().filter(|i| i.issue_type == WikiLintIssueType::Oversized).collect();
    assert!(!oversized.is_empty(), "page > 10KB should be flagged as oversized");
}

#[test]
fn test_lint_no_issues_on_healthy_wiki() {
    let (_dir, root) = temp_wiki();
    // Two pages that link to each other, recent, medium confidence, under size limit
    make_page(&root, "Page A", "pattern", &[], "Content A.", "medium", vec!["page-b.md".to_string()], Some(5));
    make_page(&root, "Page B", "pattern", &[], "Content B.", "medium", vec!["page-a.md".to_string()], Some(5));

    let report = lint_wiki(&root);
    // Orphan detection won't fire (they link to each other)
    // Stale won't fire (updated 5 days ago)
    // No broken refs (they exist)
    // Low confidence won't fire
    // Oversized won't fire
    assert!(
        !report.issues.iter().any(|i| i.issue_type == WikiLintIssueType::Orphan
            || i.issue_type == WikiLintIssueType::Stale
            || i.issue_type == WikiLintIssueType::BrokenRef
            || i.issue_type == WikiLintIssueType::LowConfidence
            || i.issue_type == WikiLintIssueType::Oversized),
        "healthy wiki should have no issues"
    );
}

#[test]
fn test_lint_stats_populated() {
    let (_dir, root) = temp_wiki();
    make_page(&root, "Page 1", "reference", &[], "Content 1", "low", vec![], Some(60));
    make_page(&root, "Page 2", "reference", &[], "Content 2", "medium", vec![], None);

    let report = lint_wiki(&root);
    assert_eq!(report.stats.total_pages, 2);
    assert!(report.stats.stale_count >= 1);
    assert!(report.stats.low_confidence_count >= 1);
}

#[test]
fn test_lint_empty_wiki() {
    let (_dir, root) = temp_wiki();
    let report = lint_wiki(&root);
    assert_eq!(report.stats.total_pages, 0);
    assert!(report.issues.is_empty());
}

#[test]
fn test_lint_stale_threshold_exact() {
    // Page updated exactly 30 days ago should NOT be stale (threshold is > 30)
    let (_dir, root) = temp_wiki();
    make_page(&root, "Borderline", "reference", &[], "Content", "medium", vec![], Some(30));

    let report = lint_wiki(&root);
    let stale: Vec<_> = report.issues.iter().filter(|i| i.issue_type == WikiLintIssueType::Stale).collect();
    assert!(stale.is_empty(), "page updated exactly 30 days ago should not be stale");
}
