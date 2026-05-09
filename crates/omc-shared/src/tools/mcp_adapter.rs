use std::sync::Arc;

use super::tool_trait::{BoxFuture, ExecResult, Tool, ToolParameters};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct McpToolDef {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// Wraps an MCP tool definition as a native Tool.
/// The actual MCP client call is abstracted via a callback.
pub struct McpToolAdapter {
    def: McpToolDef,
    caller: Arc<
        dyn Fn(String, serde_json::Value) -> BoxFuture<'static, anyhow::Result<String>>
            + Send
            + Sync,
    >,
}

impl McpToolAdapter {
    pub fn new(
        def: McpToolDef,
        caller: Arc<
            dyn Fn(String, serde_json::Value) -> BoxFuture<'static, anyhow::Result<String>>
                + Send
                + Sync,
        >,
    ) -> Self {
        Self { def, caller }
    }
}

impl Tool for McpToolAdapter {
    fn name(&self) -> &str {
        &self.def.name
    }

    fn description(&self) -> &str {
        &self.def.description
    }

    fn parameters(&self) -> serde_json::Value {
        self.def.input_schema.clone()
    }

    fn execute(&self, params: ToolParameters) -> BoxFuture<'_, anyhow::Result<ExecResult>> {
        Box::pin(async move {
            let args = serde_json::to_value(&params).unwrap_or_default();
            match (self.caller)(self.def.name.clone(), args).await {
                Ok(result) => Ok(ExecResult::ok(result)),
                Err(e) => Ok(ExecResult::err(e.to_string())),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn mock_caller_ok() -> Arc<
        dyn Fn(String, serde_json::Value) -> BoxFuture<'static, anyhow::Result<String>>
            + Send
            + Sync,
    > {
        Arc::new(|_name, _args| Box::pin(async { Ok("mcp result".to_string()) }))
    }

    fn mock_caller_err() -> Arc<
        dyn Fn(String, serde_json::Value) -> BoxFuture<'static, anyhow::Result<String>>
            + Send
            + Sync,
    > {
        Arc::new(|_name, _args| Box::pin(async { Err(anyhow::anyhow!("mcp failure")) }))
    }

    fn sample_def() -> McpToolDef {
        McpToolDef {
            name: "test_mcp".to_string(),
            description: "An MCP tool".to_string(),
            input_schema: serde_json::json!({ "type": "object" }),
        }
    }

    #[test]
    fn test_adapter_metadata() {
        let adapter = McpToolAdapter::new(sample_def(), mock_caller_ok());
        assert_eq!(adapter.name(), "test_mcp");
        assert_eq!(adapter.description(), "An MCP tool");
    }

    #[test]
    fn test_adapter_parameters() {
        let adapter = McpToolAdapter::new(sample_def(), mock_caller_ok());
        assert_eq!(
            adapter.parameters(),
            serde_json::json!({ "type": "object" })
        );
    }

    #[tokio::test]
    async fn test_adapter_execute_success() {
        let adapter = McpToolAdapter::new(sample_def(), mock_caller_ok());
        let result = adapter.execute(HashMap::new()).await.unwrap();
        assert!(!result.is_error);
        assert_eq!(result.content, "mcp result");
    }

    #[tokio::test]
    async fn test_adapter_execute_error() {
        let adapter = McpToolAdapter::new(sample_def(), mock_caller_err());
        let result = adapter.execute(HashMap::new()).await.unwrap();
        assert!(result.is_error);
        assert_eq!(result.content, "mcp failure");
    }

    #[tokio::test]
    async fn test_adapter_passes_params_to_caller() {
        let caller: Arc<
            dyn Fn(String, serde_json::Value) -> BoxFuture<'static, anyhow::Result<String>>
                + Send
                + Sync,
        > = Arc::new(|name, args| Box::pin(async move { Ok(format!("{}:{args}", name)) }));

        let adapter = McpToolAdapter::new(sample_def(), caller);
        let mut params = HashMap::new();
        params.insert("key".to_string(), serde_json::json!("val"));
        let result = adapter.execute(params).await.unwrap();
        assert!(result.content.starts_with("test_mcp:"));
    }
}
