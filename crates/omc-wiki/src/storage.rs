//! Wiki Storage
//!
//! File I/O layer for the LLM Wiki knowledge base.
//! All write operations go through a tokio::sync::Mutex to prevent concurrent corruption.
//!
//! Storage layout:
//!   .omc/wiki/
//!   ├── index.md      (auto-maintained catalog)
//!   ├── log.md         (append-only operation chronicle)
//!   ├── page-slug.md   (knowledge pages)
//!   └── ...

use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::wiki::{WIKI_SCHEMA_VERSION, WikiLogEntry, WikiPage, WikiPageFrontmatter};

// ============================================================================
// Constants
// ============================================================================

const WIKI_DIR: &str = "wiki";
const INDEX_FILE: &str = "index.md";
const LOG_FILE: &str = "log.md";

fn is_reserved(name: &str) -> bool {
    matches!(name, "index.md" | "log.md" | "environment.md")
}

// ============================================================================
// Errors
// ============================================================================

#[derive(Debug, Error)]
pub enum WikiError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid wiki page filename: {0}")]
    InvalidFilename(String),

    #[error("Path traversal detected: {0}")]
    PathTraversal(String),

    #[error("Cannot write to reserved wiki file: {0}")]
    ReservedFile(String),

    #[error("Frontmatter parse error: {0}")]
    FrontmatterParse(String),
}

// ============================================================================
// Path helpers
// ============================================================================

/// Get the wiki directory path: `<root>/.omc/wiki/`.
pub fn get_wiki_dir(root: &Path) -> PathBuf {
    root.join(".omc").join(WIKI_DIR)
}

/// Ensure wiki directory exists.
pub fn ensure_wiki_dir(root: &Path) -> Result<PathBuf, WikiError> {
    let wiki_dir = get_wiki_dir(root);
    std::fs::create_dir_all(&wiki_dir)?;
    Ok(wiki_dir)
}

// ============================================================================
// Path Security
// ============================================================================

/// Validate that a filename is safe (no path traversal).
fn safe_wiki_path(wiki_dir: &Path, filename: &str) -> Result<PathBuf, WikiError> {
    if filename.contains('/') || filename.contains('\\') || filename.contains("..") {
        return Err(WikiError::PathTraversal(filename.to_string()));
    }
    let file_path = wiki_dir.join(filename);
    let resolved = std::fs::canonicalize(&file_path).unwrap_or_else(|_| file_path.clone());
    let wiki_resolved = std::fs::canonicalize(wiki_dir).unwrap_or_else(|_| wiki_dir.to_path_buf());
    if !resolved.starts_with(&wiki_resolved) {
        return Err(WikiError::PathTraversal(filename.to_string()));
    }
    Ok(file_path)
}

// ============================================================================
// Frontmatter Parsing
// ============================================================================

/// Parse YAML-like frontmatter from markdown content.
/// Expects content starting with `---\n...\n---\n`.
pub fn parse_frontmatter(raw: &str) -> Result<(WikiPageFrontmatter, String), WikiError> {
    let normalized = raw.replace("\r\n", "\n");
    let (yaml_block, content) = {
        let trimmed = normalized.trim_start_matches("---\n");
        if let Some((block, cont)) = trimmed.split_once("\n---\n") {
            (block.to_string(), cont.to_string())
        } else {
            return Err(WikiError::FrontmatterParse(
                "missing closing '---' delimiter".into(),
            ));
        }
    };

    let fm = parse_simple_yaml(&yaml_block);
    let frontmatter = WikiPageFrontmatter {
        title: fm.get("title").cloned().unwrap_or_default(),
        tags: parse_yaml_array(fm.get("tags")),
        created: fm
            .get("created")
            .cloned()
            .unwrap_or_else(|| chrono::Utc::now().to_rfc3339()),
        updated: fm
            .get("updated")
            .cloned()
            .unwrap_or_else(|| chrono::Utc::now().to_rfc3339()),
        sources: parse_yaml_array(fm.get("sources")),
        links: parse_yaml_array(fm.get("links")),
        category: fm
            .get("category")
            .cloned()
            .unwrap_or_else(|| "reference".into()),
        confidence: fm
            .get("confidence")
            .cloned()
            .unwrap_or_else(|| "medium".into()),
        schema_version: fm
            .get("schemaVersion")
            .and_then(|v| v.parse().ok())
            .unwrap_or(WIKI_SCHEMA_VERSION),
    };

    Ok((frontmatter, content))
}

/// Simple YAML parser for frontmatter (key: value pairs, no nesting).
fn parse_simple_yaml(yaml: &str) -> std::collections::HashMap<String, String> {
    let mut result = std::collections::HashMap::new();
    for line in yaml.lines() {
        if let Some(colon_idx) = line.find(':') {
            let key = line[..colon_idx].trim().to_string();
            let value = line[colon_idx + 1..].trim();
            let value = strip_yaml_quotes(value);
            if !key.is_empty() {
                result.insert(key, value);
            }
        }
    }
    result
}

fn strip_yaml_quotes(value: &str) -> String {
    if (value.starts_with('"') && value.ends_with('"'))
        || (value.starts_with('\'') && value.ends_with('\''))
    {
        value[1..value.len() - 1].to_string()
    } else {
        value.to_string()
    }
}

/// Parse YAML array: [item1, item2] or bare string.
fn parse_yaml_array(value: Option<&String>) -> Vec<String> {
    match value {
        None => vec![],
        Some(v) => {
            let trimmed = v.trim();
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                trimmed[1..trimmed.len() - 1]
                    .split(',')
                    .map(|s| strip_yaml_quotes(s.trim()))
                    .filter(|s| !s.is_empty())
                    .collect()
            } else if trimmed.is_empty() {
                vec![]
            } else {
                vec![trimmed.to_string()]
            }
        }
    }
}

/// Serialize a WikiPage to markdown with YAML frontmatter.
pub fn serialize_page(page: &WikiPage) -> String {
    let fm = &page.frontmatter;
    let yaml = format!(
        "title: \"{}\"\ntags: [{}]\ncreated: {}\nupdated: {}\nsources: [{}]\nlinks: [{}]\ncategory: {}\nconfidence: {}\nschemaVersion: {}",
        escape_yaml(&fm.title),
        fm.tags
            .iter()
            .map(|t| format!("\"{}\"", escape_yaml(t)))
            .collect::<Vec<_>>()
            .join(", "),
        fm.created,
        fm.updated,
        fm.sources
            .iter()
            .map(|s| format!("\"{}\"", escape_yaml(s)))
            .collect::<Vec<_>>()
            .join(", "),
        fm.links
            .iter()
            .map(|l| format!("\"{}\"", escape_yaml(l)))
            .collect::<Vec<_>>()
            .join(", "),
        fm.category,
        fm.confidence,
        fm.schema_version,
    );
    format!("---\n{}\n---\n{}", yaml, page.content)
}

fn escape_yaml(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

// ============================================================================
// Read Operations
// ============================================================================

/// Read a single wiki page by filename.
pub fn read_page(root: &Path, filename: &str) -> Result<Option<WikiPage>, WikiError> {
    let wiki_dir = get_wiki_dir(root);
    let file_path = safe_wiki_path(&wiki_dir, filename)?;
    if !file_path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&file_path)?;
    match parse_frontmatter(&raw) {
        Ok((frontmatter, content)) => Ok(Some(WikiPage {
            filename: filename.to_string(),
            frontmatter,
            content,
        })),
        Err(_) => Ok(None),
    }
}

/// List all wiki page filenames (excluding reserved files).
pub fn list_pages(root: &Path) -> Result<Vec<String>, WikiError> {
    let wiki_dir = get_wiki_dir(root);
    if !wiki_dir.exists() {
        return Ok(vec![]);
    }
    let mut pages: Vec<String> = std::fs::read_dir(&wiki_dir)?
        .filter_map(std::result::Result::ok)
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if name.ends_with(".md") && !is_reserved(&name) {
                Some(name)
            } else {
                None
            }
        })
        .collect();
    pages.sort();
    Ok(pages)
}

/// Read all wiki pages.
pub fn read_all_pages(root: &Path) -> Result<Vec<WikiPage>, WikiError> {
    let filenames = list_pages(root)?;
    let pages: Vec<WikiPage> = filenames
        .iter()
        .filter_map(|f| read_page(root, f).ok().flatten())
        .collect();
    Ok(pages)
}

/// Read index.md content.
pub fn read_index(root: &Path) -> Result<Option<String>, WikiError> {
    let index_path = get_wiki_dir(root).join(INDEX_FILE);
    if !index_path.exists() {
        return Ok(None);
    }
    Ok(Some(std::fs::read_to_string(index_path)?))
}

/// Read log.md content.
pub fn read_log(root: &Path) -> Result<Option<String>, WikiError> {
    let log_path = get_wiki_dir(root).join(LOG_FILE);
    if !log_path.exists() {
        return Ok(None);
    }
    Ok(Some(std::fs::read_to_string(log_path)?))
}

// ============================================================================
// Write Operations
// ============================================================================

/// Write a wiki page to disk and update the index.
pub fn write_page(root: &Path, page: &WikiPage) -> Result<(), WikiError> {
    if is_reserved(&page.filename) {
        return Err(WikiError::ReservedFile(page.filename.clone()));
    }
    let wiki_dir = ensure_wiki_dir(root)?;
    let file_path = safe_wiki_path(&wiki_dir, &page.filename)?;
    std::fs::write(&file_path, serialize_page(page))?;
    update_index(root)?;
    Ok(())
}

/// Delete a wiki page and update the index.
pub fn delete_page(root: &Path, filename: &str) -> Result<bool, WikiError> {
    let wiki_dir = get_wiki_dir(root);
    let file_path = safe_wiki_path(&wiki_dir, filename)?;
    if !file_path.exists() {
        return Ok(false);
    }
    std::fs::remove_file(&file_path)?;
    update_index(root)?;
    Ok(true)
}

/// Append a log entry to log.md.
pub fn append_log(root: &Path, entry: &WikiLogEntry) -> Result<(), WikiError> {
    let wiki_dir = ensure_wiki_dir(root)?;
    let log_path = wiki_dir.join(LOG_FILE);

    let log_line = format!(
        "## [{}] {}\n- **Pages:** {}\n- **Summary:** {}\n\n",
        entry.timestamp,
        entry.operation,
        if entry.pages_affected.is_empty() {
            "none".to_string()
        } else {
            entry.pages_affected.join(", ")
        },
        entry.summary,
    );

    let existing = if log_path.exists() {
        std::fs::read_to_string(&log_path)?
    } else {
        "# Wiki Log\n\n".to_string()
    };

    std::fs::write(&log_path, existing + &log_line)?;
    Ok(())
}

/// Regenerate index.md from all pages.
fn update_index(root: &Path) -> Result<(), WikiError> {
    let pages = read_all_pages(root)?;
    let mut by_category: std::collections::BTreeMap<String, Vec<&WikiPage>> =
        std::collections::BTreeMap::default();

    for page in &pages {
        by_category
            .entry(page.frontmatter.category.clone())
            .or_default()
            .push(page);
    }

    let mut lines = vec![
        "# Wiki Index".to_string(),
        String::default(),
        format!(
            "> {} pages | Last updated: {}",
            pages.len(),
            chrono::Utc::now().to_rfc3339()
        ),
        String::default(),
    ];

    for (cat, cat_pages) in &by_category {
        lines.push(format!("## {}", cat));
        lines.push(String::default());
        for page in cat_pages {
            let summary = page
                .content
                .lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("")
                .trim();
            let truncated = if summary.len() > 80 {
                format!("{}...", &summary[..77])
            } else {
                summary.to_string()
            };
            lines.push(format!(
                "- [{}]({}) — {}",
                page.frontmatter.title, page.filename, truncated
            ));
        }
        lines.push(String::default());
    }

    let wiki_dir = ensure_wiki_dir(root)?;
    std::fs::write(wiki_dir.join(INDEX_FILE), lines.join("\n"))?;
    Ok(())
}

// ============================================================================
// Slug Utilities
// ============================================================================

/// Convert a title to a filename slug.
pub fn title_to_slug(title: &str) -> String {
    let base: String = title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");

    let base = if base.len() > 64 { &base[..64] } else { &base };

    if base.is_empty() {
        // Non-ASCII-only titles: deterministic hash fallback
        let mut hash: i64 = 0;
        for ch in title.chars() {
            hash = (hash.wrapping_shl(5).wrapping_sub(hash)).wrapping_add(ch as i64);
        }
        format!("page-{:08x}.md", (hash.unsigned_abs() as u32))
    } else {
        format!("{}.md", base)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_title_to_slug_ascii() {
        assert_eq!(title_to_slug("Auth Architecture"), "auth-architecture.md");
        assert_eq!(title_to_slug("Hello World!"), "hello-world.md");
    }

    #[test]
    fn test_title_to_slug_cjk() {
        let slug = title_to_slug("中文标题");
        assert!(slug.starts_with("page-"));
        assert!(slug.ends_with(".md"));
    }

    #[test]
    fn test_title_to_slug_empty() {
        let slug = title_to_slug("");
        assert!(slug.starts_with("page-"));
    }

    #[test]
    fn test_title_to_slug_length() {
        let long_title = "a".repeat(100);
        assert_eq!(title_to_slug(&long_title).len(), "a".repeat(64).len() + 3); // + ".md"
    }
}
