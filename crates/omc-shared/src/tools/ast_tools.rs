//! AST Tools using ast-grep
//!
//! Provides AST-aware code search and transformation:
//! - Pattern matching with meta-variables ($VAR, $$$)
//! - Code replacement while preserving structure
//! - Support for 25+ programming languages
//!
//! The actual ast-grep execution is delegated to the MCP server runtime,
//! which invokes the `sg` CLI or the ast-grep-core library.

use super::{ToolAnnotations, ToolCategory, ToolInfo, ToolResult};
use serde_json::{Value, json};

/// Supported programming languages for AST analysis.
pub const SUPPORTED_LANGUAGES: &[&str] = &[
    "javascript",
    "typescript",
    "tsx",
    "python",
    "ruby",
    "go",
    "rust",
    "java",
    "kotlin",
    "swift",
    "c",
    "cpp",
    "csharp",
    "html",
    "css",
    "json",
    "yaml",
];

/// Map file extensions to language identifiers.
pub const EXT_TO_LANG: &[(&str, &str)] = &[
    (".js", "javascript"),
    (".mjs", "javascript"),
    (".cjs", "javascript"),
    (".jsx", "javascript"),
    (".ts", "typescript"),
    (".mts", "typescript"),
    (".cts", "typescript"),
    (".tsx", "tsx"),
    (".py", "python"),
    (".rb", "ruby"),
    (".go", "go"),
    (".rs", "rust"),
    (".java", "java"),
    (".kt", "kotlin"),
    (".kts", "kotlin"),
    (".swift", "swift"),
    (".c", "c"),
    (".h", "c"),
    (".cpp", "cpp"),
    (".cc", "cpp"),
    (".cxx", "cpp"),
    (".hpp", "cpp"),
    (".cs", "csharp"),
    (".html", "html"),
    (".htm", "html"),
    (".css", "css"),
    (".json", "json"),
    (".yaml", "yaml"),
    (".yml", "yaml"),
];

/// Look up the language identifier for a file extension.
pub fn lang_for_extension(ext: &str) -> Option<&'static str> {
    let ext_lower = ext.to_ascii_lowercase();
    EXT_TO_LANG
        .iter()
        .find(|(e, _)| *e == ext_lower)
        .map(|(_, lang)| *lang)
}

// ---------------------------------------------------------------------------
// Tool definitions
// ---------------------------------------------------------------------------

/// All AST tool definitions.
pub fn all_ast_tools() -> Vec<ToolInfo> {
    vec![ast_grep_search_info(), ast_grep_replace_info()]
}

// ---------------------------------------------------------------------------
// 1. ast_grep_search
// ---------------------------------------------------------------------------

pub fn ast_grep_search_info() -> ToolInfo {
    ToolInfo {
        name: "ast_grep_search",
        description: "Search for code patterns using AST matching. More precise than text search.\n\n\
             Use meta-variables in patterns:\n\
             - $NAME - matches any single AST node (identifier, expression, etc.)\n\
             - $$$ARGS - matches multiple nodes (for function arguments, list items, etc.)\n\n\
             Examples:\n\
             - \"function $NAME($$$ARGS)\" - find all function declarations\n\
             - \"console.log($MSG)\" - find all console.log calls\n\
             - \"if ($COND) { $$$BODY }\" - find all if statements\n\
             - \"$X === null\" - find null equality checks\n\
             - \"import $$$IMPORTS from '$MODULE'\" - find imports\n\n\
             Note: Patterns must be valid AST nodes for the language.",
        category: ToolCategory::Ast,
        annotations: ToolAnnotations {
            read_only_hint: Some(true),
            ..Default::default()
        },
        input_schema: json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "AST pattern with meta-variables ($VAR, $$$VARS)"
                },
                "language": {
                    "type": "string",
                    "enum": [
                        "javascript", "typescript", "tsx", "python", "ruby",
                        "go", "rust", "java", "kotlin", "swift", "c", "cpp",
                        "csharp", "html", "css", "json", "yaml"
                    ],
                    "description": "Programming language"
                },
                "path": {
                    "type": "string",
                    "description": "Directory or file to search (default: current directory)"
                },
                "context": {
                    "type": "integer",
                    "minimum": 0,
                    "maximum": 10,
                    "description": "Lines of context around matches (default: 2)"
                },
                "maxResults": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 100,
                    "description": "Maximum results to return (default: 20)"
                }
            },
            "required": ["pattern", "language"]
        }),
    }
}

pub async fn ast_grep_search(args: Value) -> ToolResult {
    let pattern = match required_str(&args, "pattern") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let language = match required_str(&args, "language") {
        Ok(v) => v,
        Err(e) => return e,
    };

    if !SUPPORTED_LANGUAGES.contains(&language.as_str()) {
        return ToolResult::error(format!(
            "Unsupported language: {language}\n\nSupported: {}",
            SUPPORTED_LANGUAGES.join(", ")
        ));
    }

    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .unwrap_or(".")
        .to_string();
    let context = args
        .get("context")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(2);
    let max_results = args
        .get("maxResults")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(20);

    let sg = match require_ast_grep("search") {
        Ok(s) => s,
        Err(e) => return e,
    };
    sg.search(
        &pattern,
        &language,
        &path,
        context as usize,
        max_results as usize,
    )
    .await
}

// ---------------------------------------------------------------------------
// 2. ast_grep_replace
// ---------------------------------------------------------------------------

pub fn ast_grep_replace_info() -> ToolInfo {
    ToolInfo {
        name: "ast_grep_replace",
        description: "Replace code patterns using AST matching. Preserves matched content via meta-variables.\n\n\
             Use meta-variables in both pattern and replacement:\n\
             - $NAME in pattern captures a node, use $NAME in replacement to insert it\n\
             - $$$ARGS captures multiple nodes\n\n\
             Examples:\n\
             - Pattern: \"console.log($MSG)\" -> Replacement: \"logger.info($MSG)\"\n\
             - Pattern: \"var $NAME = $VALUE\" -> Replacement: \"const $NAME = $VALUE\"\n\
             - Pattern: \"$OBJ.forEach(($ITEM) => { $$$BODY })\" -> \
               Replacement: \"for (const $ITEM of $OBJ) { $$$BODY }\"\n\n\
             IMPORTANT: dryRun=true (default) only previews changes. Set dryRun=false to apply.",
        category: ToolCategory::Ast,
        annotations: ToolAnnotations {
            read_only_hint: Some(false),
            destructive_hint: Some(true),
            ..Default::default()
        },
        input_schema: json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Pattern to match"
                },
                "replacement": {
                    "type": "string",
                    "description": "Replacement pattern (use same meta-variables)"
                },
                "language": {
                    "type": "string",
                    "enum": [
                        "javascript", "typescript", "tsx", "python", "ruby",
                        "go", "rust", "java", "kotlin", "swift", "c", "cpp",
                        "csharp", "html", "css", "json", "yaml"
                    ],
                    "description": "Programming language"
                },
                "path": {
                    "type": "string",
                    "description": "Directory or file to search (default: current directory)"
                },
                "dryRun": {
                    "type": "boolean",
                    "description": "Preview only, don't apply changes (default: true)"
                }
            },
            "required": ["pattern", "replacement", "language"]
        }),
    }
}

pub async fn ast_grep_replace(args: Value) -> ToolResult {
    let pattern = match required_str(&args, "pattern") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let replacement = match required_str(&args, "replacement") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let language = match required_str(&args, "language") {
        Ok(v) => v,
        Err(e) => return e,
    };

    if !SUPPORTED_LANGUAGES.contains(&language.as_str()) {
        return ToolResult::error(format!(
            "Unsupported language: {language}\n\nSupported: {}",
            SUPPORTED_LANGUAGES.join(", ")
        ));
    }

    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .unwrap_or(".")
        .to_string();
    let dry_run = args
        .get("dryRun")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);

    let sg = match require_ast_grep("replace") {
        Ok(s) => s,
        Err(e) => return e,
    };
    sg.replace(&pattern, &replacement, &language, &path, dry_run)
        .await
}

// ---------------------------------------------------------------------------
// AstGrep trait — abstracts the ast-grep execution backend
// ---------------------------------------------------------------------------

/// Trait abstracting the ast-grep execution engine.
///
/// The MCP server runtime provides a concrete implementation that invokes
/// the `sg` CLI or uses ast-grep-core directly.
#[async_trait::async_trait]
pub trait AstGrep: Send + Sync {
    async fn search(
        &self,
        pattern: &str,
        language: &str,
        path: &str,
        context: usize,
        max_results: usize,
    ) -> ToolResult;

    async fn replace(
        &self,
        pattern: &str,
        replacement: &str,
        language: &str,
        path: &str,
        dry_run: bool,
    ) -> ToolResult;
}

// ---------------------------------------------------------------------------
// Client registry — set by the MCP server at startup
// ---------------------------------------------------------------------------

use std::sync::OnceLock;

static AST_GREP: OnceLock<Box<dyn AstGrep>> = OnceLock::new();

/// Register the ast-grep implementation. Called once by the MCP server at startup.
pub fn set_ast_grep(sg: Box<dyn AstGrep>) {
    let _ = AST_GREP.set(sg);
}

/// Get a reference to the registered ast-grep implementation, if any.
pub fn get_ast_grep() -> Option<&'static dyn AstGrep> {
    AST_GREP.get().map(std::convert::AsRef::as_ref)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn require_ast_grep(operation: &str) -> Result<&'static dyn AstGrep, ToolResult> {
    get_ast_grep().ok_or_else(|| {
        ToolResult::error(format!(
            "ast-grep is not available.\n\n\
             Install it with: npm install -g @ast-grep/cli\n\
             Or configure the MCP server with an ast-grep implementation.\n\
             (operation: {operation})"
        ))
    })
}

fn required_str(args: &Value, key: &str) -> Result<String, ToolResult> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| ToolResult::error(format!("Missing required parameter: {key}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn supported_languages_count() {
        assert_eq!(SUPPORTED_LANGUAGES.len(), 17);
    }

    #[test]
    fn ext_to_lang_covers_common_extensions() {
        assert_eq!(lang_for_extension(".rs"), Some("rust"));
        assert_eq!(lang_for_extension(".ts"), Some("typescript"));
        assert_eq!(lang_for_extension(".py"), Some("python"));
        assert_eq!(lang_for_extension(".go"), Some("go"));
        assert_eq!(lang_for_extension(".unknown"), None);
    }

    #[test]
    fn all_ast_tools_count() {
        assert_eq!(all_ast_tools().len(), 2);
    }

    #[test]
    fn search_tool_is_read_only() {
        let info = ast_grep_search_info();
        assert_eq!(info.annotations.read_only_hint, Some(true));
    }

    #[test]
    fn replace_tool_is_destructive() {
        let info = ast_grep_replace_info();
        assert_eq!(info.annotations.read_only_hint, Some(false));
        assert_eq!(info.annotations.destructive_hint, Some(true));
    }

    #[test]
    fn unsupported_language_returns_error() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(ast_grep_search(json!({
            "pattern": "fn $NAME",
            "language": "brainfuck"
        })));
        assert_eq!(result.is_error, Some(true));
        assert!(result.content[0].text.contains("Unsupported language"));
    }

    #[test]
    fn missing_pattern_returns_error() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(ast_grep_search(json!({
            "language": "rust"
        })));
        assert_eq!(result.is_error, Some(true));
        assert!(
            result.content[0]
                .text
                .contains("Missing required parameter: pattern")
        );
    }

    #[test]
    fn no_backend_returns_error() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(ast_grep_search(json!({
            "pattern": "fn $NAME",
            "language": "rust"
        })));
        assert_eq!(result.is_error, Some(true));
        assert!(result.content[0].text.contains("ast-grep is not available"));
    }

    #[test]
    fn replace_schema_requires_replacement() {
        let info = ast_grep_replace_info();
        let required = info.input_schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("pattern")));
        assert!(required.contains(&json!("replacement")));
        assert!(required.contains(&json!("language")));
    }
}
