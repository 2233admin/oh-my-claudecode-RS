use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub use omc_shared::types::hooks::HookEvent;

/// ToolName represents the available Claude Code tools.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ToolName {
    Bash,
    Read,
    Write,
    Edit,
    Grep,
    Glob,
    Lsp,
    WebSearch,
    WebFetch,
    Task,
    NotepadRead,
    NotepadWrite,
    Agent,
    Other(String),
}

impl ToolName {
    /// Parse a tool name from a string (case-insensitive).
    pub fn parse_str(s: &str) -> Result<Self, String> {
        let normalized = s.trim();
        match normalized {
            "Bash" | "bash" => Ok(Self::Bash),
            "Read" | "read" => Ok(Self::Read),
            "Write" | "write" => Ok(Self::Write),
            "Edit" | "edit" => Ok(Self::Edit),
            "Grep" | "grep" => Ok(Self::Grep),
            "Glob" | "glob" => Ok(Self::Glob),
            "Lsp" | "lsp" => Ok(Self::Lsp),
            "WebSearch" | "web_search" | "web-search" | "websearch" => Ok(Self::WebSearch),
            "WebFetch" | "web_fetch" | "web-fetch" | "webfetch" => Ok(Self::WebFetch),
            "Task" | "task" => Ok(Self::Task),
            "NotepadRead" | "notepad_read" | "notepad-read" => Ok(Self::NotepadRead),
            "NotepadWrite" | "notepad_write" | "notepad-write" => Ok(Self::NotepadWrite),
            "Agent" | "agent" => Ok(Self::Agent),
            other => Ok(Self::Other(other.to_string())),
        }
    }

    /// Convert a tool name to its canonical string representation.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Bash => "Bash",
            Self::Read => "Read",
            Self::Write => "Write",
            Self::Edit => "Edit",
            Self::Grep => "Grep",
            Self::Glob => "Glob",
            Self::Lsp => "Lsp",
            Self::WebSearch => "WebSearch",
            Self::WebFetch => "WebFetch",
            Self::Task => "Task",
            Self::NotepadRead => "NotepadRead",
            Self::NotepadWrite => "NotepadWrite",
            Self::Agent => "Agent",
            Self::Other(s) => s,
        }
    }
}

impl std::fmt::Display for ToolName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for ToolName {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse_str(s)
    }
}

impl Serialize for ToolName {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ToolName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::parse_str(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_name_from_str() {
        assert_eq!(ToolName::parse_str("Bash").unwrap(), ToolName::Bash);
        assert_eq!(ToolName::parse_str("Read").unwrap(), ToolName::Read);
        assert_eq!(ToolName::parse_str("Write").unwrap(), ToolName::Write);
        assert_eq!(ToolName::parse_str("Edit").unwrap(), ToolName::Edit);
        assert_eq!(ToolName::parse_str("Grep").unwrap(), ToolName::Grep);
        assert_eq!(ToolName::parse_str("Glob").unwrap(), ToolName::Glob);
        assert_eq!(ToolName::parse_str("Lsp").unwrap(), ToolName::Lsp);
        assert_eq!(
            ToolName::parse_str("WebSearch").unwrap(),
            ToolName::WebSearch
        );
        assert_eq!(ToolName::parse_str("WebFetch").unwrap(), ToolName::WebFetch);
        assert_eq!(ToolName::parse_str("Task").unwrap(), ToolName::Task);
        assert_eq!(
            ToolName::parse_str("NotepadRead").unwrap(),
            ToolName::NotepadRead
        );
        assert_eq!(
            ToolName::parse_str("NotepadWrite").unwrap(),
            ToolName::NotepadWrite
        );
        assert_eq!(ToolName::parse_str("Agent").unwrap(), ToolName::Agent);
    }

    #[test]
    fn tool_name_from_str_lowercase() {
        assert_eq!(ToolName::parse_str("bash").unwrap(), ToolName::Bash);
        assert_eq!(ToolName::parse_str("read").unwrap(), ToolName::Read);
        assert_eq!(
            ToolName::parse_str("web_search").unwrap(),
            ToolName::WebSearch
        );
        assert_eq!(
            ToolName::parse_str("web-search").unwrap(),
            ToolName::WebSearch
        );
        assert_eq!(
            ToolName::parse_str("websearch").unwrap(),
            ToolName::WebSearch
        );
    }

    #[test]
    fn tool_name_from_str_other() {
        let tool = ToolName::parse_str("CustomTool").unwrap();
        assert!(matches!(tool, ToolName::Other(s) if s == "CustomTool"));
    }

    #[test]
    fn tool_name_as_str() {
        assert_eq!(ToolName::Bash.as_str(), "Bash");
        assert_eq!(ToolName::Read.as_str(), "Read");
        assert_eq!(ToolName::WebSearch.as_str(), "WebSearch");
        assert_eq!(ToolName::Other("Custom".to_string()).as_str(), "Custom");
    }

    #[test]
    fn tool_name_roundtrip() {
        for tool in [
            ToolName::Bash,
            ToolName::Read,
            ToolName::Write,
            ToolName::Edit,
            ToolName::Grep,
            ToolName::Glob,
            ToolName::Lsp,
            ToolName::WebSearch,
            ToolName::WebFetch,
            ToolName::Task,
            ToolName::NotepadRead,
            ToolName::NotepadWrite,
            ToolName::Agent,
        ] {
            assert_eq!(ToolName::parse_str(tool.as_str()).unwrap(), tool);
        }
    }

    #[test]
    fn tool_name_display() {
        assert_eq!(format!("{}", ToolName::Bash), "Bash");
        assert_eq!(
            format!("{}", ToolName::Other("Custom".to_string())),
            "Custom"
        );
    }

    #[test]
    fn tool_name_serde() {
        let tool = ToolName::Bash;
        let json = serde_json::to_string(&tool).unwrap();
        assert_eq!(json, "\"Bash\"");
        let parsed: ToolName = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, tool);

        let custom = ToolName::Other("MyTool".to_string());
        let custom_json = serde_json::to_string(&custom).unwrap();
        assert_eq!(custom_json, "\"MyTool\"");
    }

    #[test]
    fn tool_name_parse_trait() {
        let tool: ToolName = "Bash".parse().unwrap();
        assert_eq!(tool, ToolName::Bash);

        let tool: ToolName = "CustomTool".parse().unwrap();
        assert!(matches!(tool, ToolName::Other(s) if s == "CustomTool"));
    }
}
