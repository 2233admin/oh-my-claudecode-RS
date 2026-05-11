//! LSP (Language Server Protocol) Tools
//!
//! Provides IDE-like capabilities to agents via real LSP server integration:
//! - Hover information
//! - Go to definition
//! - Find references
//! - Document/workspace symbols
//! - Diagnostics
//! - Completion
//! - Signature help
//! - Code actions
//! - Formatting
//! - Rename
//! - Code lens

use super::{ToolAnnotations, ToolCategory, ToolInfo, ToolResult};
use serde_json::{Value, json};

// ---------------------------------------------------------------------------
// Tool metadata
// ---------------------------------------------------------------------------

/// All 12 LSP tool definitions.
pub fn all_lsp_tools() -> Vec<ToolInfo> {
    vec![
        lsp_hover_info(),
        lsp_goto_definition_info(),
        lsp_find_references_info(),
        lsp_diagnostics_info(),
        lsp_document_symbols_info(),
        lsp_workspace_symbols_info(),
        lsp_completion_info(),
        lsp_signature_help_info(),
        lsp_code_actions_info(),
        lsp_formatting_info(),
        lsp_rename_info(),
        lsp_code_lens_info(),
    ]
}

// ---------------------------------------------------------------------------
// 1. lsp_hover
// ---------------------------------------------------------------------------

pub fn lsp_hover_info() -> ToolInfo {
    ToolInfo {
        name: "lsp_hover",
        description: "Get type information, documentation, and signature at a specific position in a file. \
             Useful for understanding what a symbol represents.",
        category: ToolCategory::Lsp,
        annotations: ToolAnnotations {
            read_only_hint: Some(true),
            ..Default::default()
        },
        input_schema: json!({
            "type": "object",
            "properties": {
                "file": {
                    "type": "string",
                    "description": "Path to the source file"
                },
                "line": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Line number (1-indexed)"
                },
                "character": {
                    "type": "integer",
                    "minimum": 0,
                    "description": "Character position in the line (0-indexed)"
                }
            },
            "required": ["file", "line", "character"]
        }),
    }
}

/// Handler for lsp_hover.
pub async fn lsp_hover(args: Value) -> ToolResult {
    let file = match required_str(&args, "file") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let line = match required_i64(&args, "line") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let character = match required_i64(&args, "character") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let client = match require_lsp_client("hover") {
        Ok(c) => c,
        Err(e) => return e,
    };
    client.hover(&file, line - 1, character).await
}

// ---------------------------------------------------------------------------
// 2. lsp_goto_definition
// ---------------------------------------------------------------------------

pub fn lsp_goto_definition_info() -> ToolInfo {
    ToolInfo {
        name: "lsp_goto_definition",
        description: "Find the definition location of a symbol (function, variable, class, etc.). \
             Returns the file path and position where the symbol is defined.",
        category: ToolCategory::Lsp,
        annotations: ToolAnnotations {
            read_only_hint: Some(true),
            ..Default::default()
        },
        input_schema: json!({
            "type": "object",
            "properties": {
                "file": {
                    "type": "string",
                    "description": "Path to the source file"
                },
                "line": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Line number (1-indexed)"
                },
                "character": {
                    "type": "integer",
                    "minimum": 0,
                    "description": "Character position in the line (0-indexed)"
                }
            },
            "required": ["file", "line", "character"]
        }),
    }
}

pub async fn lsp_goto_definition(args: Value) -> ToolResult {
    let file = match required_str(&args, "file") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let line = match required_i64(&args, "line") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let character = match required_i64(&args, "character") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let client = match require_lsp_client("goto definition") {
        Ok(c) => c,
        Err(e) => return e,
    };
    client.definition(&file, line - 1, character).await
}

// ---------------------------------------------------------------------------
// 3. lsp_find_references
// ---------------------------------------------------------------------------

pub fn lsp_find_references_info() -> ToolInfo {
    ToolInfo {
        name: "lsp_find_references",
        description: "Find all references to a symbol across the codebase. \
             Useful for understanding usage patterns and impact of changes.",
        category: ToolCategory::Lsp,
        annotations: ToolAnnotations {
            read_only_hint: Some(true),
            ..Default::default()
        },
        input_schema: json!({
            "type": "object",
            "properties": {
                "file": {
                    "type": "string",
                    "description": "Path to the source file"
                },
                "line": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Line number (1-indexed)"
                },
                "character": {
                    "type": "integer",
                    "minimum": 0,
                    "description": "Character position in the line (0-indexed)"
                },
                "includeDeclaration": {
                    "type": "boolean",
                    "description": "Include the declaration in results (default: true)"
                }
            },
            "required": ["file", "line", "character"]
        }),
    }
}

pub async fn lsp_find_references(args: Value) -> ToolResult {
    let file = match required_str(&args, "file") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let line = match required_i64(&args, "line") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let character = match required_i64(&args, "character") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let include_declaration = args
        .get("includeDeclaration")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);

    let client = match require_lsp_client("find references") {
        Ok(c) => c,
        Err(e) => return e,
    };
    client
        .references(&file, line - 1, character, include_declaration)
        .await
}

// ---------------------------------------------------------------------------
// 4. lsp_diagnostics
// ---------------------------------------------------------------------------

pub fn lsp_diagnostics_info() -> ToolInfo {
    ToolInfo {
        name: "lsp_diagnostics",
        description: "Get language server diagnostics (errors, warnings, hints) for a file. \
             Useful for finding issues without running the compiler.",
        category: ToolCategory::Lsp,
        annotations: ToolAnnotations {
            read_only_hint: Some(true),
            ..Default::default()
        },
        input_schema: json!({
            "type": "object",
            "properties": {
                "file": {
                    "type": "string",
                    "description": "Path to the source file"
                },
                "severity": {
                    "type": "string",
                    "enum": ["error", "warning", "info", "hint"],
                    "description": "Filter by severity level"
                }
            },
            "required": ["file"]
        }),
    }
}

pub async fn lsp_diagnostics(args: Value) -> ToolResult {
    let file = match required_str(&args, "file") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let severity = args
        .get("severity")
        .and_then(|v| v.as_str())
        .map(String::from);

    let client = match require_lsp_client("diagnostics") {
        Ok(c) => c,
        Err(e) => return e,
    };
    client.diagnostics(&file, severity.as_deref()).await
}

// ---------------------------------------------------------------------------
// 5. lsp_document_symbols
// ---------------------------------------------------------------------------

pub fn lsp_document_symbols_info() -> ToolInfo {
    ToolInfo {
        name: "lsp_document_symbols",
        description: "Get a hierarchical outline of all symbols in a file \
             (functions, classes, variables, etc.). Useful for understanding file structure.",
        category: ToolCategory::Lsp,
        annotations: ToolAnnotations {
            read_only_hint: Some(true),
            ..Default::default()
        },
        input_schema: json!({
            "type": "object",
            "properties": {
                "file": {
                    "type": "string",
                    "description": "Path to the source file"
                }
            },
            "required": ["file"]
        }),
    }
}

pub async fn lsp_document_symbols(args: Value) -> ToolResult {
    let file = match required_str(&args, "file") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let client = match require_lsp_client("document symbols") {
        Ok(c) => c,
        Err(e) => return e,
    };
    client.document_symbols(&file).await
}

// ---------------------------------------------------------------------------
// 6. lsp_workspace_symbols
// ---------------------------------------------------------------------------

pub fn lsp_workspace_symbols_info() -> ToolInfo {
    ToolInfo {
        name: "lsp_workspace_symbols",
        description: "Search for symbols (functions, classes, etc.) across the entire workspace by name. \
             Useful for finding definitions without knowing the exact file.",
        category: ToolCategory::Lsp,
        annotations: ToolAnnotations {
            read_only_hint: Some(true),
            ..Default::default()
        },
        input_schema: json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Symbol name or pattern to search"
                },
                "file": {
                    "type": "string",
                    "description": "Any file in the workspace (used to determine which language server to use)"
                }
            },
            "required": ["query", "file"]
        }),
    }
}

pub async fn lsp_workspace_symbols(args: Value) -> ToolResult {
    let query = match required_str(&args, "query") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let _file = match required_str(&args, "file") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let client = match require_lsp_client("workspace symbols") {
        Ok(c) => c,
        Err(e) => return e,
    };
    client.workspace_symbols(&query).await
}

// ---------------------------------------------------------------------------
// 7. lsp_completion
// ---------------------------------------------------------------------------

pub fn lsp_completion_info() -> ToolInfo {
    ToolInfo {
        name: "lsp_completion",
        description: "Get auto-completion suggestions at a specific position in a file. \
             Returns available completions including functions, variables, types, and snippets.",
        category: ToolCategory::Lsp,
        annotations: ToolAnnotations {
            read_only_hint: Some(true),
            ..Default::default()
        },
        input_schema: json!({
            "type": "object",
            "properties": {
                "file": {
                    "type": "string",
                    "description": "Path to the source file"
                },
                "line": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Line number (1-indexed)"
                },
                "character": {
                    "type": "integer",
                    "minimum": 0,
                    "description": "Character position in the line (0-indexed)"
                }
            },
            "required": ["file", "line", "character"]
        }),
    }
}

pub async fn lsp_completion(args: Value) -> ToolResult {
    let file = match required_str(&args, "file") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let line = match required_i64(&args, "line") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let character = match required_i64(&args, "character") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let client = match require_lsp_client("completion") {
        Ok(c) => c,
        Err(e) => return e,
    };
    client.completion(&file, line - 1, character).await
}

// ---------------------------------------------------------------------------
// 8. lsp_signature_help
// ---------------------------------------------------------------------------

pub fn lsp_signature_help_info() -> ToolInfo {
    ToolInfo {
        name: "lsp_signature_help",
        description: "Get function/method signature help at a call site. \
             Shows parameter names, types, and documentation for the function being called.",
        category: ToolCategory::Lsp,
        annotations: ToolAnnotations {
            read_only_hint: Some(true),
            ..Default::default()
        },
        input_schema: json!({
            "type": "object",
            "properties": {
                "file": {
                    "type": "string",
                    "description": "Path to the source file"
                },
                "line": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Line number (1-indexed)"
                },
                "character": {
                    "type": "integer",
                    "minimum": 0,
                    "description": "Character position in the line (0-indexed)"
                }
            },
            "required": ["file", "line", "character"]
        }),
    }
}

pub async fn lsp_signature_help(args: Value) -> ToolResult {
    let file = match required_str(&args, "file") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let line = match required_i64(&args, "line") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let character = match required_i64(&args, "character") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let client = match require_lsp_client("signature help") {
        Ok(c) => c,
        Err(e) => return e,
    };
    client.signature_help(&file, line - 1, character).await
}

// ---------------------------------------------------------------------------
// 9. lsp_code_actions
// ---------------------------------------------------------------------------

pub fn lsp_code_actions_info() -> ToolInfo {
    ToolInfo {
        name: "lsp_code_actions",
        description: "Get available code actions (refactorings, quick fixes) for a selection. \
             Returns a list of possible actions that can be applied.",
        category: ToolCategory::Lsp,
        annotations: ToolAnnotations {
            read_only_hint: Some(true),
            ..Default::default()
        },
        input_schema: json!({
            "type": "object",
            "properties": {
                "file": {
                    "type": "string",
                    "description": "Path to the source file"
                },
                "startLine": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Start line of selection (1-indexed)"
                },
                "startCharacter": {
                    "type": "integer",
                    "minimum": 0,
                    "description": "Start character of selection (0-indexed)"
                },
                "endLine": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "End line of selection (1-indexed)"
                },
                "endCharacter": {
                    "type": "integer",
                    "minimum": 0,
                    "description": "End character of selection (0-indexed)"
                }
            },
            "required": ["file", "startLine", "startCharacter", "endLine", "endCharacter"]
        }),
    }
}

pub async fn lsp_code_actions(args: Value) -> ToolResult {
    let file = match required_str(&args, "file") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let start_line = match required_i64(&args, "startLine") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let start_char = match required_i64(&args, "startCharacter") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let end_line = match required_i64(&args, "endLine") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let end_char = match required_i64(&args, "endCharacter") {
        Ok(v) => v,
        Err(e) => return e,
    };

    let client = match require_lsp_client("code actions") {
        Ok(c) => c,
        Err(e) => return e,
    };
    client
        .code_actions(&file, start_line - 1, start_char, end_line - 1, end_char)
        .await
}

// ---------------------------------------------------------------------------
// 10. lsp_formatting
// ---------------------------------------------------------------------------

pub fn lsp_formatting_info() -> ToolInfo {
    ToolInfo {
        name: "lsp_formatting",
        description: "Format a file using the language server's formatting provider. \
             Returns the formatted content or a list of edits to apply.",
        category: ToolCategory::Lsp,
        annotations: ToolAnnotations {
            read_only_hint: Some(false),
            idempotent_hint: Some(true),
            ..Default::default()
        },
        input_schema: json!({
            "type": "object",
            "properties": {
                "file": {
                    "type": "string",
                    "description": "Path to the source file"
                },
                "tabSize": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 16,
                    "description": "Tab/indentation size (default: 4)"
                },
                "insertSpaces": {
                    "type": "boolean",
                    "description": "Use spaces instead of tabs (default: true)"
                }
            },
            "required": ["file"]
        }),
    }
}

pub async fn lsp_formatting(args: Value) -> ToolResult {
    let file = match required_str(&args, "file") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let tab_size = args.get("tabSize").and_then(serde_json::Value::as_i64).unwrap_or(4);
    let insert_spaces = args
        .get("insertSpaces")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);

    let client = match require_lsp_client("formatting") {
        Ok(c) => c,
        Err(e) => return e,
    };
    client
        .formatting(&file, tab_size as u32, insert_spaces)
        .await
}

// ---------------------------------------------------------------------------
// 11. lsp_rename
// ---------------------------------------------------------------------------

pub fn lsp_rename_info() -> ToolInfo {
    ToolInfo {
        name: "lsp_rename",
        description: "Rename a symbol (variable, function, class, etc.) across all files in the project. \
             Returns the list of edits that would be made. Does NOT apply the changes automatically.",
        category: ToolCategory::Lsp,
        annotations: ToolAnnotations {
            read_only_hint: Some(true),
            ..Default::default()
        },
        input_schema: json!({
            "type": "object",
            "properties": {
                "file": {
                    "type": "string",
                    "description": "Path to the source file"
                },
                "line": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Line number (1-indexed)"
                },
                "character": {
                    "type": "integer",
                    "minimum": 0,
                    "description": "Character position in the line (0-indexed)"
                },
                "newName": {
                    "type": "string",
                    "minLength": 1,
                    "description": "New name for the symbol"
                }
            },
            "required": ["file", "line", "character", "newName"]
        }),
    }
}

pub async fn lsp_rename(args: Value) -> ToolResult {
    let file = match required_str(&args, "file") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let line = match required_i64(&args, "line") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let character = match required_i64(&args, "character") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let new_name = match required_str(&args, "newName") {
        Ok(v) => v,
        Err(e) => return e,
    };

    let client = match require_lsp_client("rename") {
        Ok(c) => c,
        Err(e) => return e,
    };
    client.rename(&file, line - 1, character, &new_name).await
}

// ---------------------------------------------------------------------------
// 12. lsp_code_lens
// ---------------------------------------------------------------------------

pub fn lsp_code_lens_info() -> ToolInfo {
    ToolInfo {
        name: "lsp_code_lens",
        description: "Get code lens items for a file. Code lenses are inline actionable items \
             (e.g., references count, run tests, implementations) shown above functions and classes.",
        category: ToolCategory::Lsp,
        annotations: ToolAnnotations {
            read_only_hint: Some(true),
            ..Default::default()
        },
        input_schema: json!({
            "type": "object",
            "properties": {
                "file": {
                    "type": "string",
                    "description": "Path to the source file"
                }
            },
            "required": ["file"]
        }),
    }
}

pub async fn lsp_code_lens(args: Value) -> ToolResult {
    let file = match required_str(&args, "file") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let client = match require_lsp_client("code lens") {
        Ok(c) => c,
        Err(e) => return e,
    };
    client.code_lens(&file).await
}

// ---------------------------------------------------------------------------
// Supplementary: lsp_servers (list available language servers)
// ---------------------------------------------------------------------------

pub fn lsp_servers_info() -> ToolInfo {
    ToolInfo {
        name: "lsp_servers",
        description: "List all known language servers and their installation status. \
             Shows which servers are available and how to install missing ones.",
        category: ToolCategory::Lsp,
        annotations: ToolAnnotations {
            read_only_hint: Some(true),
            ..Default::default()
        },
        input_schema: json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        }),
    }
}

pub async fn lsp_servers(_args: Value) -> ToolResult {
    // This is a placeholder — the actual implementation queries installed servers.
    // The MCP server (omc-mcp crate) wires this up with the real server registry.
    ToolResult::text(
        "## Language Server Status\n\n\
         Use lsp_diagnostics on a specific file to trigger server auto-detection.\n\
         Server registration is handled by the MCP server runtime.",
    )
}

// ---------------------------------------------------------------------------
// Supplementary: lsp_diagnostics_directory
// ---------------------------------------------------------------------------

pub fn lsp_diagnostics_directory_info() -> ToolInfo {
    ToolInfo {
        name: "lsp_diagnostics_directory",
        description: "Run project-level diagnostics on a directory using tsc --noEmit (preferred) \
             or LSP iteration (fallback). Useful for checking the entire codebase for errors.",
        category: ToolCategory::Lsp,
        annotations: ToolAnnotations {
            read_only_hint: Some(true),
            ..Default::default()
        },
        input_schema: json!({
            "type": "object",
            "properties": {
                "directory": {
                    "type": "string",
                    "description": "Project directory to check"
                },
                "strategy": {
                    "type": "string",
                    "enum": ["tsc", "lsp", "auto"],
                    "description": "Strategy: tsc, lsp, or auto (default: auto)"
                }
            },
            "required": ["directory"]
        }),
    }
}

pub async fn lsp_diagnostics_directory(args: Value) -> ToolResult {
    let directory = match required_str(&args, "directory") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let strategy = args
        .get("strategy")
        .and_then(|v| v.as_str())
        .unwrap_or("auto");

    ToolResult::text(format!(
        "## Directory Diagnostics\n\nDirectory: {directory}\nStrategy: {strategy}\n\n\
         Placeholder: the MCP server runtime wires this up with the real diagnostics runner."
    ))
}

// ---------------------------------------------------------------------------
// Supplementary: lsp_prepare_rename
// ---------------------------------------------------------------------------

pub fn lsp_prepare_rename_info() -> ToolInfo {
    ToolInfo {
        name: "lsp_prepare_rename",
        description: "Check if a symbol at the given position can be renamed. \
             Returns the range of the symbol if rename is possible.",
        category: ToolCategory::Lsp,
        annotations: ToolAnnotations {
            read_only_hint: Some(true),
            ..Default::default()
        },
        input_schema: json!({
            "type": "object",
            "properties": {
                "file": {
                    "type": "string",
                    "description": "Path to the source file"
                },
                "line": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Line number (1-indexed)"
                },
                "character": {
                    "type": "integer",
                    "minimum": 0,
                    "description": "Character position in the line (0-indexed)"
                }
            },
            "required": ["file", "line", "character"]
        }),
    }
}

pub async fn lsp_prepare_rename(args: Value) -> ToolResult {
    let file = match required_str(&args, "file") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let line = match required_i64(&args, "line") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let character = match required_i64(&args, "character") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let client = match require_lsp_client("prepare rename") {
        Ok(c) => c,
        Err(e) => return e,
    };
    client.prepare_rename(&file, line - 1, character).await
}

// ---------------------------------------------------------------------------
// Supplementary: lsp_code_action_resolve
// ---------------------------------------------------------------------------

pub fn lsp_code_action_resolve_info() -> ToolInfo {
    ToolInfo {
        name: "lsp_code_action_resolve",
        description: "Get the full edit details for a specific code action. \
             Use after lsp_code_actions to see what changes an action would make.",
        category: ToolCategory::Lsp,
        annotations: ToolAnnotations {
            read_only_hint: Some(true),
            ..Default::default()
        },
        input_schema: json!({
            "type": "object",
            "properties": {
                "file": {
                    "type": "string",
                    "description": "Path to the source file"
                },
                "startLine": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Start line of selection (1-indexed)"
                },
                "startCharacter": {
                    "type": "integer",
                    "minimum": 0,
                    "description": "Start character of selection (0-indexed)"
                },
                "endLine": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "End line of selection (1-indexed)"
                },
                "endCharacter": {
                    "type": "integer",
                    "minimum": 0,
                    "description": "End character of selection (0-indexed)"
                },
                "actionIndex": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Index of the action (1-indexed, from lsp_code_actions output)"
                }
            },
            "required": ["file", "startLine", "startCharacter", "endLine", "endCharacter", "actionIndex"]
        }),
    }
}

pub async fn lsp_code_action_resolve(args: Value) -> ToolResult {
    let file = match required_str(&args, "file") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let action_index = match required_i64(&args, "actionIndex") {
        Ok(v) => v,
        Err(e) => return e,
    };

    let client = match require_lsp_client("code action resolve") {
        Ok(c) => c,
        Err(e) => return e,
    };
    client.code_action_resolve(&file, action_index).await
}

// ---------------------------------------------------------------------------
// LspClient trait — abstracts the language server protocol client
// ---------------------------------------------------------------------------

/// Trait abstracting the LSP client operations.
///
/// The MCP server runtime provides a concrete implementation that communicates
/// with real language servers over JSON-RPC on stdio.
#[async_trait::async_trait]
pub trait LspClient: Send + Sync {
    async fn hover(&self, file: &str, line: i64, character: i64) -> ToolResult;
    async fn definition(&self, file: &str, line: i64, character: i64) -> ToolResult;
    async fn references(
        &self,
        file: &str,
        line: i64,
        character: i64,
        include_declaration: bool,
    ) -> ToolResult;
    async fn diagnostics(&self, file: &str, severity: Option<&str>) -> ToolResult;
    async fn document_symbols(&self, file: &str) -> ToolResult;
    async fn workspace_symbols(&self, query: &str) -> ToolResult;
    async fn completion(&self, file: &str, line: i64, character: i64) -> ToolResult;
    async fn signature_help(&self, file: &str, line: i64, character: i64) -> ToolResult;
    async fn code_actions(
        &self,
        file: &str,
        start_line: i64,
        start_char: i64,
        end_line: i64,
        end_char: i64,
    ) -> ToolResult;
    async fn formatting(&self, file: &str, tab_size: u32, insert_spaces: bool) -> ToolResult;
    async fn rename(&self, file: &str, line: i64, character: i64, new_name: &str) -> ToolResult;
    async fn code_lens(&self, file: &str) -> ToolResult;
    async fn prepare_rename(&self, file: &str, line: i64, character: i64) -> ToolResult;
    async fn code_action_resolve(&self, file: &str, action_index: i64) -> ToolResult;
}

// ---------------------------------------------------------------------------
// Client registry — set by the MCP server at startup
// ---------------------------------------------------------------------------

use std::sync::OnceLock;

static LSP_CLIENT: OnceLock<Box<dyn LspClient>> = OnceLock::new();

/// Register the LSP client implementation. Called once by the MCP server at startup.
pub fn set_lsp_client(client: Box<dyn LspClient>) {
    let _ = LSP_CLIENT.set(client);
}

/// Get a reference to the registered LSP client, if any.
pub fn get_lsp_client() -> Option<&'static dyn LspClient> {
    LSP_CLIENT.get().map(std::convert::AsRef::as_ref)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Get the registered LSP client, or return a "no server" error.
fn require_lsp_client(operation: &str) -> Result<&'static dyn LspClient, ToolResult> {
    get_lsp_client().ok_or_else(|| {
        ToolResult::error(format!(
            "No language server available.\n\n\
             Use lsp_servers to see available language servers.\n\
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

fn required_i64(args: &Value, key: &str) -> Result<i64, ToolResult> {
    args.get(key)
        .and_then(serde_json::Value::as_i64)
        .ok_or_else(|| ToolResult::error(format!("Missing required parameter: {key}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn all_12_tools_are_listed() {
        let tools = all_lsp_tools();
        assert_eq!(tools.len(), 12);
    }

    #[test]
    fn tool_names_are_unique() {
        let tools = all_lsp_tools();
        let mut names: Vec<&str> = tools.iter().map(|t| t.name).collect();
        let before = names.len();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), before, "duplicate tool names detected");
    }

    #[test]
    fn hover_schema_requires_file_line_character() {
        let info = lsp_hover_info();
        let schema = &info.input_schema;
        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("file")));
        assert!(required.contains(&json!("line")));
        assert!(required.contains(&json!("character")));
    }

    #[test]
    fn rename_schema_requires_new_name() {
        let info = lsp_rename_info();
        let schema = &info.input_schema;
        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("newName")));
    }

    #[test]
    fn missing_param_returns_error() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(lsp_hover(json!({})));
        assert_eq!(result.is_error, Some(true));
        assert!(
            result.content[0]
                .text
                .contains("Missing required parameter")
        );
    }

    #[test]
    fn no_client_returns_error() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(lsp_hover(json!({
            "file": "test.rs",
            "line": 1,
            "character": 0
        })));
        assert_eq!(result.is_error, Some(true));
        assert!(result.content[0].text.contains("No language server"));
    }

    #[test]
    fn formatting_schema_defaults() {
        let info = lsp_formatting_info();
        let schema = &info.input_schema;
        let required = schema["required"].as_array().unwrap();
        assert_eq!(required.len(), 1);
        assert!(required.contains(&json!("file")));
    }

    #[test]
    fn code_lens_is_read_only() {
        let info = lsp_code_lens_info();
        assert_eq!(info.annotations.read_only_hint, Some(true));
    }

    #[test]
    fn formatting_is_not_read_only() {
        let info = lsp_formatting_info();
        assert_eq!(info.annotations.read_only_hint, Some(false));
        assert_eq!(info.annotations.idempotent_hint, Some(true));
    }
}
