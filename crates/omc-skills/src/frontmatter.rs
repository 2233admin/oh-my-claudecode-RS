//! Frontmatter parsing module - extracts YAML metadata from skill files

use regex::Regex;
use serde_yaml::Error as YamlError;
use thiserror::Error;

use crate::loader::SkillMetadata;

/// Errors that can occur during frontmatter parsing
#[derive(Error, Debug)]
pub enum FrontmatterError {
    #[error("No frontmatter found")]
    NoFrontmatter,
    #[error("Invalid YAML: {0}")]
    YamlError(#[from] YamlError),
    #[error("Missing required field: {0}")]
    MissingField(String),
}

/// Parse YAML frontmatter from skill content
///
/// The frontmatter is expected to be at the beginning of the file, enclosed
/// in `---` delimiters:
///
/// ```yaml
/// ---
/// name: my-skill
/// description: A skill that does something
/// ---
/// ```
///
/// # Arguments
///
/// * `content` - The raw file content including frontmatter
///
/// # Returns
///
/// * `Ok(SkillMetadata)` if parsing succeeds
/// * `Err(FrontmatterError)` if parsing fails
pub fn parse_frontmatter(content: &str) -> Result<SkillMetadata, FrontmatterError> {
    // Match frontmatter block between --- markers
    let frontmatter_re = Regex::new(r"(?s)^---\s*\n(.*?)\n---").unwrap();

    let caps = frontmatter_re
        .captures(content)
        .ok_or(FrontmatterError::NoFrontmatter)?;

    let yaml_str = caps.get(1).ok_or(FrontmatterError::NoFrontmatter)?.as_str();

    let metadata: SkillMetadata = serde_yaml::from_str(yaml_str)?;

    // Validate required fields
    if metadata.name.is_empty() {
        return Err(FrontmatterError::MissingField("name".to_string()));
    }
    if metadata.description.is_empty() {
        return Err(FrontmatterError::MissingField("description".to_string()));
    }

    Ok(metadata)
}

/// Extract just the body content (without frontmatter)
pub fn extract_body(content: &str) -> Option<String> {
    let frontmatter_re = Regex::new(r"(?s)^---\s*\n.*?\n---\s*\n(.*)").unwrap();
    frontmatter_re
        .captures(content)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_frontmatter() {
        let content = r#"---
name: test-skill
description: A test skill
argument_hint: "task: string"
level: beginner
aliases: [alias1, alias2]
agent: researcher
model: opus
---

# Skill Body

Some content here.
"#;

        let metadata = parse_frontmatter(content).unwrap();

        assert_eq!(metadata.name, "test-skill");
        assert_eq!(metadata.description, "A test skill");
        assert_eq!(metadata.argument_hint, Some("task: string".to_string()));
        assert_eq!(metadata.level, Some("beginner".to_string()));
        assert_eq!(metadata.aliases, vec!["alias1", "alias2"]);
        assert_eq!(metadata.agent, Some("researcher".to_string()));
        assert_eq!(metadata.model, Some("opus".to_string()));
    }

    #[test]
    fn test_parse_minimal_frontmatter() {
        let content = r#"---
name: minimal
description: Minimal skill
---

Content
"#;

        let metadata = parse_frontmatter(content).unwrap();

        assert_eq!(metadata.name, "minimal");
        assert_eq!(metadata.description, "Minimal skill");
        assert!(metadata.argument_hint.is_none());
        assert!(metadata.level.is_none());
        assert!(metadata.aliases.is_empty());
    }

    #[test]
    fn test_parse_missing_frontmatter() {
        let content = "Just some content without frontmatter";

        let result = parse_frontmatter(content);
        assert!(matches!(result, Err(FrontmatterError::NoFrontmatter)));
    }

    #[test]
    fn test_parse_missing_name() {
        let content = r#"---
description: Missing name
---

Content
"#;

        let result = parse_frontmatter(content);
        assert!(matches!(result, Err(FrontmatterError::MissingField(_))));
    }

    #[test]
    fn test_parse_missing_description() {
        let content = r#"---
name: no-description
---

Content
"#;

        let result = parse_frontmatter(content);
        assert!(matches!(result, Err(FrontmatterError::MissingField(_))));
    }

    #[test]
    fn test_extract_body() {
        let content = r#"---
name: test
description: Test
---

# Heading

Body content here.
"#;

        let body = extract_body(content).unwrap();
        assert!(body.contains("# Heading"));
        assert!(body.contains("Body content here."));
    }

    #[test]
    fn test_extract_body_no_frontmatter() {
        let content = "# Just heading\n\nBody only";
        assert!(extract_body(content).is_none());
    }

    #[test]
    fn test_parse_with_inline_yaml() {
        // Test that YAML arrays can be parsed inline
        let content = r#"---
name: inline-array
description: Test
aliases: [one, two, three]
---

Content
"#;

        let metadata = parse_frontmatter(content).unwrap();
        assert_eq!(metadata.aliases, vec!["one", "two", "three"]);
    }

    #[test]
    fn test_parse_with_multiline_yaml() {
        // Test multiline YAML values
        let content = r#"---
name: multiline
description: |
  This is a multiline
  description that spans
  multiple lines
---

Content
"#;

        let metadata = parse_frontmatter(content).unwrap();
        assert!(metadata.description.contains("multiline"));
    }
}
