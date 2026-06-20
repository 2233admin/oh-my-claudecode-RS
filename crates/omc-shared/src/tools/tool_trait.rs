use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
pub type ToolParameters = HashMap<String, serde_json::Value>;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum ToolRiskLevel {
    ReadOnly,
    Standard,
    Dangerous,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExecResult {
    pub content: String,
    pub is_error: bool,
    pub metadata: Option<serde_json::Value>,
}

impl ExecResult {
    pub fn ok(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_error: false,
            metadata: None,
        }
    }

    pub fn err(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_error: true,
            metadata: None,
        }
    }
}

pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> serde_json::Value;
    fn execute(&self, parameters: ToolParameters) -> BoxFuture<'_, anyhow::Result<ExecResult>>;
    fn risk_level(&self) -> ToolRiskLevel {
        ToolRiskLevel::Standard
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockTool;

    impl Tool for MockTool {
        fn name(&self) -> &str {
            "mock_tool"
        }

        fn description(&self) -> &str {
            "A mock tool for testing"
        }

        fn parameters(&self) -> serde_json::Value {
            serde_json::json!({ "type": "object", "properties": {} })
        }

        fn execute(&self, params: ToolParameters) -> BoxFuture<'_, anyhow::Result<ExecResult>> {
            Box::pin(async move {
                if params.contains_key("fail") {
                    Ok(ExecResult::err("intentional failure"))
                } else {
                    Ok(ExecResult::ok("success"))
                }
            })
        }

        fn risk_level(&self) -> ToolRiskLevel {
            ToolRiskLevel::ReadOnly
        }
    }

    #[test]
    fn test_tool_metadata() {
        let tool = MockTool;
        assert_eq!(tool.name(), "mock_tool");
        assert_eq!(tool.description(), "A mock tool for testing");
        assert_eq!(tool.risk_level(), ToolRiskLevel::ReadOnly);
    }

    #[test]
    fn test_default_risk_level() {
        struct DefaultRisk;
        impl Tool for DefaultRisk {
            fn name(&self) -> &str {
                "default"
            }
            fn description(&self) -> &str {
                "d"
            }
            fn parameters(&self) -> serde_json::Value {
                serde_json::json!({})
            }
            fn execute(&self, _: ToolParameters) -> BoxFuture<'_, anyhow::Result<ExecResult>> {
                Box::pin(async { Ok(ExecResult::ok("ok")) })
            }
        }
        assert_eq!(DefaultRisk.risk_level(), ToolRiskLevel::Standard);
    }

    #[tokio::test]
    async fn test_execute_success() {
        let tool = MockTool;
        let result = tool.execute(HashMap::new()).await.unwrap();
        assert!(!result.is_error);
        assert_eq!(result.content, "success");
        assert!(result.metadata.is_none());
    }

    #[tokio::test]
    async fn test_execute_error() {
        let tool = MockTool;
        let mut params = HashMap::new();
        params.insert("fail".to_string(), serde_json::json!(true));
        let result = tool.execute(params).await.unwrap();
        assert!(result.is_error);
        assert_eq!(result.content, "intentional failure");
    }
}
