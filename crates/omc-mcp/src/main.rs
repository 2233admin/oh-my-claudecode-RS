//! MCP Tool Server entry point.
//!
//! Implements a JSON-RPC 2.0 server over stdin/stdout following the
//! Model Context Protocol (MCP) specification.
//!
//! Supports the following JSON-RPC methods:
//! - `initialize` - Server initialization handshake
//! - `notifications/initialized` - Client acknowledgment
//! - `tools/list` - List available tools
//! - `tools/call` - Invoke a tool

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{self, BufRead, Write};

use omc_mcp::{McpTool, ToolDefinition, all_tools};

// ============================================================================
// JSON-RPC types
// ============================================================================

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    #[serde(default)]
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Option<Value>,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i64,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

impl JsonRpcResponse {
    fn result(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: Some(result),
            error: None,
        }
    }

    fn error(id: Option<Value>, code: i64, message: String) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message,
                data: None,
            }),
        }
    }

    /// Notifications have no id and no result/error.
    fn notification() -> Self {
        Self {
            jsonrpc: "2.0",
            id: None,
            result: None,
            error: None,
        }
    }
}

// ============================================================================
// MCP server
// ============================================================================

struct McpServer {
    tools: Vec<Box<dyn McpTool>>,
}

impl McpServer {
    fn new() -> Self {
        Self { tools: all_tools() }
    }

    fn handle_request(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        match request.method.as_str() {
            "initialize" => self.handle_initialize(request.id),
            "notifications/initialized" => {
                // Client acknowledgment, no response needed (notification)
                JsonRpcResponse::notification()
            }
            "tools/list" => self.handle_tools_list(request.id),
            "tools/call" => self.handle_tools_call(request.id, request.params),
            _ => JsonRpcResponse::error(
                request.id,
                -32601,
                format!("Method not found: {}", request.method),
            ),
        }
    }

    fn handle_initialize(&self, id: Option<Value>) -> JsonRpcResponse {
        let result = serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "omc-mcp",
                "version": env!("CARGO_PKG_VERSION")
            }
        });
        JsonRpcResponse::result(id, result)
    }

    fn handle_tools_list(&self, id: Option<Value>) -> JsonRpcResponse {
        let definitions: Vec<ToolDefinition> = self.tools.iter().map(|t| t.definition()).collect();
        let tools: Vec<Value> = definitions
            .iter()
            .map(|d| serde_json::to_value(d).unwrap_or_default())
            .collect();

        let result = serde_json::json!({ "tools": tools });
        JsonRpcResponse::result(id, result)
    }

    fn handle_tools_call(&self, id: Option<Value>, params: Option<Value>) -> JsonRpcResponse {
        let params = match params {
            Some(p) => p,
            None => {
                return JsonRpcResponse::error(id, -32602, "Missing params".into());
            }
        };

        let tool_name = match params.get("name").and_then(|v| v.as_str()) {
            Some(n) => n,
            None => {
                return JsonRpcResponse::error(id, -32602, "Missing tool name".into());
            }
        };

        let args = params
            .get("arguments")
            .cloned()
            .unwrap_or(Value::Object(serde_json::Map::new()));

        // Find the tool
        let tool = match self.tools.iter().find(|t| t.definition().name == tool_name) {
            Some(t) => t,
            None => {
                return JsonRpcResponse::error(id, -32602, format!("Unknown tool: {tool_name}"));
            }
        };

        let result = tool.handle(args);
        match serde_json::to_value(&result) {
            Ok(value) => JsonRpcResponse::result(id, value),
            Err(e) => JsonRpcResponse::error(id, -32603, format!("Serialization error: {e}")),
        }
    }
}

// ============================================================================
// Main: JSON-RPC over stdio loop
// ============================================================================

fn main() -> io::Result<()> {
    let server = McpServer::new();
    let stdin = io::stdin().lock();
    let mut stdout = io::stdout().lock();

    for line in stdin.lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                eprintln!("omc-mcp: stdin read error: {e}");
                break;
            }
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let request: JsonRpcRequest = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                let response = JsonRpcResponse::error(None, -32700, format!("Parse error: {e}"));
                write_response(&mut stdout, &response)?;
                continue;
            }
        };

        let is_notification = request.id.is_none();
        let response = server.handle_request(request);

        // Don't send responses for notifications
        if !is_notification {
            write_response(&mut stdout, &response)?;
        }
    }

    Ok(())
}

fn write_response(stdout: &mut impl Write, response: &JsonRpcResponse) -> io::Result<()> {
    let json = serde_json::to_string(response).unwrap_or_default();
    stdout.write_all(json.as_bytes())?;
    stdout.write_all(b"\n")?;
    stdout.flush()
}
