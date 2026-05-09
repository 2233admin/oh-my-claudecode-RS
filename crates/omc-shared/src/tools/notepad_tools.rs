//! Notepad Tools
//!
//! Provides tools for reading and writing notepad sections.
//! The notepad is a markdown file used for cross-session persistent notes.
//!
//! Path: `.omc/notepad.md`
//! Sections: Priority Context, Working Memory, MANUAL

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use super::ToolResult;
use crate::config::OmcPaths;

/// Section name for notepad operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NotepadSection {
    All,
    Priority,
    Working,
    Manual,
}

impl NotepadSection {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Priority => "priority",
            Self::Working => "working",
            Self::Manual => "manual",
        }
    }
}

impl std::fmt::Display for NotepadSection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Statistics about the notepad.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotepadStats {
    pub exists: bool,
    pub total_size: usize,
    pub priority_size: usize,
    pub working_memory_entries: usize,
    pub oldest_entry: Option<String>,
    pub path: String,
}

/// Default age threshold for pruning working memory entries (7 days).
const DEFAULT_PRUNE_DAYS: u32 = 7;

/// Timestamp format used in working memory entries.
#[allow(dead_code)]
const TIMESTAMP_PREFIX_LEN: usize = 22; // "[YYYY-MM-DD HH:MM:SS] "

fn notepad_path(paths: &OmcPaths) -> PathBuf {
    paths.home.join("notepad.md")
}

fn ensure_omc_dir(paths: &OmcPaths) -> Result<(), std::io::Error> {
    fs::create_dir_all(&paths.home)
}

/// Parse the notepad markdown file into sections.
fn parse_notepad(path: &Path) -> Option<String> {
    if !path.exists() {
        return None;
    }
    fs::read_to_string(path).ok()
}

/// Extract a specific section from the full notepad content.
fn extract_section(content: &str, section: &str) -> Option<String> {
    let section_header = format!("## {section}");
    let start = content.find(&section_header)?;
    let after_header = &content[start + section_header.len()..];

    // Find next section header or end of file
    let end = after_header.find("\n## ").unwrap_or(after_header.len());

    let section_content = after_header[..end].trim();
    if section_content.is_empty() {
        return None;
    }
    Some(section_content.to_string())
}

/// Read the notepad content.
///
/// Can read the full notepad or a specific section (priority, working, manual).
pub fn notepad_read(section: NotepadSection, working_directory: Option<&str>) -> ToolResult {
    let paths = resolve_paths(working_directory);
    let path = notepad_path(&paths);

    match section {
        NotepadSection::All => {
            let content = parse_notepad(&path);
            match content {
                Some(c) if !c.trim().is_empty() => {
                    ToolResult::text(format!("## Notepad\n\nPath: {}\n\n{}", path.display(), c))
                }
                _ => ToolResult::text(
                    "Notepad does not exist. Use notepad_write_* tools to create it.",
                ),
            }
        }
        section => {
            let section_name = match section {
                NotepadSection::Priority => "Priority Context",
                NotepadSection::Working => "Working Memory",
                NotepadSection::Manual => "MANUAL",
                _ => unreachable!(),
            };

            let content = parse_notepad(&path);
            match content.and_then(|c| extract_section(&c, section_name)) {
                Some(section_content) => {
                    ToolResult::text(format!("## {section_name}\n\n{section_content}"))
                }
                None => ToolResult::text(format!(
                    "## {section_name}\n\n(Empty or notepad does not exist)"
                )),
            }
        }
    }
}

/// Write to the Priority Context section.
///
/// This REPLACES the existing content. Keep under 500 chars - this is always loaded at session start.
pub fn notepad_write_priority(content: &str, working_directory: Option<&str>) -> ToolResult {
    let paths = resolve_paths(working_directory);
    if let Err(e) = ensure_omc_dir(&paths) {
        return ToolResult::error(format!("Error creating directory: {e}"));
    }

    let path = notepad_path(&paths);
    let existing = parse_notepad(&path).unwrap_or_default();

    let new_content = replace_section(&existing, "Priority Context", content);

    match fs::write(&path, &new_content) {
        Ok(()) => {
            let mut response = format!(
                "Successfully wrote to Priority Context ({} chars)",
                content.len()
            );
            if content.len() > 500 {
                response.push_str("\n\n**Warning:** Content exceeds recommended 500 char limit for Priority Context.");
            }
            ToolResult::text(response)
        }
        Err(e) => ToolResult::error(format!("Error writing to Priority Context: {e}")),
    }
}

/// Add an entry to Working Memory section.
///
/// Entries are timestamped and auto-pruned after 7 days.
pub fn notepad_write_working(content: &str, working_directory: Option<&str>) -> ToolResult {
    let paths = resolve_paths(working_directory);
    if let Err(e) = ensure_omc_dir(&paths) {
        return ToolResult::error(format!("Error creating directory: {e}"));
    }

    let path = notepad_path(&paths);
    let existing = parse_notepad(&path).unwrap_or_default();
    let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S");
    let entry = format!("[{timestamp}] {content}");

    let new_content = append_to_section(&existing, "Working Memory", &entry);

    match fs::write(&path, &new_content) {
        Ok(()) => ToolResult::text(format!(
            "Successfully added entry to Working Memory ({} chars)",
            content.len()
        )),
        Err(e) => ToolResult::error(format!("Error writing to Working Memory: {e}")),
    }
}

/// Add an entry to the MANUAL section.
///
/// Content in this section is never auto-pruned.
pub fn notepad_write_manual(content: &str, working_directory: Option<&str>) -> ToolResult {
    let paths = resolve_paths(working_directory);
    if let Err(e) = ensure_omc_dir(&paths) {
        return ToolResult::error(format!("Error creating directory: {e}"));
    }

    let path = notepad_path(&paths);
    let existing = parse_notepad(&path).unwrap_or_default();
    let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S");
    let entry = format!("[{timestamp}] {content}");

    let new_content = append_to_section(&existing, "MANUAL", &entry);

    match fs::write(&path, &new_content) {
        Ok(()) => ToolResult::text(format!(
            "Successfully added entry to MANUAL section ({} chars)",
            content.len()
        )),
        Err(e) => ToolResult::error(format!("Error writing to MANUAL: {e}")),
    }
}

/// Prune Working Memory entries older than N days.
pub fn notepad_prune(days_old: Option<u32>, working_directory: Option<&str>) -> ToolResult {
    let days = days_old.unwrap_or(DEFAULT_PRUNE_DAYS);
    let paths = resolve_paths(working_directory);
    let path = notepad_path(&paths);

    let content = match parse_notepad(&path) {
        Some(c) => c,
        None => return ToolResult::text("## Prune Results\n\nNotepad does not exist."),
    };

    let section_content = match extract_section(&content, "Working Memory") {
        Some(s) => s,
        None => {
            return ToolResult::text(format!(
                "## Prune Results\n\n- Pruned: 0 entries\n- Remaining: 0 entries\n- Threshold: {days} days"
            ));
        }
    };

    let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
    let mut pruned = 0usize;
    let mut remaining = 0usize;
    let mut kept_lines = Vec::new();

    for line in section_content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Some(entry_date) = parse_entry_timestamp(trimmed)
            && entry_date < cutoff
        {
            pruned += 1;
            continue;
        }
        remaining += 1;
        kept_lines.push(trimmed.to_string());
    }

    // Rebuild the section
    let new_section = if kept_lines.is_empty() {
        String::new()
    } else {
        kept_lines.join("\n")
    };

    let new_content = replace_section(&content, "Working Memory", &new_section);

    if let Err(e) = fs::write(&path, &new_content) {
        return ToolResult::error(format!("Error writing pruned notepad: {e}"));
    }

    ToolResult::text(format!(
        "## Prune Results\n\n- Pruned: {pruned} entries\n- Remaining: {remaining} entries\n- Threshold: {days} days"
    ))
}

/// Get statistics about the notepad.
pub fn notepad_stats(working_directory: Option<&str>) -> ToolResult {
    let paths = resolve_paths(working_directory);
    let path = notepad_path(&paths);

    if !path.exists() {
        return ToolResult::text("## Notepad Statistics\n\nNotepad does not exist yet.");
    }

    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => return ToolResult::error(format!("Error reading notepad: {e}")),
    };

    let total_size = content.len();
    let priority_size = extract_section(&content, "Priority Context")
        .map(|s| s.len())
        .unwrap_or(0);
    let working_memory_entries = extract_section(&content, "Working Memory")
        .map(|s| s.lines().filter(|l| !l.trim().is_empty()).count())
        .unwrap_or(0);
    let oldest_entry = find_oldest_entry(&content);

    ToolResult::text(format!(
        "## Notepad Statistics\n\n- **Total Size:** {total_size} bytes\n- **Priority Context Size:** {priority_size} bytes\n- **Working Memory Entries:** {working_memory_entries}\n- **Oldest Entry:** {}\n- **Path:** {}",
        oldest_entry.as_deref().unwrap_or("None"),
        path.display()
    ))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn resolve_paths(working_directory: Option<&str>) -> OmcPaths {
    match working_directory {
        Some(dir) => OmcPaths::new_with_root(PathBuf::from(dir).join(".omc")),
        None => OmcPaths::new(),
    }
}

/// Replace the content of a section (or create it if missing).
fn replace_section(content: &str, section_name: &str, new_content: &str) -> String {
    let header = format!("## {section_name}");
    let mut result = String::new();

    if let Some(start) = content.find(&header) {
        // Find the end of this section
        let after_header = &content[start + header.len()..];
        let end = after_header.find("\n## ").unwrap_or(after_header.len());

        // Keep everything before and after this section
        let before = &content[..start];
        let after = &content[start + header.len() + end..];

        result.push_str(before);
        result.push_str(&header);
        result.push('\n');
        result.push('\n');
        if !new_content.is_empty() {
            result.push_str(new_content);
            result.push('\n');
        }
        result.push_str(after);
    } else {
        // Section doesn't exist, append it
        result.push_str(content);
        if !content.is_empty() && !content.ends_with('\n') {
            result.push('\n');
        }
        result.push('\n');
        result.push_str(&header);
        result.push('\n');
        result.push('\n');
        if !new_content.is_empty() {
            result.push_str(new_content);
            result.push('\n');
        }
    }

    result
}

/// Append an entry to a section (or create it if missing).
fn append_to_section(content: &str, section_name: &str, entry: &str) -> String {
    let header = format!("## {section_name}");

    if let Some(start) = content.find(&header) {
        let after_header = &content[start + header.len()..];
        let end = after_header.find("\n## ").unwrap_or(after_header.len());

        let before = &content[..start + header.len() + end];
        let after = &content[start + header.len() + end..];

        let mut result = String::new();
        result.push_str(before);
        if !before.ends_with('\n') {
            result.push('\n');
        }
        result.push_str(entry);
        result.push('\n');
        result.push_str(after);
        result
    } else {
        let mut result = String::new();
        result.push_str(content);
        if !content.is_empty() && !content.ends_with('\n') {
            result.push('\n');
        }
        result.push('\n');
        result.push_str(&header);
        result.push('\n');
        result.push('\n');
        result.push_str(entry);
        result.push('\n');
        result
    }
}

/// Parse a timestamp from an entry like `[YYYY-MM-DD HH:MM:SS] content`.
fn parse_entry_timestamp(entry: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    if !entry.starts_with('[') {
        return None;
    }
    let end = entry.find(']')?;
    let ts_str = &entry[1..end];
    chrono::NaiveDateTime::parse_from_str(ts_str, "%Y-%m-%d %H:%M:%S")
        .ok()
        .map(|naive| naive.and_utc())
}

/// Find the oldest entry in the notepad.
fn find_oldest_entry(content: &str) -> Option<String> {
    let working = extract_section(content, "Working Memory")?;
    let mut oldest: Option<chrono::DateTime<chrono::Utc>> = None;
    let mut oldest_str: Option<String> = None;

    for line in working.lines() {
        let trimmed = line.trim();
        if let Some(ts) = parse_entry_timestamp(trimmed)
            && (oldest.is_none() || ts < oldest.unwrap())
        {
            oldest = Some(ts);
            oldest_str = Some(trimmed.to_string());
        }
    }

    oldest_str
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, String) {
        let tmp = TempDir::new().unwrap();
        let wd = tmp.path().to_string_lossy().to_string();
        (tmp, wd)
    }

    #[test]
    fn test_notepad_read_empty() {
        let (_tmp, wd) = setup();
        let result = notepad_read(NotepadSection::All, Some(&wd));
        assert!(result.content[0].text.contains("does not exist"));
    }

    #[test]
    fn test_notepad_write_and_read_priority() {
        let (_tmp, wd) = setup();

        notepad_write_priority("This is high priority", Some(&wd));
        let result = notepad_read(NotepadSection::Priority, Some(&wd));
        assert!(result.content[0].text.contains("This is high priority"));
    }

    #[test]
    fn test_notepad_write_working() {
        let (_tmp, wd) = setup();

        notepad_write_working("Working on feature X", Some(&wd));
        let result = notepad_read(NotepadSection::Working, Some(&wd));
        assert!(result.content[0].text.contains("Working on feature X"));
        // Should have a timestamp prefix
        assert!(result.content[0].text.contains("["));
    }

    #[test]
    fn test_notepad_write_manual() {
        let (_tmp, wd) = setup();

        notepad_write_manual("Manual instruction", Some(&wd));
        let result = notepad_read(NotepadSection::Manual, Some(&wd));
        assert!(result.content[0].text.contains("Manual instruction"));
    }

    #[test]
    fn test_notepad_stats() {
        let (_tmp, wd) = setup();

        notepad_write_priority("Priority content", Some(&wd));
        notepad_write_working("Working entry", Some(&wd));
        notepad_write_working("Another entry", Some(&wd));

        let result = notepad_stats(Some(&wd));
        assert!(result.content[0].text.contains("Total Size"));
        assert!(result.content[0].text.contains("Working Memory Entries"));
    }

    #[test]
    fn test_notepad_priority_replaces() {
        let (_tmp, wd) = setup();

        notepad_write_priority("First", Some(&wd));
        notepad_write_priority("Second", Some(&wd));

        let result = notepad_read(NotepadSection::Priority, Some(&wd));
        assert!(result.content[0].text.contains("Second"));
        assert!(!result.content[0].text.contains("First"));
    }

    #[test]
    fn test_extract_section() {
        let content = "## Priority Context\n\nhigh priority stuff\n\n## Working Memory\n\nworking stuff\n\n## MANUAL\n\nmanual stuff";
        assert_eq!(
            extract_section(content, "Priority Context"),
            Some("high priority stuff".into())
        );
        assert_eq!(
            extract_section(content, "Working Memory"),
            Some("working stuff".into())
        );
        assert_eq!(
            extract_section(content, "MANUAL"),
            Some("manual stuff".into())
        );
    }
}
