use serde::{Deserialize, Serialize};

/// Events emitted by an agent during execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    /// A chunk of streamed text token.
    Token { text: String },
    /// A tool call was initiated.
    ToolCall {
        name: String,
        arguments: serde_json::Value,
    },
    /// A tool returned a successful result.
    ToolResult {
        name: String,
        result: serde_json::Value,
    },
    /// A tool call produced an error.
    ToolError { name: String, error: String },
    /// The final answer from the agent.
    FinalAnswer { text: String },
    /// Extended thinking started.
    ThinkStart,
    /// Extended thinking ended.
    ThinkEnd,
    /// Context window was compressed.
    ContextCompressed {
        before_tokens: usize,
        after_tokens: usize,
    },
    /// A memory entry was recalled from the memory store.
    MemoryRecalled { key: String, summary: String },
    /// A progress update.
    Progress {
        message: String,
        percent: Option<f32>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_roundtrip_token() {
        let event = AgentEvent::Token {
            text: "hello".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: AgentEvent = serde_json::from_str(&json).unwrap();
        match back {
            AgentEvent::Token { text } => assert_eq!(text, "hello"),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_serialize_roundtrip_tool_call() {
        let event = AgentEvent::ToolCall {
            name: "read_file".to_string(),
            arguments: serde_json::json!({ "path": "/tmp/test" }),
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: AgentEvent = serde_json::from_str(&json).unwrap();
        match back {
            AgentEvent::ToolCall { name, arguments } => {
                assert_eq!(name, "read_file");
                assert_eq!(arguments["path"], "/tmp/test");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_serialize_roundtrip_all_variants() {
        let events = vec![
            AgentEvent::ToolResult {
                name: "t".to_string(),
                result: serde_json::json!(null),
            },
            AgentEvent::ToolError {
                name: "t".to_string(),
                error: "boom".to_string(),
            },
            AgentEvent::FinalAnswer {
                text: "done".to_string(),
            },
            AgentEvent::ThinkStart,
            AgentEvent::ThinkEnd,
            AgentEvent::ContextCompressed {
                before_tokens: 1000,
                after_tokens: 500,
            },
            AgentEvent::MemoryRecalled {
                key: "k".to_string(),
                summary: "s".to_string(),
            },
            AgentEvent::Progress {
                message: "working".to_string(),
                percent: Some(0.5),
            },
        ];

        for event in events {
            let json = serde_json::to_string(&event).unwrap();
            let back: AgentEvent = serde_json::from_str(&json).unwrap();
            let json2 = serde_json::to_string(&back).unwrap();
            assert_eq!(json, json2);
        }
    }

    #[test]
    fn test_json_has_type_tag() {
        let event = AgentEvent::ThinkStart;
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "think_start");
    }
}
