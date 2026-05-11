//! Multi-protocol tool registry for routing tool calls across different backends.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

/// Metadata for a tool exposed through a protocol.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProtocolToolMeta {
    pub name: String,
    pub description: String,
    pub protocol: String,
    pub input_schema: serde_json::Value,
}

/// Result returned by a protocol tool execution.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProtocolToolResult {
    pub content: String,
    pub is_error: bool,
}

/// Trait implemented by each protocol backend (MCP, HTTP, gRPC, etc.).
#[async_trait::async_trait]
pub trait ToolProtocol: Send + Sync {
    /// Execute a tool by name with the given JSON parameters.
    async fn execute(
        &self,
        tool_name: &str,
        params: serde_json::Value,
    ) -> anyhow::Result<ProtocolToolResult>;

    /// List all tools provided by this protocol.
    async fn list_tools(&self) -> anyhow::Result<Vec<ProtocolToolMeta>>;

    /// Human-readable protocol name (e.g. "mcp", "http", "grpc").
    fn protocol_name(&self) -> &str;
}

/// Registry that routes tool calls to the correct protocol backend.
#[derive(Default)]
pub struct ProtocolRegistry {
    protocols: HashMap<String, Arc<dyn ToolProtocol>>,
    /// Maps tool name -> protocol name for auto-routing.
    tool_index: HashMap<String, String>,
}

/// Thread-safe shared registry.
pub type SharedRegistry = Arc<RwLock<ProtocolRegistry>>;

impl ProtocolRegistry {
    pub fn new() -> Self {
        Self {
            protocols: HashMap::new(),
            tool_index: HashMap::new(),
        }
    }

    /// Register a protocol and index all of its tools.
    pub async fn register(&mut self, protocol: Arc<dyn ToolProtocol>) -> anyhow::Result<()> {
        let name = protocol.protocol_name().to_string();
        let tools = protocol.list_tools().await?;
        for tool in tools {
            self.tool_index.insert(tool.name, name.clone());
        }
        self.protocols.insert(name, protocol);
        Ok(())
    }

    /// Rebuild the tool index from all registered protocols.
    pub async fn reindex(&mut self) -> anyhow::Result<()> {
        self.tool_index.clear();
        for (proto_name, protocol) in &self.protocols {
            let tools = protocol.list_tools().await?;
            for tool in tools {
                self.tool_index.insert(tool.name, proto_name.clone());
            }
        }
        Ok(())
    }

    /// Execute a tool on a specific protocol by name.
    pub async fn execute(
        &self,
        protocol: &str,
        tool: &str,
        params: serde_json::Value,
    ) -> anyhow::Result<ProtocolToolResult> {
        let proto = self
            .protocols
            .get(protocol)
            .ok_or_else(|| anyhow::anyhow!("unknown protocol: {protocol}"))?;
        proto.execute(tool, params).await
    }

    /// Execute a tool by auto-routing to the protocol that owns it.
    pub async fn execute_any(
        &self,
        tool: &str,
        params: serde_json::Value,
    ) -> anyhow::Result<ProtocolToolResult> {
        let proto_name = self
            .tool_index
            .get(tool)
            .ok_or_else(|| anyhow::anyhow!("unknown tool: {tool}"))?;
        self.execute(proto_name, tool, params).await
    }

    /// List all tools across all registered protocols.
    pub async fn list_all(&self) -> anyhow::Result<Vec<ProtocolToolMeta>> {
        let mut all = Vec::new();
        for protocol in self.protocols.values() {
            all.extend(protocol.list_tools().await?);
        }
        Ok(all)
    }

    /// Names of all registered protocols.
    pub fn protocol_names(&self) -> Vec<&str> {
        self.protocols.keys().map(std::string::String::as_str).collect()
    }

    /// Total number of indexed tools.
    pub fn tool_count(&self) -> usize {
        self.tool_index.len()
    }
}

/// Create a new shared (thread-safe) registry.
pub fn new_shared() -> SharedRegistry {
    Arc::new(RwLock::new(ProtocolRegistry::default()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal protocol stub for testing.
    struct StubProtocol {
        name: String,
        tools: Vec<ProtocolToolMeta>,
    }

    impl StubProtocol {
        fn new(name: &str, tool_names: &[&str]) -> Self {
            let tools = tool_names
                .iter()
                .map(|t| ProtocolToolMeta {
                    name: t.to_string(),
                    description: format!("{t} tool"),
                    protocol: name.to_string(),
                    input_schema: serde_json::json!({}),
                })
                .collect();
            Self {
                name: name.to_string(),
                tools,
            }
        }
    }

    #[async_trait::async_trait]
    impl ToolProtocol for StubProtocol {
        async fn execute(
            &self,
            tool_name: &str,
            _params: serde_json::Value,
        ) -> anyhow::Result<ProtocolToolResult> {
            Ok(ProtocolToolResult {
                content: format!("{}:{tool_name}", self.name),
                is_error: false,
            })
        }

        async fn list_tools(&self) -> anyhow::Result<Vec<ProtocolToolMeta>> {
            Ok(self.tools.clone())
        }

        fn protocol_name(&self) -> &str {
            &self.name
        }
    }

    #[tokio::test]
    async fn register_and_list_all() {
        let mut reg = ProtocolRegistry::default();
        let proto = Arc::new(StubProtocol::new("mcp", &["tool_a", "tool_b"]));
        reg.register(proto).await.unwrap();

        let all = reg.list_all().await.unwrap();
        assert_eq!(all.len(), 2);
        assert!(all.iter().any(|t| t.name == "tool_a"));
        assert!(all.iter().any(|t| t.name == "tool_b"));
        assert_eq!(reg.tool_count(), 2);
    }

    #[tokio::test]
    async fn execute_routes_to_correct_protocol() {
        let mut reg = ProtocolRegistry::default();
        let proto = Arc::new(StubProtocol::new("mcp", &["tool_a"]));
        reg.register(proto).await.unwrap();

        let result = reg
            .execute("mcp", "tool_a", serde_json::json!({}))
            .await
            .unwrap();
        assert_eq!(result.content, "mcp:tool_a");
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn execute_any_auto_routes() {
        let mut reg = ProtocolRegistry::default();
        let mcp = Arc::new(StubProtocol::new("mcp", &["tool_a"]));
        let http = Arc::new(StubProtocol::new("http", &["tool_b"]));
        reg.register(mcp).await.unwrap();
        reg.register(http).await.unwrap();

        let result_a = reg
            .execute_any("tool_a", serde_json::json!({}))
            .await
            .unwrap();
        assert_eq!(result_a.content, "mcp:tool_a");

        let result_b = reg
            .execute_any("tool_b", serde_json::json!({}))
            .await
            .unwrap();
        assert_eq!(result_b.content, "http:tool_b");
    }

    #[tokio::test]
    async fn unknown_protocol_returns_error() {
        let mut reg = ProtocolRegistry::default();
        let proto = Arc::new(StubProtocol::new("mcp", &["tool_a"]));
        reg.register(proto).await.unwrap();

        let err = reg
            .execute("grpc", "tool_a", serde_json::json!({}))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("unknown protocol"));
    }

    #[tokio::test]
    async fn unknown_tool_returns_error() {
        let mut reg = ProtocolRegistry::default();
        let proto = Arc::new(StubProtocol::new("mcp", &["tool_a"]));
        reg.register(proto).await.unwrap();

        let err = reg
            .execute_any("nonexistent", serde_json::json!({}))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("unknown tool"));
    }

    #[tokio::test]
    async fn multiple_protocols_with_different_tools() {
        let mut reg = ProtocolRegistry::default();
        let mcp = Arc::new(StubProtocol::new("mcp", &["read_state", "write_state"]));
        let http = Arc::new(StubProtocol::new("http", &["fetch_url"]));
        let grpc = Arc::new(StubProtocol::new("grpc", &["call_service"]));
        reg.register(mcp).await.unwrap();
        reg.register(http).await.unwrap();
        reg.register(grpc).await.unwrap();

        assert_eq!(reg.protocol_names().len(), 3);
        assert_eq!(reg.tool_count(), 4);

        let all = reg.list_all().await.unwrap();
        assert_eq!(all.len(), 4);
    }

    #[tokio::test]
    async fn reindex_after_late_registration() {
        let mut reg = ProtocolRegistry::default();
        let mcp = Arc::new(StubProtocol::new("mcp", &["tool_a"]));
        reg.register(mcp).await.unwrap();
        assert_eq!(reg.tool_count(), 1);

        // Add a second protocol after initial registration
        let http = Arc::new(StubProtocol::new("http", &["tool_b", "tool_c"]));
        reg.register(http).await.unwrap();
        assert_eq!(reg.tool_count(), 3);

        // Reindex should rebuild from all protocols
        reg.reindex().await.unwrap();
        assert_eq!(reg.tool_count(), 3);

        // Auto-routing should work for late-registered tools
        let result = reg
            .execute_any("tool_b", serde_json::json!({}))
            .await
            .unwrap();
        assert_eq!(result.content, "http:tool_b");
    }
}
